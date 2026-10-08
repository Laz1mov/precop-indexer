use precop_tusm_indexer::state_db::StateDb;

fn main() {
    let db_path = "./tusm_indexer.db";
    let db = StateDb::new(db_path);
    
    println!("=== ADVANCED PRECOPSCAN DB AUDIT ===");
    println!("Database: {}", db_path);
    println!("Last Scanned Height: {}", db.get_last_scanned_height());
    
    println!("\n--- CRC-20 Tokens ---");
    let tokens = db.get_all_tokens();
    for (t, sigma) in tokens {
        println!("Ticker: {}", t);
        println!("  Supply: {} / {}", sigma.supply, sigma.max_supply);
        println!("  Deployed Height: {}", sigma.height);
    }
    
    println!("\n--- CRC-21 Collections ---");
    let mut count = 0;
    for res in db.db.scan_prefix("crc21:collection:") {
        if let Ok((key, _)) = res {
            println!("Found CRC-21 collection: {:?}", String::from_utf8_lossy(&key));
            count += 1;
        }
    }
    println!("Total CRC-21 Collections: {}", count);
}
