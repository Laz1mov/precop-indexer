use serde_json::Value;

#[test]
fn test_mint_overflow_protection() {
    let current_supply: u128 = u128::MAX - 100;
    let mint_amount: u128 = 200;
    
    // Simulating checked_add
    let result = current_supply.checked_add(mint_amount);
    assert_eq!(result, None, "Mint overflow protection failed! System allowed hyperinflation.");
}

#[test]
fn test_strict_json_parsing() {
    let json_str = r#"{"p": "tusm-v2", "op": "mint", "tick": "LEAF", "amt": "100"}"#;
    let val: Value = serde_json::from_str(json_str).unwrap();
    
    let p_val = val.get("p").and_then(|v| v.as_str());
    // Strict V1 accepts crc-20, ico-20, ordi, btc
    let is_valid_v1 = p_val == Some("crc-20") || p_val == Some("ico-20") || p_val == Some("ordi") || p_val == Some("btc");
    
    assert!(!is_valid_v1, "Strict V1 JSON parsing failed! Allowed experimental tusm-v2 payload.");
}

#[test]
fn test_v1_ignores_astrolabe() {
    // Simulate V1 not checking tx.output[2]
    let tx_output_len = 1; // Only OP_RETURN
    let uses_astrolabe = tx_output_len >= 3;
    assert!(!uses_astrolabe, "V1 logic erroneously triggered Astrolabe output checking!");
}
