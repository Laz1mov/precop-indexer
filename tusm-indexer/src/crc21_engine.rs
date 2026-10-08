use bitcoin::{Transaction, TxOut, ScriptBuf, OutPoint};
use hex;
use tusm_core::crc21_v2::{VaultStateV2, match_known_cmr};
use crate::crc21_envelope::InscriptionEnvelope;

/// Identifies if a transaction is deploying a CRC-21 collection via Fast Flag
pub fn route_deploy_flag(tx: &Transaction) -> bool {
    let flag = br#"{"p":"crc-21","op":"deploy","type":"ord"}"#;
    for out in &tx.output {
        if out.script_pubkey.is_op_return() {
            let mut instructions = out.script_pubkey.instructions();
            let _ = instructions.next(); // Skip OP_RETURN
            if let Some(Ok(bitcoin::script::Instruction::PushBytes(bytes))) = instructions.next() {
                if bytes.as_bytes() == flag {
                    return true;
                }
            }
        }
    }
    false
}

/// Computes the collection ID based on Mode A parent extraction rules
pub fn compute_collection_id(tx: &Transaction) -> Option<String> {
    if tx.output.len() >= 2 {
        if tx.output[0].value == 546 && tx.output[1].value == 0 {
            return Some(format!("{}i0", tx.txid()));
        }
    }
    None
}

fn append_to_file(filename: &str, log: &serde_json::Value) {
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(filename) {
        use std::io::Write;
        let _ = writeln!(f, "{}", serde_json::to_string(log).unwrap_or_default());
    }
}

fn resolve_satoshi_destination(
    tx: &Transaction,
    prevouts: &std::collections::HashMap<bitcoin::OutPoint, tusm_core::prevout::Coin>,
    input_idx: usize,
) -> Option<usize> {
    let mut satoshi_offset: u64 = 0;
    for i in 0..input_idx {
        if let Some(prev_out) = prevouts.get(&tx.input[i].previous_output) {
            satoshi_offset += prev_out.value;
        } else {
            return None;
        }
    }
    
    let mut output_satoshi_acc: u64 = 0;
    for (out_idx, output) in tx.output.iter().enumerate() {
        output_satoshi_acc += output.value;
        if satoshi_offset < output_satoshi_acc {
            return Some(out_idx);
        }
    }
    None
}

