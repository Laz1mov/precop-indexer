use bitcoin::{Transaction, Block, OutPoint, ScriptBuf, TxOut, TxIn, Sequence, Witness};
use bitcoin::absolute::LockTime;
use bitcoin::hashes::Hash;
use std::collections::HashMap;
use std::sync::Arc;
use hex::FromHex;
use precop_tusm_indexer::scanner_engine::ScannerEngine;
use precop_tusm_indexer::state_db::{StateDb, SigmaLeaf};
use tusm_core::prevout::{Coin, PrevoutError, PrevoutProvider};

struct MockPrevoutProvider {
    cache: HashMap<OutPoint, Coin>,
}

impl PrevoutProvider for MockPrevoutProvider {
    fn lookup(&self, outpoint: OutPoint) -> Result<Coin, PrevoutError> {
        self.cache.get(&outpoint).cloned().ok_or(PrevoutError::MissingPrevout(outpoint))
    }
}

// =============================================================================
// TEST 1 (INVERSION): Unsigned / Forged Local JSON Injection MUST BE REJECTED
// =============================================================================
#[tokio::test]
async fn test_inversion_unauthenticated_forged_json_must_be_rejected() {
    let test_db_path = "/tmp/tusm_test_inversion_forgery_db";
    let _ = std::fs::remove_dir_all(test_db_path);
    let db = StateDb::new(test_db_path);

    let test_dir = "/tmp/test_inversion_forgery";
    let _ = std::fs::remove_dir_all(test_dir);
    std::fs::create_dir_all(test_dir).unwrap();

    let ext_path = format!("{}/external_payments.json", test_dir);
    let pending_path = format!("{}/pending_mints.jsonl", test_dir);

    std::env::set_var("TUSM_EXTERNAL_PAYMENTS_PATH", &ext_path);
    std::env::set_var("TUSM_PENDING_MINTS_PATH", &pending_path);

    // Initial state: CRC supply = 0
    let crc_state = SigmaLeaf {
        supply: 0,
        max_supply: 1_000_000_000_00000000,
        limit_per_mint: 10_000_000_00000000,
        btc_rate: 3333333000000,
        dog_rate: 47917,
        leaf_rate: 0,
        ordi_rate: 0,
        anchor: OutPoint::null(),
        height: 968894,
        version: 1,
        halted: false,
        tge_root: [0u8; 32],
        consumed_points: 0,
    };
    db.commit_block_batch(968894, "0000000000000000000000000000000000000000000000000000000000000000", &[("CRC".to_string(), crc_state.clone(), None)], &[]);

    let (tx_chan, mut rx_chan) = tokio::sync::mpsc::channel(100);
    tokio::spawn(async move { while rx_chan.recv().await.is_some() {} });

    let scanner = ScannerEngine {
        db: Arc::new(db.clone()),
        rpc_url: "http://127.0.0.1:8332".into(),
        rpc_user: "".into(),
        rpc_pass: "".into(),
        external_payments: HashMap::new(),
        pg_tx: tx_chan,
        crc21_tx: None,
    };

    // Attacker crafts an unauthenticated transaction WITHOUT a legitimate CSV Taproot seal
    // (e.g. output 2 is just a regular Taproot spendable key without lock script)
    let fake_attacker_tx = Transaction {
        version: 2,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::null(),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![
            // vout 0: OP_RETURN {"p":"crc-20","op":"mint","tick":"CRC"}
            TxOut {
                value: 0,
                script_pubkey: ScriptBuf::from_bytes(vec![
                    0x6a, 0x22, 0x7b, 0x22, 0x70, 0x22, 0x3a, 0x22, 0x63, 0x72, 0x63, 0x2d, 0x32, 0x30, 0x22, 0x2c, 0x22, 0x6f, 0x70, 0x22, 0x3a, 0x22, 0x6d, 0x69, 0x6e, 0x74, 0x22, 0x2c, 0x22, 0x74, 0x69, 0x63, 0x6b, 0x22, 0x3a, 0x22, 0x43, 0x52, 0x43, 0x22, 0x7d
                ]),
            },
            // vout 1: Treasury
            TxOut {
                value: 10_546,
                script_pubkey: ScriptBuf::from_hex("5120382b79806cc956d6f88236a8fdd70b7bfce89c2745b04f8f694df2302a853f92").unwrap(),
            },
            // vout 2: Attacker unsealed output (>= 330 sats, but NOT a covenant lock)
            TxOut {
                value: 330,
                script_pubkey: ScriptBuf::from_hex("51200000000000000000000000000000000000000000000000000000000000000001").unwrap(),
            },
        ],
    };

    let fake_txid = fake_attacker_tx.txid().to_string();

    // Attacker writes an unsigned forged entry directly into external_payments.json
    let forged_payload = serde_json::json!({
        fake_txid.clone(): {
            "asset": "DOG",
            "amount": 999999999999u64,
            "rate": 47913u64,
            "status": "confirmed"
            // NOTICE: NO cryptographic signature!
        }
    });
    std::fs::write(&ext_path, serde_json::to_string_pretty(&forged_payload).unwrap()).unwrap();

    let block = Block {
        header: bitcoin::block::Header {
            version: bitcoin::block::Version::from_consensus(1),
            prev_blockhash: bitcoin::BlockHash::all_zeros(),
            merkle_root: bitcoin::hash_types::TxMerkleNode::all_zeros(),
            time: 0,
            bits: bitcoin::CompactTarget::from_consensus(0),
            nonce: 0,
        },
        txdata: vec![fake_attacker_tx],
    };

    let mut prevout_map = HashMap::new();
    prevout_map.insert(OutPoint::null(), Coin { value: 100000, script_pubkey: ScriptBuf::new() });
    let provider = MockPrevoutProvider { cache: prevout_map };

    // Process the block
    let _ = scanner.process_block(&block, 968966, &provider).await;

    // INVERSION INVARIANT: The forged unauthenticated mint MUST NOT be credited!
    let final_sigma = scanner.db.get_sigma("CRC").unwrap();
    assert_eq!(
        final_sigma.supply, 0,
        "SECURITY CRITICAL: An unsigned forged entry in external_payments.json MUST NEVER inflate supply!"
    );

    // Clean up
    let _ = std::fs::remove_dir_all(test_db_path);
    let _ = std::fs::remove_dir_all(test_dir);
}

