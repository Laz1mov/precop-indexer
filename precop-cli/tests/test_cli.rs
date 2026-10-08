use std::process::Command;

#[test]
fn test_cli_help() {
    let output = Command::new("cargo")
        .args(&["run", "-p", "precop-cli", "--", "--help"])
        .output()
        .expect("Failed to execute precop CLI");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Autonomous Layer 1 consensus validator"));
    assert!(stdout.contains("verify-tx"));
    assert!(stdout.contains("parse-speech"));
    assert!(stdout.contains("curve"));
    assert!(stdout.contains("query"));
    assert!(stdout.contains("db"));
    assert!(stdout.contains("daemon"));
}

#[test]
fn test_cli_parse_leaf_mint_speech_no_amt() {
    // Canonical LEAF Mint payload: strictly {"p":"crc-20","op":"mint","tick":"LEAF"}
    // Notice: NO 'amt' field exists in mint! Allocation is determined by L1 consensus geometry.
    let script_hex = "6a237b2270223a226372632d3230222c226f70223a226d696e74222c227469636b223a224c454146227d";
    let output = Command::new("cargo")
        .args(&["run", "-p", "precop-cli", "--", "parse-speech", "--input", script_hex])
        .output()
        .expect("Failed to execute precop CLI");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"p\": \"crc-20\""));
    assert!(stdout.contains("\"op\": \"mint\""));
    assert!(stdout.contains("\"tick\": \"LEAF\""));
    // Explicitly verify NO amt is present in the payload
    assert!(!stdout.contains("\"amt\""));
}

#[test]
fn test_cli_parse_leaf_transfer_speech_with_amt() {
    // Canonical LEAF Transfer payload: {"p":"crc-20","op":"transfer","tick":"LEAF","amt":"50000"}
    // In transfer, 'amt' is required because the sender specifies the transferred amount.
    let script_hex = "6a377b2270223a226372632d3230222c226f70223a227472616e73666572222c227469636b223a224c454146222c22616d74223a223530303030227d";
    let output = Command::new("cargo")
        .args(&["run", "-p", "precop-cli", "--", "parse-speech", "--input", script_hex])
        .output()
        .expect("Failed to execute precop CLI");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"p\": \"crc-20\""));
    assert!(stdout.contains("\"op\": \"transfer\""));
    assert!(stdout.contains("\"tick\": \"LEAF\""));
    assert!(stdout.contains("\"amt\": \"50000\""));
}

#[test]
fn test_cli_parse_bons_mint_speech_no_amt() {
    // Canonical BONS Mint payload: strictly {"p":"crc-20","op":"mint","tick":"BONS"}
    let script_hex = "6a237b2270223a226372632d3230222c226f70223a226d696e74222c227469636b223a22424f4e53227d";
    let output = Command::new("cargo")
        .args(&["run", "-p", "precop-cli", "--", "parse-speech", "--input", script_hex])
        .output()
        .expect("Failed to execute precop CLI");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"p\": \"crc-20\""));
    assert!(stdout.contains("\"op\": \"mint\""));
    assert!(stdout.contains("\"tick\": \"BONS\""));
    assert!(!stdout.contains("\"amt\""));
}

#[test]
fn test_cli_parse_crc_mint_speech_no_amt() {
    // Canonical CRC Mint payload: strictly {"p":"crc-20","op":"mint","tick":"CRC"}
    let script_hex = "6a227b2270223a226372632d3230222c226f70223a226d696e74222c227469636b223a22435243227d";
    let output = Command::new("cargo")
        .args(&["run", "-p", "precop-cli", "--", "parse-speech", "--input", script_hex])
        .output()
        .expect("Failed to execute precop CLI");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"p\": \"crc-20\""));
    assert!(stdout.contains("\"op\": \"mint\""));
    assert!(stdout.contains("\"tick\": \"CRC\""));
    assert!(!stdout.contains("\"amt\""));
}

#[test]
fn test_cli_parse_crc21_koala_deploy_speech() {
    // Canonical CRC-21 Merkel Koala deploy payload: {"p":"crc-21","op":"deploy","type":"ord"}
    let script_hex = "6a297b2270223a226372632d3231222c226f70223a226465706c6f79222c2274797065223a226f7264227d";
    let output = Command::new("cargo")
        .args(&["run", "-p", "precop-cli", "--", "parse-speech", "--input", script_hex])
        .output()
        .expect("Failed to execute precop CLI");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"p\": \"crc-21\""));
    assert!(stdout.contains("\"op\": \"deploy\""));
    assert!(stdout.contains("\"type\": \"ord\""));
}

