fn main() {
    let db = precop_tusm_indexer::state_db::StateDb::new("snapshot_db");
    if let Some(sigma) = db.get_sigma("LEAF") {
        println!("SNAPSHOT SUPPLY: {}", sigma.supply);
    } else {
        println!("SNAPSHOT SUPPLY: Not found");
    }
    
    let holders = db.get_holders_count("LEAF");
    println!("SNAPSHOT HOLDERS: {}", holders);
    
    let height = db.get_last_scanned_height();
    println!("SNAPSHOT HEIGHT: {}", height);
}
