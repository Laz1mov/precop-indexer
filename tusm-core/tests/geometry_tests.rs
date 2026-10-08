use bitcoin::{Transaction, TxOut, ScriptBuf, TxIn, Witness, OutPoint, Sequence};
use std::collections::HashMap;

// A factored out structure for detecting geometry
#[derive(Debug, PartialEq)]
pub struct GeometryResult {
    pub payment_asset: String,
    pub payment_atoms: u128,
    pub seal_found: bool,
    pub matched_multiplier: u64,
    pub treasury_sats: u64,
}

pub fn detect_geometry(
    tx: &Transaction,
    i: usize, // OP_RETURN index
    seal_index: Option<usize>,
    external_payments: &HashMap<String, (String, u128)>
) -> Option<GeometryResult> {
    let mut payment_asset = "UNKNOWN".to_string();
    let mut payment_atoms = 0u128;
    let mut treasury_sats = 0u64;
    let mut seal_found = false;
    let mut matched_multiplier = 0u64;

    let txid_str = tx.txid().to_string();

    if let Some((asset, amount)) = external_payments.get(&txid_str) {
        payment_asset = asset.clone();
        payment_atoms = *amount;
    } else if tx.output.len() >= 2 {
        let bons_payload: [u8; 32] = [
            0x77, 0xd8, 0xbb, 0x78, 0xcc, 0x47, 0xa7, 0x15, 
            0xf3, 0xf0, 0xe9, 0x84, 0x63, 0xe2, 0xd7, 0x38, 
            0xa9, 0x82, 0xfc, 0xfb, 0x74, 0x26, 0x0c, 0x86, 
            0x14, 0x37, 0x2b, 0x41, 0xae, 0xf5, 0x14, 0x65
        ];
        
        if tx.output.len() >= 3 && tx.output[2].script_pubkey.as_bytes().ends_with(&bons_payload) {
            payment_asset = "BTC".to_string();
            payment_atoms = tx.output[2].value as u128;
            treasury_sats = tx.output[2].value;
            seal_found = true;
            matched_multiplier = 10_000_000;
        } else if i == 1 && seal_index == Some(2) && tx.output.len() >= 3 {
            payment_asset = "BTC".to_string();
            payment_atoms = tx.output[0].value as u128;
            treasury_sats = tx.output[0].value;
        } else if i == 2 && seal_index == Some(3) && tx.output.len() >= 4 && tx.output[0].script_pubkey.is_op_return() && tx.output[0].script_pubkey.as_bytes().len() > 2 {
            let script_bytes = tx.output[0].script_pubkey.as_bytes();
            if let Ok(json_str) = std::str::from_utf8(&script_bytes[2..]) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                    let p = val.get("p").and_then(|v| v.as_str());
                    let tick = val.get("tick").and_then(|v| v.as_str()).map(|s| s.to_lowercase());
                    if p == Some("ico-20") && tick.as_deref() == Some("leaf") {
                        if let Some(amt_str) = val.get("amt").and_then(|v| v.as_str()) {
                            if let Ok(a) = amt_str.parse::<u128>() {
                                payment_asset = "LEAF".to_string();
                                payment_atoms = a;
                            }
                        }
                    } else if p == Some("brc-20") && tick.as_deref() == Some("ordi") {
                        if let Some(amt_str) = val.get("amt").and_then(|v| v.as_str()) {
                            if let Ok(a) = amt_str.parse::<u128>() {
                                payment_asset = "ORDI".to_string();
                                payment_atoms = a;
                            }
                        }
                    }
                }
            }
        } else if i + 1 < tx.output.len() && tx.output[i + 1].script_pubkey.is_v1_p2tr() {
            payment_asset = "BTC".to_string();
            payment_atoms = tx.output[i + 1].value as u128;
            treasury_sats = tx.output[i + 1].value;
        } else if seal_index == Some(i + 1) && tx.output.len() > i + 2 {
            payment_asset = "BTC".to_string();
            payment_atoms = tx.output[i + 2].value as u128;
            treasury_sats = tx.output[i + 2].value;
        }
    }

    if payment_asset != "UNKNOWN" {
        Some(GeometryResult {
            payment_asset,
            payment_atoms,
            seal_found,
            matched_multiplier,
            treasury_sats,
        })
    } else {
        None
    }
}

fn build_tx(outputs: Vec<TxOut>) -> Transaction {
    Transaction {
        version: 2,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::null(),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: outputs,
    }
}

#[test]
fn test_geometry_1_btc() {
    let out0 = TxOut { value: 0, script_pubkey: ScriptBuf::from_bytes(vec![0x6a]) }; // OP_RETURN
    let out1 = TxOut { value: 1000, script_pubkey: ScriptBuf::from_bytes(vec![0x51, 0x20, 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]) }; // P2TR Treasury
    let tx = build_tx(vec![out0, out1]);
    
    let res = detect_geometry(&tx, 0, None, &HashMap::new());
    assert!(res.is_some());
    let geom = res.unwrap();
    assert_eq!(geom.payment_asset, "BTC");
    assert_eq!(geom.payment_atoms, 1000);
}

#[test]
fn test_geometry_leaf() {
    let payload = r#"{"p":"ico-20","op":"transfer","tick":"LEAF","amt":"5000000"}"#;
    let mut script = vec![0x6a, payload.len() as u8];
    script.extend_from_slice(payload.as_bytes());

    let out0 = TxOut { value: 0, script_pubkey: ScriptBuf::from_bytes(script) }; // LEAF transfer
    let out1 = TxOut { value: 0, script_pubkey: ScriptBuf::new() }; // Sink
    let out2 = TxOut { value: 0, script_pubkey: ScriptBuf::from_bytes(vec![0x6a]) }; // OP_RETURN mint
    let out3 = TxOut { value: 0, script_pubkey: ScriptBuf::new() }; // Seal

    let tx = build_tx(vec![out0, out1, out2, out3]);
    let res = detect_geometry(&tx, 2, Some(3), &HashMap::new());
    
    assert!(res.is_some());
    let geom = res.unwrap();
    assert_eq!(geom.payment_asset, "LEAF");
    assert_eq!(geom.payment_atoms, 5000000);
}
