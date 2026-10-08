//! Security & Coverage Boost Tests
//! Tests attestation RFC 2104 HMAC verification, historical payment checks,
//! and V2 Law invariant enforcement to achieve high audit coverage (>85-90%).

use tusm_core::attestation::{
    is_canonical_historical_payment, verify_canonical_historical_payment,
    compute_attestation_digest, compute_hmac_rfc2104,
    verify_attestation_signature, create_attestation_hmac,
    CANONICAL_HISTORICAL_DOG_PAYMENTS,
};
use tusm_core::law::{check_invariants, apply_mint_leaf, RejectReason};
use tusm_core::state::SigmaLeaf;
use tusm_core::tx_env_bitcoin::BitcoinTxEnv;
use tusm_core::complexity_adapter::ComplexityAdapter;
use tusm_core::complexity::env::TxEnv;
use bitcoin::{Transaction, TxIn, TxOut, ScriptBuf, Sequence, Witness, OutPoint};

#[test]
fn test_attestation_canonical_historical_payments() {
    for (txid, amount, rate) in CANONICAL_HISTORICAL_DOG_PAYMENTS {
        assert!(is_canonical_historical_payment(txid, "DOG"));
        assert!(!is_canonical_historical_payment(txid, "BTC"));
        assert!(!is_canonical_historical_payment("0000000000000000000000000000000000000000000000000000000000000000", "DOG"));

        assert!(verify_canonical_historical_payment(txid, "DOG", amount, Some(rate)));
        // Tampered amount rejected
        assert!(!verify_canonical_historical_payment(txid, "DOG", amount + 1, Some(rate)));
        // Tampered rate rejected
        assert!(!verify_canonical_historical_payment(txid, "DOG", amount, Some(rate + 1)));
        // Missing rate rejected
        assert!(!verify_canonical_historical_payment(txid, "DOG", amount, None));
    }
}

#[test]
fn test_attestation_hmac_rfc2104_full_roundtrip() {
    let secret = "e9635c8657a15f14d06d8752bf19afecd3289b834d728345434590cfca2b8df0";
    let txid = "ed6996d754ade9b8443a54bf76c46e2415b9047a447e4076398f0cc1d859236c";
    let asset = "DOG";
    let amount = 30_000_000_000u128;
    let rate = Some(47917u64);

    std::env::set_var("SIDECAR_ATTESTATION_SECRET", secret);

    let digest = compute_attestation_digest(txid, asset, amount, rate);
    assert_ne!(digest, [0u8; 32]);
    let direct_hmac = compute_hmac_rfc2104(secret.as_bytes(), &digest);
    assert_ne!(direct_hmac, [0u8; 32]);

    let hmac_hex = create_attestation_hmac(txid, asset, amount, rate, secret);
    assert_eq!(hmac_hex.len(), 64);
    assert_eq!(hex::decode(&hmac_hex).unwrap(), direct_hmac.to_vec());

    let is_valid = verify_attestation_signature(txid, asset, amount, rate, &hmac_hex);
    assert!(is_valid, "Valid HMAC attestation signature must verify successfully");

    // Tampered txid
    let invalid_tx = verify_attestation_signature("1111111111111111111111111111111111111111111111111111111111111111", asset, amount, rate, &hmac_hex);
    assert!(!invalid_tx, "Tampered txid must be rejected");

    // Tampered amount
    let invalid_amt = verify_attestation_signature(txid, asset, amount + 1, rate, &hmac_hex);
    assert!(!invalid_amt, "Tampered amount must be rejected");

    // Tampered rate
    let invalid_rate = verify_attestation_signature(txid, asset, amount, Some(47913), &hmac_hex);
    assert!(!invalid_rate, "Tampered rate must be rejected");

    // Invalid signature hex
    assert!(!verify_attestation_signature(txid, asset, amount, rate, "deadbeef"));
    assert!(!verify_attestation_signature(txid, asset, amount, rate, "invalid_hex_string"));
}

#[test]
fn test_law_check_invariants() {
    let mut state = SigmaLeaf {
        supply: 100,
        max_supply: 1000,
        limit_per_mint: 100,
        btc_rate: 0,
        dog_rate: 0,
        leaf_rate: 0,
        ordi_rate: 0,
        anchor: vec![0u8; 36],
        height: 100,
        version: 1,
        halted: false,
        consumed_points: 0,
    };

    assert!(check_invariants(&state).is_ok());

    // Invariant violation: supply > max_supply
    state.supply = 1001;
    assert_eq!(check_invariants(&state), Err(RejectReason::Overflow));

    // Invariant violation: halted
    state.supply = 100;
    state.halted = true;
    assert_eq!(check_invariants(&state), Err(RejectReason::Halted));
}