pub fn process_tx(tx: &Transaction, db: &crate::state_db::StateDb, block_hash: &str, height: u32, prevouts: &std::collections::HashMap<bitcoin::OutPoint, tusm_core::prevout::Coin>) {
    let txid = tx.txid().to_string();

    // 1. Check for Mint (spending Parent UTXO)
    for (i, input) in tx.input.iter().enumerate() {
        let prev_out = format!("{}:{}", input.previous_output.txid, input.previous_output.vout);
        if let Some(col_json) = db.get_crc21_utxo_parent(&prev_out) {
            if let Some(dest_idx) = resolve_satoshi_destination(tx, prevouts, i) {
                let col_id = col_json["collection_id"].as_str().unwrap_or("").to_string();
                
                let new_parent_out = format!("{}:{}", txid, dest_idx);
                db.set_crc21_utxo_parent(&new_parent_out, &col_json);
                db.delete_crc21_utxo_parent(&prev_out);

                // Note: In real Ordinals, the item would follow the parent on the same satoshi if minted on it,
                // or on the next satoshi. Here we assign it to the same destination output or +1 if logic dictates.
                // Assuming item is minted on dest_idx + 1 for simplicity if not overlapping.
                let item_dest_idx = if dest_idx + 1 < tx.output.len() { dest_idx + 1 } else { dest_idx };
                let item_id = format!("{}:{}", txid, item_dest_idx);
                let item_json = serde_json::json!({
                    "collection_id": col_id,
                    "item_id": item_id,
                    "mint_txid": txid,
                });
                let new_item_out = format!("{}:{}", txid, item_dest_idx);
                db.set_crc21_utxo_item(&new_item_out, &item_json);
                db.set_crc21_item(&col_id, &item_id, &item_json);
                let log_entry = serde_json::json!({
                    "event": "Mint",
                    "collection_id": col_id,
                    "item_id": item_id,
                    "txid": txid,
                    "height": height
                });
                db.push_crc21_log(block_hash, &txid, log_entry.clone());
                append_to_file("/app/data/crc21_mints.jsonl", &log_entry);
                
                if crate::bridge_client::is_enabled() {
                    let payload = crate::bridge_client::BridgeClient::new("".into(), "".into())
                        .prepare_crc21_mint(&item_id, &col_id, &new_item_out, height, "", "", 0, serde_json::json!({}));
                    crate::bridge_client::emit(payload);
                }
                
                println!("[CRC-21] Minted Item {} for Collection {} at output {}", item_id, col_id, item_dest_idx);
            }
        }
    }

    // 2. Check for Transfer of known Items
    for (i, input) in tx.input.iter().enumerate() {
        let prev_out = format!("{}:{}", input.previous_output.txid, input.previous_output.vout);
        if let Some(item_json) = db.get_crc21_utxo_item(&prev_out) {
            if let Some(dest_idx) = resolve_satoshi_destination(tx, prevouts, i) {
                let new_out = format!("{}:{}", txid, dest_idx);
                db.set_crc21_utxo_item(&new_out, &item_json);
                db.delete_crc21_utxo_item(&prev_out);
                let log_entry = serde_json::json!({
                    "event": "Transfer",
                    "item_id": item_json["item_id"],
                    "from": prev_out,
                    "to": new_out,
                    "txid": txid,
                    "height": height
                });
                db.push_crc21_log(block_hash, &txid, log_entry.clone());
                append_to_file("/app/data/crc21_transfers.jsonl", &log_entry);

                if crate::bridge_client::is_enabled() {
                    let item_id_str = item_json["item_id"].as_str().unwrap_or("");
                    let payload = crate::bridge_client::BridgeClient::new("".into(), "".into())
                        .prepare_crc21_transfer_at(item_id_str, &prev_out, &new_out, &txid, height);
                    crate::bridge_client::emit(payload);
                }

                println!("[CRC-21] Transferred Item from {} to {}", prev_out, new_out);
            } else {
                // If None, the item is burned to fees.
                println!("[CRC-21] Item burned to fees: {}", prev_out);
                db.delete_crc21_utxo_item(&prev_out);
            }
        }
    }

    // 3. Check for Deploy
    if route_deploy_flag(tx) {
        if let Some(col_id) = compute_collection_id(tx) {
            let col_json = serde_json::json!({
                "collection_id": col_id,
                "deploy_txid": txid,
            });
            db.set_crc21_collection(&col_id, &col_json);
            
            // Output 0 is the Parent UTXO
            let parent_out = format!("{}:0", txid);
            db.set_crc21_utxo_parent(&parent_out, &col_json);
            
            db.push_crc21_log(block_hash, &txid, serde_json::json!({
                "event": "Collection Deploy",
                "collection_id": col_id,
                "txid": txid,
                "height": height
            }));
            
            if crate::bridge_client::is_enabled() {
                let payload = crate::bridge_client::BridgeClient::new("".into(), "".into())
                    .prepare_crc21_deploy_at(&col_id, &txid, &parent_out, height, serde_json::Value::Null);
                crate::bridge_client::emit(payload);
            }
            
            println!("[CRC-21] Deployed Collection {}", col_id);
        }
    }

    // 4. Check for Witness Envelope (Ordinals Mint - e.g. Vault)
    for (i, input) in tx.input.iter().enumerate() {
        if input.witness.len() < 2 {
            continue;
        }
        let script_bytes = input.witness.iter().nth(input.witness.len() - 2).unwrap();
        let script = bitcoin::ScriptBuf::from(script_bytes.to_vec());
        
        // A. Modern CRC-21 v2 Vault Envelope (Structured Ordinals with CBOR metadata)
        if let Some(env) = InscriptionEnvelope::parse_from_witness_script(&script) {
            if let Some(s_bytes) = env.get_vault_state_bytes() {
                match VaultStateV2::parse(s_bytes) {
                    Err(err) => {
                        let rej = serde_json::json!({
                            "txid": txid,
                            "input_index": i,
                            "height": height,
                            "error": err.to_string(),
                            "s_hex": hex::encode(s_bytes),
                        });
                        append_to_file("/app/data/crc21_rejections.jsonl", &rej);
                        println!("[CRC-21] Vault v2 rejected for tx {}: {}", txid, err);
                        continue;
                    }
                    Ok(vault_state) => {
                        let mut matching_vault = None;
                        for (out_i, out) in tx.output.iter().enumerate() {
                            if let Some(derived) = match_known_cmr(&vault_state, &out.script_pubkey) {
                                matching_vault = Some((out_i, derived));
                                break;
                            }
                        }

                        if let Some((out_i, derived)) = matching_vault {
                            let (name_opt, mut traits) = env.get_properties_info();
                            let item_name = name_opt.unwrap_or_else(|| format!("Koala #{}", vault_state.item_number));
                            let col_id = vault_state.collection_id();
                            let item_id = format!("{}i{}", txid, i);
                            let new_item_out = format!("{}:{}", txid, out_i);

                            if let Some(obj) = traits.as_object_mut() {
                                obj.insert("item_name".to_string(), serde_json::json!(item_name));
                            }

                            let proof_json = derived.proof_json(&vault_state);
                            let item_json = serde_json::json!({
                                "collection_id": col_id,
                                "item_id": item_id,
                                "item_name": item_name,
                                "mint_txid": txid,
                                "vault_utxo": new_item_out,
                                "body_sha256": hex::encode(env.body_sha256),
                                "proof": proof_json,
                            });

                            db.set_crc21_utxo_item(&new_item_out, &item_json);
                            db.set_crc21_item(&col_id, &item_id, &item_json);

                            let log_entry = serde_json::json!({
                                "event": "Mint Ordinals Vault v2",
                                "collection_id": col_id,
                                "item_id": item_id,
                                "item_name": item_name,
                                "txid": txid,
                                "height": height,
                                "merkle_root": hex::encode(derived.merkle_root),
                                "address": derived.address,
                            });
                            db.push_crc21_log(block_hash, &txid, log_entry.clone());
                            append_to_file("/app/data/crc21_mints.jsonl", &log_entry);

                            if crate::bridge_client::is_enabled() {
                                let payload = crate::bridge_client::BridgeClient::new("".into(), "".into())
                                    .prepare_crc21_mint_v2(
                                        &item_id,
                                        &col_id,
                                        &new_item_out,
                                        height,
                                        &derived.address,
                                        &hex::encode(derived.commitment),
                                        vault_state.csv_claim as u32,
                                        traits,
                                        proof_json.clone(),
                                        Some(hex::encode(env.body_sha256)),
                                    );
                                crate::bridge_client::emit(payload);

                                let leaves = vec![
                                    crate::bridge_client::VaultLeaf {
                                        script_hex: hex::encode(derived.leaves[0].script.as_bytes()),
                                        depth: derived.leaves[0].depth,
                                        label: derived.leaves[0].label.clone(),
                                    },
                                    crate::bridge_client::VaultLeaf {
                                        script_hex: hex::encode(derived.leaves[1].script.as_bytes()),
                                        depth: derived.leaves[1].depth,
                                        label: derived.leaves[1].label.clone(),
                                    },
                                    crate::bridge_client::VaultLeaf {
                                        script_hex: hex::encode(derived.leaves[2].script.as_bytes()),
                                        depth: derived.leaves[2].depth,
                                        label: derived.leaves[2].label.clone(),
                                    },
                                ];
                                crate::vault_watch::register(db, &new_item_out, height);
                                crate::bridge_client::emit(crate::bridge_client::builder().prepare_vault_created(
                                    "CRC21_ESCROW",
                                    &txid,
                                    out_i as u32,
                                    height,
                                    tx.output[out_i].value,
                                    &derived.address,
                                    &derived.internal_key.to_string(),
                                    Some(&vault_state.owner_key_o.to_string()),
                                    &leaves,
                                    serde_json::json!({
                                        "collection_id": col_id,
                                        "item_id": item_id,
                                        "action": "MINT_VAULT",
                                        "proof": proof_json,
                                    }),
                                ));
                            }

                            println!("[CRC-21] Verified and Minted Vault Item {} for Collection {}", item_id, col_id);
                            continue;
                        } else {
                            let rej = serde_json::json!({
                                "txid": txid,
                                "input_index": i,
                                "height": height,
                                "error": "vault_spk_not_in_outputs",
                                "state_root": hex::encode(vault_state.state_root()),
                            });
                            append_to_file("/app/data/crc21_rejections.jsonl", &rej);
                            println!("[CRC-21] Vault v2 address not found in outputs for tx {}", txid);
                            continue;
                        }
                    }
                }
            }
        }

        // B. Legacy v1 Astrolabe Ordinals Check
        let mut in_envelope = false;
        let mut expect_ord = false;
        let mut found_vault_flag_v1 = false;
        let mut vault_c = vec![];
        let mut vault_h_a = vec![];
        let mut vault_t = vec![];
        let mut vault_o = vec![];
        let mut pushes_after_flag = 0;

        let mut instructions = script.instructions();
        while let Some(Ok(instruction)) = instructions.next() {
            match instruction {
                bitcoin::script::Instruction::PushBytes(bytes) if bytes.is_empty() => {
                    if let Some(Ok(bitcoin::script::Instruction::Op(bitcoin::opcodes::all::OP_IF))) = instructions.next() {
                        expect_ord = true;
                    }
                }
                bitcoin::script::Instruction::PushBytes(bytes) if expect_ord => {
                    if bytes.as_bytes() == b"ord" {
                        in_envelope = true;
                    }
                    expect_ord = false;
                }
                bitcoin::script::Instruction::PushBytes(bytes) if in_envelope => {
                    if found_vault_flag_v1 {
                        pushes_after_flag += 1;
                        if pushes_after_flag == 1 {
                            vault_c = bytes.as_bytes().to_vec();
                        } else if pushes_after_flag == 2 {
                            vault_h_a = bytes.as_bytes().to_vec();
                        } else if pushes_after_flag == 3 {
                            vault_t = bytes.as_bytes().to_vec();
                        } else if pushes_after_flag == 4 {
                            vault_o = bytes.as_bytes().to_vec();
                        }
                    } else {
                        let hex_str = hex::encode(bytes.as_bytes());
                        if hex_str.contains("63726332315f7661756c745f7631") {
                            found_vault_flag_v1 = true;
                        }
                    }
                }
                bitcoin::script::Instruction::Op(bitcoin::opcodes::all::OP_ENDIF) if in_envelope => {
                    in_envelope = false;
                }
                _ => {}
            }
        }

        if found_vault_flag_v1 && pushes_after_flag >= 4 {
            use bitcoin::secp256k1::XOnlyPublicKey;
            if let (Ok(t_key), Ok(o_key)) = (XOnlyPublicKey::from_slice(&vault_t), XOnlyPublicKey::from_slice(&vault_o)) {
                let (vault_address, merkle_root) = tusm_core::astrolabe::compute_crc21_vault(t_key, o_key, &vault_h_a, &vault_c);
                
                let mut vault_out_idx = None;
                for (out_i, out) in tx.output.iter().enumerate() {
                    if out.script_pubkey == vault_address.script_pubkey() {
                        vault_out_idx = Some(out_i);
                        break;
                    }
                }
                
                if let Some(out_i) = vault_out_idx {
                    let col_id = hex::encode(&vault_c); 
                    let item_id = format!("{}i{}", txid, i); 
                    let item_name = hex::encode(&vault_h_a); 
                    
                    let new_item_out = format!("{}:{}", txid, out_i);
                    let item_json = serde_json::json!({
                        "collection_id": col_id,
                        "item_id": item_id,
                        "item_name": item_name,
                        "mint_txid": txid,
                        "vault_utxo": new_item_out
                    });
                    
                    db.set_crc21_utxo_item(&new_item_out, &item_json);
                    db.set_crc21_item(&col_id, &item_id, &item_json);
                    
                    let log_entry = serde_json::json!({
                        "event": "Mint Ordinals Vault",
                        "collection_id": col_id,
                        "item_id": item_id,
                        "item_name": item_name,
                        "txid": txid,
                        "height": height
                    });
                    db.push_crc21_log(block_hash, &txid, log_entry.clone());
                    append_to_file("/app/data/crc21_mints.jsonl", &log_entry);
                    
                    if crate::bridge_client::is_enabled() {
                        let payload = crate::bridge_client::BridgeClient::new("".into(), "".into())
                            .prepare_crc21_mint(
                                &item_id, 
                                &col_id, 
                                &new_item_out, 
                                height, 
                                &vault_address.to_string(), 
                                &merkle_root.to_string(), 
                                0, 
                                serde_json::json!({"item_name": item_name})
                            );
                        crate::bridge_client::emit(payload);

                        let leaves = vec![
                            crate::bridge_client::VaultLeaf { script_hex: hex::encode(tusm_core::astrolabe::build_claim_leaf(&vault_c, &vault_h_a, &t_key).as_bytes()), depth: 1, label: "CLAIM_5000".into() },
                            crate::bridge_client::VaultLeaf { script_hex: hex::encode(tusm_core::astrolabe::build_owner_leaf(&vault_c, &o_key).as_bytes()), depth: 2, label: "OWNER_RESCUE_10000".into() },
                            crate::bridge_client::VaultLeaf { script_hex: hex::encode(tusm_core::astrolabe::build_public_leaf(&vault_c).as_bytes()), depth: 2, label: "PUBLIC_RECOVERY_21000".into() },
                        ];
                        crate::vault_watch::register(db, &new_item_out, height);
                        crate::bridge_client::emit(crate::bridge_client::builder().prepare_vault_created(
                            "CRC21_ESCROW",
                            &txid,
                            out_i as u32,
                            height,
                            tx.output[out_i].value,
                            &vault_address.to_string(),
                            &tusm_core::legacy_v1_router::LegacyV1Router::nums_point().to_string(),
                            Some(&o_key.to_string()),
                            &leaves,
                            serde_json::json!({ "collection_id": col_id, "item_id": item_id, "action": "MINT_VAULT" }),
                        ));
                    }
                    println!("[CRC-21] Verified and Minted Vault Item {} for Collection {}", item_id, col_id);
                    break;
                } else {
                    println!("[CRC-21] Vault address verification failed for tx {}", txid);
                }
            }
        }

    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hashes::Hash;

    #[test]
    fn test_route_deploy_flag() {
        let flag = br#"{"p":"crc-21","op":"deploy","type":"ord"}"#;
        let push_buf = bitcoin::script::PushBytesBuf::try_from(flag.to_vec()).unwrap();
        let script = ScriptBuf::new_op_return(&push_buf);
        let tx = Transaction {
            version: 1,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![TxOut { value: 0, script_pubkey: script }],
        };
        assert!(route_deploy_flag(&tx));
    }

    #[test]
    fn test_compute_collection_id() {
        let tx = Transaction {
            version: 1,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![
                TxOut { value: 546, script_pubkey: ScriptBuf::new() },
                TxOut { value: 0, script_pubkey: ScriptBuf::new() },
            ],
        };
        let id = compute_collection_id(&tx).unwrap();
        assert_eq!(id, format!("{}i0", tx.txid()));
    }

    #[test]
    fn test_crc21_deploy_mint_transfer() {
        let db_path = "test_crc21.db";
        let _ = std::fs::remove_dir_all(db_path);
        let db = crate::state_db::StateDb::new(db_path);

        // 1. Deploy
        let flag = br#"{"p":"crc-21","op":"deploy","type":"ord"}"#;
        let push_buf = bitcoin::script::PushBytesBuf::try_from(flag.to_vec()).unwrap();
        let script = ScriptBuf::new_op_return(&push_buf);
        let tx_deploy = Transaction {
            version: 1,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![
                TxOut { value: 546, script_pubkey: ScriptBuf::new() },
                TxOut { value: 0, script_pubkey: script },
            ],
        };
        let deploy_txid = tx_deploy.txid().to_string();
        crate::crc21_engine::process_tx(&tx_deploy, &db, "block1", 1, &std::collections::HashMap::new());
        let col_id = format!("{}i0", deploy_txid);
        
        let parent_out = format!("{}:0", deploy_txid);
        assert!(db.get_crc21_utxo_parent(&parent_out).is_some(), "Parent UTXO missing");

        // 2. Mint
        let tx_mint = Transaction {
            version: 1,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![bitcoin::TxIn {
                previous_output: bitcoin::OutPoint { txid: tx_deploy.txid(), vout: 0 },
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::MAX,
                witness: bitcoin::Witness::new(),
            }],
            output: vec![
                TxOut { value: 546, script_pubkey: ScriptBuf::new() }, // Parent Output
                TxOut { value: 546, script_pubkey: ScriptBuf::new() }, // Item Output
            ],
        };
        let mint_txid = tx_mint.txid().to_string();
        crate::crc21_engine::process_tx(&tx_mint, &db, "block2", 2, &std::collections::HashMap::new());

        let new_parent_out = format!("{}:0", mint_txid);
        assert!(db.get_crc21_utxo_parent(&new_parent_out).is_some(), "New Parent UTXO missing");
        
        let item_out = format!("{}:1", mint_txid);
        assert!(db.get_crc21_utxo_item(&item_out).is_some(), "Item UTXO missing");

        // 3. Transfer
        let tx_transfer = Transaction {
            version: 1,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![bitcoin::TxIn {
                previous_output: bitcoin::OutPoint { txid: tx_mint.txid(), vout: 1 },
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::MAX,
                witness: bitcoin::Witness::new(),
            }],
            output: vec![
                TxOut { value: 546, script_pubkey: ScriptBuf::new() }, // Item Transferred
            ],
        };
        let transfer_txid = tx_transfer.txid().to_string();
        crate::crc21_engine::process_tx(&tx_transfer, &db, "block3", 3, &std::collections::HashMap::new());

        let transferred_item_out = format!("{}:0", transfer_txid);
        assert!(db.get_crc21_utxo_item(&transferred_item_out).is_some(), "Transferred Item UTXO missing");
        assert!(db.get_crc21_utxo_item(&item_out).is_none(), "Old Item UTXO should be deleted");

        let _ = std::fs::remove_dir_all(db_path);
    }

    #[test]
    fn test_koala_2305_e2e_mint() {
        let db_path = "test_koala_2305.db";
        let _ = std::fs::remove_dir_all(db_path);
        let db = crate::state_db::StateDb::new(db_path);

        let hex_str = include_str!("../tests/fixtures/koala_2305_reveal.hex").trim();
        let raw = hex::decode(hex_str).unwrap();
        let tx: Transaction = bitcoin::consensus::deserialize(&raw).unwrap();
        let txid = tx.txid().to_string();

        let prevouts = std::collections::HashMap::new();
        crate::crc21_engine::process_tx(&tx, &db, "block_969160", 969160, &prevouts);

        // Vault output is output 0
        let vault_out = format!("{}:0", txid);
        let item = db.get_crc21_utxo_item(&vault_out).expect("Koala #2305 item must be indexed in UTXO store");
        assert_eq!(item["item_name"], "Merkel Koala #2305");
        assert_eq!(item["collection_id"], "0d3e58b5ebaad24b6858e976d6a039580ecddcd3136a6fff6ec87c5fed1ab3f9i0");
        assert_eq!(item["mint_txid"], txid);
        assert_eq!(item["proof"]["scheme"], "crc21-vault-v2");
        assert_eq!(item["proof"]["address"], "bc1pmx7s6ty3th2jxa0t7yn5wn8mhrp6cg0pj4kt7fh235k7z3a4tm6qp3yqr9");
        assert_eq!(item["proof"]["state"]["item_number"], 2305);
        assert_eq!(item["proof"]["state"]["csv_claim"], 5000);
        assert_eq!(item["proof"]["state"]["csv_owner"], 10000);
        assert_eq!(item["proof"]["state"]["csv_public"], 21000);
        
        let item_id = format!("{}i0", txid);
        let col_item = db.get_crc21_item("0d3e58b5ebaad24b6858e976d6a039580ecddcd3136a6fff6ec87c5fed1ab3f9i0", &item_id)
            .expect("Koala #2305 item must be indexed in collection items store");
        assert_eq!(col_item["item_name"], "Merkel Koala #2305");

        let _ = std::fs::remove_dir_all(db_path);
    }
}

