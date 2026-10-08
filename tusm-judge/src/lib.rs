use wasm_bindgen::prelude::*;
use tusm_core::state::SigmaLeaf;
use bitcoin::Transaction;
use bitcoin::consensus::encode::deserialize;
use serde_json;

#[wasm_bindgen]
pub fn evaluate_transaction(tx_hex: &str, _prevouts_hex: &str, current_sigma_json: &str, crc21_utxos_json: Option<String>) -> String {
    let tx_bytes = match hex::decode(tx_hex) {
        Ok(b) => b,
        Err(e) => return format!("{{\"error\":\"Invalid tx hex: {}\"}}", e),
    };
    let tx: Transaction = match deserialize(&tx_bytes) {
        Ok(t) => t,
        Err(e) => return format!("{{\"error\":\"Tx decode failed: {}\"}}", e),
    };

    // 1. Evaluate CRC-21 if requested or if flag is present
    let is_crc21_deploy = {
        let flag = br#"{"p":"crc-21","op":"deploy","type":"ord"}"#;
        tx.output.iter().any(|out| {
            out.script_pubkey.is_op_return() && out.script_pubkey.as_bytes().windows(flag.len()).any(|w| w == flag)
        })
    };

    if is_crc21_deploy {
        let mut col_id = None;
        if tx.output.len() >= 2 && tx.output[0].value == 546 && tx.output[1].value == 0 {
            col_id = Some(format!("{}i0", tx.txid()));
        }
        if let Some(id) = col_id {
            return format!("{{\"status\":\"accepted\", \"protocol\":\"crc-21\", \"event_type\":\"Deploy\", \"collection_id\":\"{}\"}}", id);
        }
    }

    // Evaluate Mint / Transfer for CRC-21 using utxos context
    if let Some(utxos_json) = crc21_utxos_json {
        if let Ok(utxos) = serde_json::from_str::<serde_json::Value>(&utxos_json) {
            let txid = tx.txid().to_string();
            // Expected format for utxos_json: {"parents": {"txid:vout": "col_id"}, "items": {"txid:vout": "item_id"}}
            if let Some(parents) = utxos.get("parents").and_then(|p| p.as_object()) {
                for input in &tx.input {
                    let prev = format!("{}:{}", input.previous_output.txid, input.previous_output.vout);
                    if let Some(col_id) = parents.get(&prev) {
                        if tx.output.len() >= 2 {
                            return format!("{{\"status\":\"accepted\", \"protocol\":\"crc-21\", \"event_type\":\"Mint\", \"collection_id\":{}, \"item_id\":\"{}:1\"}}", col_id, txid);
                        }
                    }
                }
            }
            if let Some(items) = utxos.get("items").and_then(|i| i.as_object()) {
                for input in &tx.input {
                    let prev = format!("{}:{}", input.previous_output.txid, input.previous_output.vout);
                    if let Some(item_id) = items.get(&prev) {
                        if !tx.output.is_empty() {
                            return format!("{{\"status\":\"accepted\", \"protocol\":\"crc-21\", \"event_type\":\"Transfer\", \"item_id\":{}, \"new_outpoint\":\"{}:0\"}}", item_id, txid);
                        }
                    }
                }
            }
        }
    }

    // 2. Evaluate CRC-20
    let sigma: SigmaLeaf = if current_sigma_json.trim().is_empty() || current_sigma_json.trim() == "{}" {
        SigmaLeaf {
            supply: 0,
            max_supply: 10_000_000_000_000_000,
            limit_per_mint: 1_000_000_000,
            btc_rate: 100_000_000,
            dog_rate: 100_000,
            leaf_rate: 100_000_000,
            ordi_rate: 1_000_000_000_000_000_000,
            anchor: vec![0u8; 36],
            height: 0,
            version: 2,
            halted: false,
            consumed_points: 0,
        }
    } else {
        match serde_json::from_str(current_sigma_json) {
            Ok(s) => s,
            Err(e) => return format!("{{\"error\":\"Invalid sigma JSON: {}\"}}", e),
        }
    };

    let mut balance_map = std::collections::HashMap::new();
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(current_sigma_json) {
        if let Some(b) = val.get("balances").and_then(|v| v.as_object()) {
            for (k, v) in b {
                if let Some(amt_str) = v.as_str() {
                    balance_map.insert(k.clone(), amt_str.parse::<u128>().unwrap_or(0));
                } else if let Some(amt_u64) = v.as_u64() {
                    balance_map.insert(k.clone(), amt_u64 as u128);
                }
            }
        }
    }

    let get_state = |_ticker: &str| -> SigmaLeaf {
        sigma.clone()
    };
    let get_balance = |_ticker: &str, address: &str| -> u128 {
        balance_map.get(address).copied().unwrap_or(0)
    };

    let empty_cache: std::collections::HashMap<bitcoin::OutPoint, tusm_core::prevout::Coin> = std::collections::HashMap::new();
    let empty_payments = std::collections::HashMap::new();
    let result = tusm_core::legacy_v1_router::LegacyV1Router::process(
        &tx, 
        get_state, 
        get_balance, 
        sigma.height, 
        &empty_cache,
        &empty_payments
    );

    match result {
        Ok(Some((ticker, new_state, event))) => {
            let mut response = serde_json::Map::new();
            response.insert("status".to_string(), serde_json::Value::String("accepted".to_string()));
            response.insert("protocol".to_string(), serde_json::Value::String("crc-20".to_string()));
            response.insert("ticker".to_string(), serde_json::Value::String(ticker));
            response.insert("new_supply".to_string(), serde_json::Value::String(new_state.supply.to_string()));
            if let Some(ev) = event {
                response.insert("event_type".to_string(), serde_json::Value::String(ev.2));
            }
            serde_json::to_string(&response).unwrap_or_else(|_| "{\"error\":\"json error\"}".to_string())
        },
        _ => {
            format!("{{\"status\":\"rejected\", \"reason\":\"Transaction did not pass TUSM Core consensus.\"}}")
        }
    }
}