#[test]
fn test_law_apply_mint_leaf_execution() {
    let mut witness = Witness::new();
    witness.push(vec![0; 32]);
    witness.push(vec![0; 960]);

    let tx = Box::new(Transaction {
        version: 2,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::null(),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::from_consensus(144),
            witness,
        }],
        output: vec![
            TxOut { value: 0, script_pubkey: ScriptBuf::new() },
            TxOut { value: 10000, script_pubkey: ScriptBuf::from_bytes(vec![0x51, 0x20, 0x38, 0x2b]) },
        ],
    });

    let utxos = Box::new(vec![TxOut {
        value: 100000,
        script_pubkey: ScriptBuf::new(),
    }]);

    let tx = Box::leak(tx);
    let utxos = Box::leak(utxos);

    let env = BitcoinTxEnv::new(tx, utxos);
    let user_pubkey_hash = env.jet_input_utxos_hash();
    let adapter = ComplexityAdapter { physical_env: env };
    let tge_root = adapter.jet_compute_merkle_root(user_pubkey_hash, &vec![0; 960]).unwrap_or([0u8; 32]);
    let env = adapter.physical_env;

    // Apply mint leaf
    let res = apply_mint_leaf(50_000_000, 100_000_000, 10_000_000, &tge_root, &env);
    assert!(res.is_ok(), "Valid mint leaf application should succeed");
    let new_supply = res.unwrap();
    assert!(new_supply > 50_000_000);
    assert!(new_supply <= 100_000_000);
}

#[test]
fn test_mock_tx_env_coverage() {
    use tusm_core::tx_env::{MockTxEnv, TusmTxEnv};

    let mock = MockTxEnv {
        num_inputs: 2,
        num_outputs: 2,
        output_values: vec![1000, 2000],
        output_scripts: vec![vec![0x51], vec![0x52]],
        input_sequences: vec![144, 1000],
        utxos_hash: [0xaa; 32],
    };

    assert_eq!(mock.num_inputs(), 2);
    assert_eq!(mock.num_outputs(), 2);
    assert_eq!(mock.output_value(0).unwrap(), 1000);
    assert_eq!(mock.output_value(1).unwrap(), 2000);
    assert_eq!(mock.output_value(2), Err(RejectReason::InvalidTopology));
    assert_eq!(mock.output_script_pubkey(0).unwrap(), &[0x51]);
    assert_eq!(mock.output_script_pubkey(2), Err(RejectReason::InvalidTopology));
    assert_eq!(mock.input_sequence(0).unwrap(), 144);
    assert_eq!(mock.input_sequence(2), Err(RejectReason::InvalidInputContext));
    assert_eq!(mock.input_utxos_hash(), [0xaa; 32]);
}

#[test]
fn test_attestation_schnorr_bip340_verification() {
    use bitcoin::secp256k1::{Secp256k1, KeyPair, Message};
    use bitcoin::secp256k1::rand::rngs::OsRng;
    
    let secp = Secp256k1::new();
    let keypair = KeyPair::new(&secp, &mut OsRng);
    let (xonly_pk, _) = keypair.x_only_public_key();
    
    let txid = "ed6996d754ade9b8443a54bf76c46e2415b9047a447e4076398f0cc1d859236c";
    let asset = "DOG";
    let amount = 30_000_000_000u128;
    let rate = Some(47917u64);

    let digest = compute_attestation_digest(txid, asset, amount, rate);
    let msg = Message::from_slice(&digest).unwrap();
    let sig = secp.sign_schnorr_no_aux_rand(&msg, &keypair);
    let sig_hex = hex::encode(sig.as_ref());

    std::env::set_var("CANONICAL_ORACLE_PUBKEY", xonly_pk.to_string());
    assert!(verify_attestation_signature(txid, asset, amount, rate, &sig_hex));

    // Tampered payload fails
    assert!(!verify_attestation_signature(txid, asset, amount + 1, rate, &sig_hex));
}

#[test]
fn test_law_deploy_v2_and_rejections() {
    let tx_empty = Box::leak(Box::new(Transaction {
        version: 2,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![],
        output: vec![
            TxOut { value: 0, script_pubkey: ScriptBuf::new() },
            TxOut { value: 0, script_pubkey: ScriptBuf::new() },
        ],
    }));
    let utxos_empty = Box::leak(Box::new(vec![]));
    let env_empty = BitcoinTxEnv::new(tx_empty, utxos_empty);
    assert_eq!(
        tusm_core::law::apply_deploy_v2("LEAF", &[0u8; 32], &[0u8; 32], &env_empty),
        Err(RejectReason::Halted)
    );
}
