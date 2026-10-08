use serde::Deserialize;
use std::fs::File;
use std::io::{BufRead, BufReader};
use precop_tusm_indexer::state_db::{StateDb, SigmaLeaf};
use bitcoin::OutPoint;

#[derive(Deserialize)]
struct BalanceEntry {
    address: String,
    atoms: String,
    tick: String,
}

#[derive(Deserialize)]
struct TokenParameters {
    #[serde(rename = "ico:point_limit")]
    point_limit: String,
    #[serde(rename = "ico:weight:BTC")]
    weight_btc: String,
    #[serde(rename = "ico:weight:LEAF")]
    weight_leaf: String,
    #[serde(rename = "ico:weight:ORDI")]
    weight_ordi: String,
}

#[derive(Deserialize)]
struct TokenEntry {
    current_supply: String,
    max_supply: String,
    parameters: TokenParameters,
    tick: String,
}

fn main() {
    let db = StateDb::new("snapshot_db");

    let snapshot_height: u32 = 968995;
    let block_hash = "0000000000000000000000000000000000000000000000000000000000000000";

    let mut sigmas = Vec::new();
    let mut balances = Vec::new();

    let snapshot_dir = std::env::args().nth(1).unwrap_or_else(|| "snapshot/db/decoded".to_string());
    let tokens_path = format!("{}/tokens.jsonl", snapshot_dir);
    let balances_path = format!("{}/balances.jsonl", snapshot_dir);

    // 1. Import tokens
    let tokens_file = File::open(&tokens_path).unwrap_or_else(|e| {
        panic!("Failed to open {}: {}", tokens_path, e);
    });
    let reader = BufReader::new(tokens_file);
    for line in reader.lines() {
        let line = line.unwrap();
        if line.trim().is_empty() { continue; }
        let entry: TokenEntry = serde_json::from_str(&line).unwrap();
        if entry.tick == "LEAF" {
            let point_limit: u128 = entry.parameters.point_limit.parse().unwrap();
            let scale_leaf = 100_000_000u128; // From ico:scale:LEAF
            let sigma = SigmaLeaf {
                supply: entry.current_supply.parse().unwrap(),
                max_supply: entry.max_supply.parse().unwrap(),
                limit_per_mint: point_limit * scale_leaf,
                btc_rate: entry.parameters.weight_btc.parse().unwrap(),
                dog_rate: 0,
                leaf_rate: entry.parameters.weight_leaf.parse().unwrap(),
                ordi_rate: entry.parameters.weight_ordi.parse().unwrap(),
                anchor: OutPoint::null(),
                height: 967485, // genesis
                version: 3,
                halted: false,
                tge_root: [0u8; 32], consumed_points: 0,
            };
            sigmas.push((entry.tick.clone(), sigma, None));
            println!("Imported token {}", entry.tick);
        }
    }

    // 2. Import balances
    let balances_file = File::open(&balances_path).unwrap_or_else(|e| {
        panic!("Failed to open {}: {}", balances_path, e);
    });
    let reader = BufReader::new(balances_file);
    let mut count = 0;
    for line in reader.lines() {
        let line = line.unwrap();
        if line.trim().is_empty() { continue; }
        let entry: BalanceEntry = serde_json::from_str(&line).unwrap();
        let balance: u128 = entry.atoms.parse().unwrap();
        if balance > 0 {
            // (ticker, address, prev_balance, new_balance)
            balances.push((entry.tick.clone(), entry.address.clone(), 0, balance));
            count += 1;
        }
    }
    println!("Imported {} non-zero balances", count);

    // 3. Commit
    db.commit_block_batch(snapshot_height, block_hash, &sigmas, &balances);
    println!("Committed snapshot state at block {}", snapshot_height);
}
