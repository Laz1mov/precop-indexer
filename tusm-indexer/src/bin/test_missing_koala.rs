use bitcoin::Transaction;
use precop_tusm_indexer::crc21_engine;
use precop_tusm_indexer::state_db::StateDb;

async fn fetch_tx(txid: &str) -> Transaction {
    let url = format!("https://mempool.space/api/tx/{}/hex", txid);
    let hex = reqwest::get(&url).await.unwrap().text().await.unwrap();
    let bytes = hex::decode(hex).unwrap();
    bitcoin::consensus::deserialize(&bytes).unwrap()
}

#[tokio::main]
async fn main() {
    let db_path = "test_missing_koala.db";
    let _ = std::fs::remove_dir_all(db_path);
    let db = StateDb::new(db_path);

    let txid = "b76b559b9341b819d6591ee70c03b896c52d46b6e5439fe589ef43afa21acb30";
    let tx = fetch_tx(txid).await;

    let empty_prevouts = std::collections::HashMap::new();
    println!("Processing TX through crc21_engine...");
    crc21_engine::process_tx(&tx, &db, "hash_missing", 969179, &empty_prevouts);

    let logs = db.get_crc21_logs();
    if logs.is_empty() {
        println!("❌ FAILED: No CRC-21 events registered for {}", txid);
    } else {
        println!("✅ SUCCESS: CRC-21 events registered:");
        for log in logs {
            println!("  {}", log);
        }
    }
}