// =============================================================================
// TEST 2 (INVERSION): Tampered Attestation Signature MUST BE REJECTED
// =============================================================================
#[tokio::test]
async fn test_inversion_tampered_signature_must_be_rejected() {
    let test_db_path = "/tmp/tusm_test_inversion_tampered_sig_db";
    let _ = std::fs::remove_dir_all(test_db_path);

    let test_dir = "/tmp/test_inversion_tampered_sig";
    let _ = std::fs::remove_dir_all(test_dir);
    std::fs::create_dir_all(test_dir).unwrap();

    let ext_path = format!("{}/external_payments.json", test_dir);
    std::env::set_var("TUSM_EXTERNAL_PAYMENTS_PATH", &ext_path);

    // Attacker crafts an entry with a corrupted/tampered signature
    let fake_txid = "a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0";
    let tampered_payload = serde_json::json!({
        fake_txid: {
            "asset": "ORDI",
            "amount": "1000000000000000000",
            "status": "confirmed",
            "signature": "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
        }
    });
    std::fs::write(&ext_path, serde_json::to_string_pretty(&tampered_payload).unwrap()).unwrap();

    // Verify attestation parser rejects corrupted signature
    let reloaded = precop_tusm_indexer::scanner_engine::load_verified_external_payments(&ext_path);
    assert!(
        !reloaded.contains_key(fake_txid),
        "Tampered signature must be rejected and not included in active consensus payments!"
    );

    let _ = std::fs::remove_dir_all(test_db_path);
    let _ = std::fs::remove_dir_all(test_dir);
}

// =============================================================================
// TEST 3 (INVERSION): Valid Cryptographic Attestation IS ACCEPTED
// =============================================================================
#[tokio::test]
async fn test_inversion_authenticated_attestation_is_accepted() {
    let test_dir = "/tmp/test_inversion_valid_sig";
    let _ = std::fs::remove_dir_all(test_dir);
    std::fs::create_dir_all(test_dir).unwrap();

    let ext_path = format!("{}/external_payments.json", test_dir);
    std::env::set_var("TUSM_EXTERNAL_PAYMENTS_PATH", &ext_path);

    let valid_txid = "b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1";
    let amount: u128 = 5_000_000_000;
    let rate: u64 = 47913;
    let secret = "test_inversion_suite_secret_key_rfc2104";
    std::env::set_var("SIDECAR_ATTESTATION_SECRET", secret);
    let valid_sig = tusm_core::attestation::create_attestation_hmac(valid_txid, "DOG", amount, Some(rate), secret);

    let authenticated_payload = serde_json::json!({
        valid_txid: {
            "asset": "DOG",
            "amount": amount,
            "rate": rate,
            "status": "confirmed",
            "signature": valid_sig
        }
    });
    std::fs::write(&ext_path, serde_json::to_string_pretty(&authenticated_payload).unwrap()).unwrap();

    let reloaded = precop_tusm_indexer::scanner_engine::load_verified_external_payments(&ext_path);
    assert!(
        reloaded.contains_key(valid_txid),
        "Authentic attestation signature must be accepted by the consensus loader!"
    );
    let (asset_key, loaded_amt) = reloaded.get(valid_txid).unwrap();
    assert_eq!(asset_key, "DOG:47913");
    assert_eq!(*loaded_amt, amount);

    let _ = std::fs::remove_dir_all(test_dir);
}

// =============================================================================
// TEST 4 (INVERSION): CRC-21 O(1) Indexed Item Lookup vs Prefix Scan
// =============================================================================
#[test]
fn test_inversion_crc21_item_indexed_lookup() {
    let test_db_path = "/tmp/tusm_test_crc21_index_db";
    let _ = std::fs::remove_dir_all(test_db_path);
    let db = StateDb::new(test_db_path);

    // Insert 100 items
    for i in 0..100 {
        let outpoint = format!("txid_{}:{}", i, 0);
        let item_id = format!("koala_{}", i);
        let item_data = serde_json::json!({
            "item_id": item_id,
            "collection_id": "merkel_koala",
            "height": 969000
        });
        db.set_crc21_utxo_item(&outpoint, &item_data);
    }

    // Direct indexed lookup by item_id MUST succeed in O(1)
    let fetched = db.get_crc21_item_by_id("koala_42");
    assert!(fetched.is_some(), "Indexed item lookup must retrieve item data");
    let (outpoint, json) = fetched.unwrap();
    assert_eq!(outpoint, "txid_42:0");
    assert_eq!(json["item_id"], "koala_42");

    let _ = std::fs::remove_dir_all(test_db_path);
}
