use precop_tusm_indexer::state_db::StateDb;

fn main() {
    let db_path = std::env::args().nth(1).expect("Usage: ... <db_path> <ticker>");
    let ticker = std::env::args().nth(2).expect("Missing ticker");
    let db = StateDb::new(&db_path);
    let balances = db.get_holders(&ticker);
    let mut balances = db.get_holders(&ticker).into_iter().collect::<Vec<_>>();
    balances.sort_by(|a, b| b.1.cmp(&a.1));
    let top_5 = balances.into_iter().take(5).collect::<Vec<_>>();
    println!("TOP 5 HOLDERS FOR {}:", ticker);
    for (i, (addr, bal)) in top_5.iter().enumerate() {
        println!("{}. {}: {} atoms ({} tokens)", i + 1, addr, bal, *bal as f64 / 100_000_000.0);
    }
}
