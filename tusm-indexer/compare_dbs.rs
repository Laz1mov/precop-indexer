use sled::Db;
use std::collections::HashMap;

fn get_balances(db: &Db, ticker: &str) -> HashMap<String, u128> {
    let prefix = format!("bal:{}:", ticker);
    let mut balances = HashMap::new();
    let mut debug_count = 0;
    for res in db.scan_prefix(prefix.as_bytes()) {
        if let Ok((k, v)) = res {
            let key_str = String::from_utf8_lossy(&k);
            if let Some(addr) = key_str.strip_prefix(&prefix) {
                if v.len() == 16 {
                    let mut bytes = [0u8; 16];
                    bytes.copy_from_slice(&v);
                    let bal = u128::from_le_bytes(bytes);
                    if debug_count < 5 {
                        println!("DEBUG key={} len={} bal={}", key_str, v.len(), bal);
                        debug_count += 1;
                    }
                    if bal > 0 {
                        balances.insert(addr.to_string(), bal);
                    }
                }
            }
        }
    }
    balances
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let snapshot_db = sled::open("snapshot_db").expect("Failed to open snapshot_db");
    let snapshot_balances = get_balances(&snapshot_db, "LEAF");
    
    println!("Snapshot has {} balances for LEAF", snapshot_balances.len());
    
    let client = reqwest::Client::new();
    let mut mismatches = 0;
    
    for (addr, snapshot_bal) in &snapshot_balances {
        let url = format!("http://127.0.0.1:8080/api/v1/crc20/balance/LEAF/{}", addr);
        let res = client.get(&url).send().await;
        
        match res {
            Ok(response) => {
                let json: serde_json::Value = response.json().await?;
                if let Some(api_bal_str) = json["balance"].as_str() {
                    if let Ok(api_bal) = api_bal_str.parse::<u128>() {
                        if *snapshot_bal != api_bal {
                            let diff = if *snapshot_bal > api_bal { *snapshot_bal - api_bal } else { api_bal - *snapshot_bal };
                            println!("Mismatch for {}: snapshot={}, api={} (Diff: {})", addr, snapshot_bal, api_bal, diff);
                            mismatches += 1;
                        }
                    }
                }
            }
            Err(_) => {}
        }
    }
    println!("Comparison finished. Mismatches: {}", mismatches);
    Ok(())
}
