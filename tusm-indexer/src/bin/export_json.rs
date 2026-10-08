use std::collections::HashMap;
use std::fs::File;
use std::io::Write;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = std::env::var("TUSM_DB_PATH").unwrap_or_else(|_| "data/tusm_indexer.db".to_string());
    let sled_db = sled::open(&db_path).expect("Failed to open local indexer db");
    
    let mut balances = HashMap::new();
    let prefix = b"bal:LEAF:";
    
    for row in sled_db.scan_prefix(prefix) {
        if let Ok((k, v)) = row {
            let key_str = String::from_utf8_lossy(&k);
            let address = key_str.trim_start_matches("bal:LEAF:");
            
            // Read Little-Endian from our V2 DB
            let mut buf = [0u8; 16];
            let len = std::cmp::min(v.len(), 16);
            buf[..len].copy_from_slice(&v[..len]);
            let amount = u128::from_le_bytes(buf);
            
            if amount > 0 {
                balances.insert(address.to_string(), amount.to_string());
            }
        }
    }
    
    let json_output = serde_json::to_string_pretty(&balances)?;
    let mut file = File::create("our_balances.json")?;
    file.write_all(json_output.as_bytes())?;
    
    println!("Exported {} accounts to our_balances.json", balances.len());
    Ok(())
}
