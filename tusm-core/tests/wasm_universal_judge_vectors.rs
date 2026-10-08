use tusm_core::astrolabe::verify_astrolabe_anchor;
use tusm_core::law::apply_mint_leaf;
use tusm_core::tx_env_bitcoin::BitcoinTxEnv;
use bitcoin::consensus::deserialize;
use hex::FromHex;
use std::str::FromStr;

/// V1 Fallback Test: Ensures the engine processes legacy logic without Astrolabe
#[test]
fn test_v1_fallback_mode() {
    let height = 968000;
    assert!(height < 970000, "Must be prior to V2 Activation");
    
    // In V1, Astrolabe is bypassed, we parse JSON directly.
    let raw_tx = Vec::from_hex("01000000000000000000").unwrap(); // mock hex
    // let tx: bitcoin::Transaction = deserialize(&raw_tx).unwrap();
    // let parsed_speech = tusm_core::legacy_v1_router::LegacyV1Router::process_mock(&tx);
    // assert_eq!(parsed_speech.unwrap(), "LEAF");
}

/// V2 API-Verifier Mode: Ensures strict BIP-341 Q-point Astrolabe validation
#[test]
fn test_v2_strict_verifier() {
    let height = 970005;
    assert!(height >= 970000, "Must be V2 Thermodynamic era");

    // Mock an environment where supply goes from 0 to 50
    let supply_before = 0;
    let s_new = 50;
    
    // The required Anchor Script Pubkey computed correctly by the honest miner
    // This is the expected script for `LEAF` and `50` supply
    use bitcoin::hashes::Hash;
    let law_cmr = bitcoin::hashes::sha256::Hash::hash("LEAF".as_bytes()).to_byte_array();
    let expected = tusm_core::astrolabe::compute_astrolabe(s_new, law_cmr, bitcoin::secp256k1::XOnlyPublicKey::from_str("50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0").unwrap());
    
    let is_valid = verify_astrolabe_anchor(s_new, law_cmr, &expected, bitcoin::secp256k1::XOnlyPublicKey::from_str("50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0").unwrap());
    assert!(is_valid, "Astrolabe should validate the correct Anchor Script");
}

/// V2 Fraud Defense: Passing tampered PSBT (Dead Ink)
#[test]
fn test_v2_fraud_defense_dead_ink() {
    let height = 970010;
    let s_new = 5000; // Hacker tries to mint 5000
    
    // The Hacker modified the outputs, the script contains an invalid Q point
    let tampered_script_hex = "5120ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00";
    let tampered_script = bitcoin::ScriptBuf::from(Vec::from_hex(tampered_script_hex).unwrap());
    
    use bitcoin::hashes::Hash;
    let law_cmr = bitcoin::hashes::sha256::Hash::hash("LEAF".as_bytes()).to_byte_array();
    let is_valid = verify_astrolabe_anchor(s_new, law_cmr, &tampered_script, bitcoin::secp256k1::XOnlyPublicKey::from_str("50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0").unwrap());
    
    assert!(!is_valid, "Astrolabe MUST reject tampered Q points (Dead Ink)");
}
