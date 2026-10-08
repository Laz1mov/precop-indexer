use bitcoin::{Transaction, Block, OutPoint, ScriptBuf, TxOut, TxIn, Sequence, Witness};
use bitcoin::absolute::LockTime;
use bitcoin::hashes::Hash;
use std::str::FromStr;
use std::collections::HashMap;
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

#[tokio::test]
async fn test_scanner_empty_to_dynamic_external_payment_resolution() {
    let test_db_path = "/tmp/tusm_test_dynamic_payments_db";
    let _ = std::fs::remove_dir_all(test_db_path);
    let db = StateDb::new(test_db_path);

    let test_dir = "/tmp/test_rust_sidecar_sync";
    let _ = std::fs::remove_dir_all(test_dir);
    std::fs::create_dir_all(test_dir).unwrap();

    let ext_path = format!("{}/external_payments.json", test_dir);
    let pending_path = format!("{}/pending_mints.jsonl", test_dir);

    std::env::set_var("TUSM_EXTERNAL_PAYMENTS_PATH", &ext_path);
    std::env::set_var("TUSM_PENDING_MINTS_PATH", &pending_path);

    // 1. Initial empty files
    std::fs::write(&ext_path, "{}").unwrap();
    std::fs::write(&pending_path, "").unwrap();

    // Deploy CRC initial state
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
    tokio::spawn(async move {
        while let Some(_) = rx_chan.recv().await {}
    });

    let scanner = ScannerEngine {
        db: std::sync::Arc::new(db),
        rpc_url: "".to_string(),
        rpc_user: "".to_string(),
        rpc_pass: "".to_string(),
        external_payments: HashMap::new(),
        pg_tx: tx_chan,
        crc21_tx: None,
    };

    // Construct mock block with candidate transaction c892254b... (DOG Mint)
    let mut tx = Transaction {
        version: 2,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::null(),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![],
    };

    // vout[0]: OP_RETURN {"p":"crc-20","op":"mint","tick":"CRC"}
    let mut spk0 = vec![0x6a];
    let payload = r#"{"p":"crc-20","op":"mint","tick":"CRC"}"#.as_bytes();
    spk0.push(payload.len() as u8);
    spk0.extend_from_slice(payload);
    tx.output.push(TxOut {
        value: 0,
        script_pubkey: ScriptBuf::from_bytes(spk0),
    });

    // vout[1]: Treasury
    let treasury_spk = ScriptBuf::from_hex("5120382b79806cc956d6f88236a8fdd70b7bfce89c2745b04f8f694df2302a853f92").unwrap();
    tx.output.push(TxOut {
        value: 10_546,
        script_pubkey: treasury_spk,
    });

    // vout[2]: Seal lock 2100 (330 sats)
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let nums = bitcoin::secp256k1::XOnlyPublicKey::from_str("50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0").unwrap();
    let seal_spk = tusm_core::legacy_v1_router::LegacyV1Router::derive_taproot_for_lock(&secp, 2100, nums, true);
    tx.output.push(TxOut {
        value: 330,
        script_pubkey: seal_spk,
    });

    let txid_str = tx.txid().to_string();

    let block = Block {
        header: bitcoin::block::Header {
            version: bitcoin::block::Version::NO_SOFT_FORK_SIGNALLING,
            prev_blockhash: bitcoin::hash_types::BlockHash::all_zeros(),
            merkle_root: bitcoin::hash_types::TxMerkleNode::all_zeros(),
            time: 0,
            bits: bitcoin::CompactTarget::from_consensus(0),
            nonce: 0,
        },
        txdata: vec![tx],
    };

    let mut prevout_map = HashMap::new();
    prevout_map.insert(OutPoint::null(), Coin { value: 100000, script_pubkey: ScriptBuf::new() });
    let provider = MockPrevoutProvider { cache: prevout_map };

    // Pass 1: Process block with EMPTY external_payments.
    // The candidate transaction will be logged into pending_mints.jsonl.
    let ok1 = scanner.process_block(&block, 968966, &provider).await;
    assert!(ok1, "Block should process without panic");

    // Assert that pending_mints.jsonl now contains this txid!
    let pending_content = std::fs::read_to_string(&pending_path).unwrap();
    assert!(pending_content.contains(&txid_str), "Pending mints must contain candidate txid");
    println!("✅ Pass 1 Passed: Candidate txid logged to pending_mints.jsonl");

    let sig = tusm_core::attestation::create_attestation_hmac(
        &txid_str,
        "DOG",
        5000000000,
        Some(47913),
        tusm_core::attestation::DEFAULT_ATTESTATION_SECRET,
    );

    let mock_verified_payments = serde_json::json!({
        txid_str.clone(): {
            "asset": "DOG",
            "amount": 5000000000u64,
            "rate": 47913u64,
            "status": "confirmed",
            "signature": sig
        }
    });
    std::fs::write(&ext_path, serde_json::to_string_pretty(&mock_verified_payments).unwrap()).unwrap();

    // Reset supply to 0 so Pass 2 measures the exact DOG mint
    let mut fresh_crc = crc_state.clone();
    fresh_crc.supply = 0;
    scanner.db.commit_block_batch(968894, "0000000000000000000000000000000000000000000000000000000000000000", &[("CRC".to_string(), fresh_crc, None)], &[]);

    // Pass 2: Process the block again with the dynamically populated external_payments.json
    let ok2 = scanner.process_block(&block, 968966, &provider).await;
    assert!(ok2, "Block should process successfully with external payments");

    // Assert that the CRC supply has been updated with the exact DOG formula!
    // 50 000 DOG x (47 913 / 5 000) x 1.136010 = 54 429 647 130 000 atoms
    let final_sigma = scanner.db.get_sigma("CRC").unwrap();
    assert_eq!(final_sigma.supply, 54_429_647_130_000u128, "Exact DOG mint allocation must match");
    println!("✅ Pass 2 Passed: Dynamic external payment reloaded and minted exact 544,296.4713 CRC tokens!");

    // Clean up
    let _ = std::fs::remove_dir_all(test_db_path);
    let _ = std::fs::remove_dir_all(test_dir);
}
