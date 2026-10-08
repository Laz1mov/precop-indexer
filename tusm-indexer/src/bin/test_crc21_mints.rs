use bitcoin::Transaction;
use std::str::FromStr;
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
    let db_path = "test_koala.db";
    let _ = std::fs::remove_dir_all(db_path);
    let db = StateDb::new(db_path);

    println!("Fetching TX1 (Merkel Koala #1 Reveal): 8b7e755ac5b133c904de1604ef402b91ee6fea5e038aafe688e3316b4ef2a259");
    let tx1 = fetch_tx("8b7e755ac5b133c904de1604ef402b91ee6fea5e038aafe688e3316b4ef2a259").await;
    
    println!("Fetching TX2 (Merkel Koala #2 Reveal): 116f5736d68aef805656c2fb3b2740ebef9cd9a428bd5f62611588550f27a28a");
    let tx2 = fetch_tx("116f5736d68aef805656c2fb3b2740ebef9cd9a428bd5f62611588550f27a28a").await;

    let empty_prevouts = std::collections::HashMap::new();
    println!("Processing TX1 through crc21_engine...");
    crc21_engine::process_tx(&tx1, &db, "hash1", 969160, &empty_prevouts);
    
    println!("Processing TX2 through crc21_engine...");
    crc21_engine::process_tx(&tx2, &db, "hash2", 969164, &empty_prevouts);

    let logs = db.get_crc21_logs();
    if logs.is_empty() {
        println!("❌ FAILURE: No CRC-21 events were registered! The engine completely ignored the mints.");
    } else {
        println!("✅ SUCCESS: CRC-21 events registered:");
        for log in logs {
            println!("  {}", log);
        }
    }
}
