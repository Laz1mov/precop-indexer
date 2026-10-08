fn main() {
    let db_path = std::env::var("TUSM_DB_PATH").unwrap_or_else(|_| "indexer_data_local".to_string());
    let db = sled::open(&db_path).expect("Failed to open Sled DB");

    let mut count = 0;
    for result in db.scan_prefix(b"crc21:uitem:") {
        if result.is_ok() {
            count += 1;
        }
    }
    
    println!("Total CRC-21 items minted (KOALA & others): {}", count);
}
