use precop_tusm_indexer::crc21_envelope::InscriptionEnvelope;
use tusm_core::crc21_v2::{VaultStateV2, match_known_cmr};
use bitcoin::Transaction;
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    let client = reqwest::Client::new();
    let res = client.get("https://opengarden.co/api/koala/items")
        .header("User-Agent", "Mozilla/5.0")
        .send().await.unwrap()
        .json::<serde_json::Value>().await.unwrap();

    let items = res["items"].as_array().unwrap();
    println!("Total OpenGarden items to verify: {}", items.len());

    // We test a representative sample of missing and common items
    let mut verified = 0;
    let mut failed = 0;

    // Test first 20 items and last 20 items
    let mut to_test = Vec::new();
    for it in &items[..15] {
        to_test.push(it);
    }
    for it in &items[items.len()-15..] {
        to_test.push(it);
    }

    for it in to_test {
        let num = it["number"].as_u64().unwrap();
        let txid = it["reveal_txid"].as_str().unwrap();
        let hex = client.get(&format!("https://mempool.space/api/tx/{}/hex", txid))
            .send().await.unwrap()
            .text().await.unwrap();
        let bytes = hex::decode(hex).unwrap();
        let tx: Transaction = bitcoin::consensus::deserialize(&bytes).unwrap();

        let mut matched = false;
        for input in &tx.input {
            if input.witness.len() < 2 { continue; }
            let script_bytes = input.witness.iter().nth(input.witness.len() - 2).unwrap();
            let script = bitcoin::ScriptBuf::from(script_bytes.to_vec());
            if let Some(env) = InscriptionEnvelope::parse_from_witness_script(&script) {
                if let Some(sb) = env.get_vault_state_bytes() {
                    if let Ok(vs) = VaultStateV2::parse(sb) {
                        for out in &tx.output {
                            if let Some(d) = match_known_cmr(&vs, &out.script_pubkey) {
                                matched = true;
                                break;
                            }
                        }
                    }
                }
            }
        }
        if matched {
            verified += 1;
            println!("  ✅ Koala #{} (tx {}) verified!", num, &txid[..12]);
        } else {
            failed += 1;
            println!("  ❌ Koala #{} (tx {}) FAILED!", num, &txid[..12]);
        }
    }

    println!("\nSample verification complete: {} verified, {} failed", verified, failed);
}