#[test]
fn test_cli_curve_quote_bons() {
    let output = Command::new("cargo")
        .args(&[
            "run", "-p", "precop-cli", "--", "curve", "quote",
            "--ticker", "BONS",
            "--deposit-sats", "50000"
        ])
        .output()
        .expect("Failed to execute precop CLI");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ticker\": \"BONS\""));
    assert!(stdout.contains("\"allocated_atoms\": 6082837338995"));
    assert!(stdout.contains("\"allocated_formatted\": \"6082.837338995\""));
}

#[test]
fn test_cli_curve_spot_price_bons() {
    let output = Command::new("cargo")
        .args(&[
            "run", "-p", "precop-cli", "--", "curve", "spot-price",
            "--ticker", "BONS"
        ])
        .output()
        .expect("Failed to execute precop CLI");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ticker\": \"BONS\""));
    assert!(stdout.contains("\"spot_price_usd\": 0.00074"));
    assert!(stdout.contains("\"target_raised_usd\": 35000.0"));
}

#[test]
fn test_cli_verify_crc21_koala_deploy_tx() {
    // Construct a valid CRC-21 deploy transaction:
    // vout 0: 546 sats carrier UTXO
    // vout 1: OP_RETURN {"p":"crc-21","op":"deploy","type":"ord"}
    use bitcoin::{Transaction, TxOut, ScriptBuf};
    use bitcoin::locktime::absolute::LockTime;

    let flag = br#"{"p":"crc-21","op":"deploy","type":"ord"}"#;
    let push_buf = bitcoin::script::PushBytesBuf::try_from(flag.to_vec()).unwrap();

    let tx = Transaction {
        version: 2,
        lock_time: LockTime::ZERO,
        input: vec![],
        output: vec![
            TxOut { value: 546, script_pubkey: ScriptBuf::new() },
            TxOut { value: 0, script_pubkey: ScriptBuf::new_op_return(&push_buf) },
        ],
    };

    let tx_hex = hex::encode(bitcoin::consensus::encode::serialize(&tx));
    let expected_collection_id = format!("{}i0", tx.txid());

    let output = Command::new("cargo")
        .args(&[
            "run", "-p", "precop-cli", "--", "verify-tx",
            "--tx-hex", &tx_hex
        ])
        .output()
        .expect("Failed to execute precop CLI");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"status\": \"accepted\""));
    assert!(stdout.contains("\"protocol\": \"crc-21\""));
    assert!(stdout.contains("\"event_type\": \"Deploy\""));
    assert!(stdout.contains(&expected_collection_id));
}

#[test]
fn test_cli_verify_leaf_mint_tx_no_amt() {
    // Construct a canonical LEAF mint transaction:
    // vout 0: Treasury payment (50,000 sats)
    // vout 1: OP_RETURN strictly {"p":"crc-20","op":"mint","tick":"LEAF"} (NO amt!)
    // vout 2: Taproot seal with CSV lock 1000 blocks
    use bitcoin::{Transaction, TxOut, TxIn, OutPoint, ScriptBuf, Witness, Sequence};
    use bitcoin::locktime::absolute::LockTime;
    use bitcoin::secp256k1::{Secp256k1, SecretKey, PublicKey, XOnlyPublicKey};
    use tusm_core::legacy_v1_router::LegacyV1Router;

    let secp = Secp256k1::new();
    let sk = SecretKey::from_slice(&[42u8; 32]).unwrap();
    let pk = PublicKey::from_secret_key(&secp, &sk);
    let owner_key = XOnlyPublicKey::from(pk);

    let seal_spk = LegacyV1Router::derive_taproot_for_lock(&secp, 1000, owner_key, false);

    // Canonical speech: strictly {"p":"crc-20","op":"mint","tick":"LEAF"} - NO amt!
    let mint_payload = br#"{"p":"crc-20","op":"mint","tick":"LEAF"}"#;
    let push_buf = bitcoin::script::PushBytesBuf::try_from(mint_payload.to_vec()).unwrap();

    let mut witness = Witness::new();
    witness.push([0u8; 64]);
    witness.push(owner_key.serialize());

    let tx = Transaction {
        version: 2,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::null(),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness,
        }],
        output: vec![
            TxOut { value: 50_000, script_pubkey: ScriptBuf::new() }, // Treasury
            TxOut { value: 0, script_pubkey: ScriptBuf::new_op_return(&push_buf) }, // OP_RETURN Mint (NO amt)
            TxOut { value: 546, script_pubkey: seal_spk }, // Seal
        ],
    };

    let tx_hex = hex::encode(bitcoin::consensus::encode::serialize(&tx));

    let output = Command::new("cargo")
        .args(&[
            "run", "-p", "precop-cli", "--", "verify-tx",
            "--tx-hex", &tx_hex
        ])
        .output()
        .expect("Failed to execute precop CLI");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"status\": \"accepted\""));
    assert!(stdout.contains("\"protocol\": \"crc-20\""));
    assert!(stdout.contains("\"ticker\": \"LEAF\""));
    assert!(stdout.contains("\"event_type\": \"mint\""));
}

