use precop_tusm_indexer::state_db::StateDb;

fn main() {
    let db = StateDb::new(".test_mainnet_db");
    
    println!("--- FORMAL INDEXER RESULTS (TOKENS) ---");
    let tokens = db.get_all_tokens();
    for (ticker, token) in tokens {
        println!("Token: {}", ticker);
        println!("  Total Supply: {}", token.supply);
        println!("  Max Supply: {}", token.max_supply);
    }
    
    println!("\n--- FORMAL INDEXER RESULTS (HOLDERS OF LEAF) ---");
    let holders = db.get_holders("LEAF");
    for (addr, amt) in holders {
        println!("Holder: {} | Balance: {}", addr, amt);
    }
}
