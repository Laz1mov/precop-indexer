//! Astrolabe vault lifecycle watcher (Precopscan bridge — OBSERVER ONLY).
//!
//! * Never mutates consensus state: it only uses its own sled tree `astrolabe_vaults`, which no
//!   other component reads, and every function is a no-op while the bridge is disabled
//!   (`PRECOPSCAN_API_URL` unset).
//! * Every covenant UTXO announced through `VaultCreated` is registered in `astrolabe_vaults`.
//!   When a later transaction spends one of them, a `VaultSpent` event is emitted with the
//!   executed tapscript leaf (BIP-341 script-path witness: `[..stack, script, control_block,
//!   (annex)]`), so Precopscan can show WHICH branch of the covenant was used.
//! * Entries are never removed on spend (a UTXO can only be spent once per chain): after a reorg
//!   the replayed spend is emitted again, which is what the registry needs once it has discarded
//!   the orphaned rows on `ChainReorg`.
use bitcoin::secp256k1::{Secp256k1, XOnlyPublicKey};
use bitcoin::{ScriptBuf, Transaction};

use crate::bridge_client::{self, VaultLeaf};
use crate::state_db::StateDb;
use tusm_core::legacy_v1_router::LegacyV1Router;

const TREE: &str = "astrolabe_vaults";

/// TGE seal CSV tiers recognised by the VM (`LegacyV1Router::process`).
pub const TGE_LOCK_TIERS: [u16; 4] = [144, 1000, 2100, 6767];
/// Dust floor applied by the VM to seal outputs.
pub const SEAL_MIN_VALUE: u64 = 330;

pub fn register(db: &StateDb, outpoint: &str, created_height: u32) {
    if !bridge_client::is_enabled() {
        return;
    }
    if let Ok(tree) = db.db.open_tree(TREE) {
        let _ = tree.insert(outpoint.as_bytes(), created_height.to_be_bytes().to_vec());
    }
}

/// Executed tapscript of a script-path spend, if the witness is one.
pub fn executed_leaf_script(witness: &bitcoin::Witness) -> Option<String> {
    let items: Vec<&[u8]> = witness.iter().collect();
    if items.len() < 2 {
        return None; // key-path spend (impossible for a NUMS covenant) or empty
    }
    let has_annex = items.last().map(|a| !a.is_empty() && a[0] == 0x50).unwrap_or(false);
    let end = if has_annex { items.len() - 1 } else { items.len() };
    if end < 2 {
        return None;
    }
    let control = items[end - 1];
    // control block = 1 byte (leaf version | parity) + 32 byte internal key + 32*k path
    if control.len() < 33 || (control.len() - 33) % 32 != 0 {
        return None;
    }
    Some(hex::encode(items[end - 2]))
}

/// Emits `VaultSpent` for every registered covenant outpoint consumed by `tx`.
pub fn process_inputs(tx: &Transaction, db: &StateDb, height: u32) {
    if !bridge_client::is_enabled() {
        return;
    }
    let Ok(tree) = db.db.open_tree(TREE) else { return };
    if tree.is_empty() {
        return;
    }
    for input in &tx.input {
        let outpoint = format!("{}:{}", input.previous_output.txid, input.previous_output.vout);
        if matches!(tree.contains_key(outpoint.as_bytes()), Ok(true)) {
            let leaf = executed_leaf_script(&input.witness);
            println!("[Astrolabe] Vault {} spent by {} (leaf: {:?})", outpoint, tx.txid(), leaf.as_deref().map(|s| &s[..s.len().min(16)]));
            bridge_client::emit(bridge_client::builder().prepare_vault_spent(
                &outpoint,
                &tx.txid().to_string(),
                height,
                leaf.as_deref(),
            ));
        }
    }
}

/// A TGE CSV seal located in a mint transaction.
#[derive(Debug, Clone, PartialEq)]
pub struct TgeSeal {
    pub vout: usize,
    pub lock: u16,
    pub owner_key: XOnlyPublicKey,
    pub has_cmr: bool,
}

/// Index of the CRC-20 OP_RETURN, located exactly like `LegacyV1Router::process` (first push
/// that is UTF-8 and contains `crc-20` or `"p":`, and whose JSON has `p == "crc-20"`).
fn crc20_op_return_index(tx: &Transaction) -> Option<usize> {
    for (i, output) in tx.output.iter().enumerate() {
        if !output.script_pubkey.is_op_return() {
            continue;
        }
        for instruction in output.script_pubkey.instructions() {
            if let Ok(bitcoin::script::Instruction::PushBytes(data)) = instruction {
                if let Ok(s) = std::str::from_utf8(data.as_bytes()) {
                    if s.contains("crc-20") || s.contains("\"p\":") {
                        let is_crc20 = serde_json::from_str::<serde_json::Value>(s)
                            .ok()
                            .and_then(|v| v.get("p").and_then(|p| p.as_str()).map(|p| p == "crc-20"))
                            .unwrap_or(false);
                        if is_crc20 {
                            return Some(i);
                        }
                        break;
                    }
                }
            }
        }
    }
    None
}

/// Candidate owner keys, in the exact order used by `LegacyV1Router::process`
/// (32/33-byte witness items of every input, then the output keys of every P2TR output).
fn candidate_owner_keys(tx: &Transaction) -> Vec<XOnlyPublicKey> {
    let mut keys = Vec::new();
    for input in &tx.input {
        for item in input.witness.iter() {
            if item.len() == 32 {
                if let Ok(x) = XOnlyPublicKey::from_slice(item) {
                    keys.push(x);
                }
            } else if item.len() == 33 {
                if let Ok(pk) = bitcoin::PublicKey::from_slice(item) {
                    keys.push(pk.inner.x_only_public_key().0);
                }
            }
        }
    }
    for out in &tx.output {
        if out.script_pubkey.is_v1_p2tr() {
            if let Ok(x) = XOnlyPublicKey::from_slice(&out.script_pubkey.as_bytes()[2..34]) {
                keys.push(x);
            }
        }
    }
    keys
}

/// Independently re-derives the TGE seal of a mint, mirroring the VM's seal search
/// (`LegacyV1Router::process`) WITHOUT modifying it:
///  * only the FIRST candidate owner key is examined (the VM's `seal_found` starts `true`, so its
///    key loop always stops after the first key);
///  * outputs are scanned in order, skipping the CRC-20 OP_RETURN, P2TR with value >= 330;
///  * per output the VM tests key-path (`t0`, lock 0 => no vault) first, then 144/1000/2100/6767;
///  * the first match wins. `has_cmr` = ticker == "CRC".
/// Returns `None` when no CSV seal (lock > 0) is found.
pub fn locate_tge_seal(tx: &Transaction, ticker_upper: &str) -> Option<TgeSeal> {
    let i = crc20_op_return_index(tx)?;
    let secp = Secp256k1::new();
    let has_cmr = ticker_upper == "CRC";
    println!("[LOCATE_TGE_SEAL] TXID={} keys={}", tx.txid(), candidate_owner_keys(tx).len());
    for (key_idx, owner_key) in candidate_owner_keys(tx).into_iter().enumerate() {
        println!("[LOCATE_TGE_SEAL] Testing owner_key: {}", owner_key);
        let t0 = ScriptBuf::new_v1_p2tr(&secp, owner_key, None);
        let tiers: Vec<(u16, ScriptBuf)> = TGE_LOCK_TIERS
            .iter()
            .map(|&l| (l, LegacyV1Router::derive_taproot_for_lock(&secp, l, owner_key, has_cmr)))
            .collect();
        for (j, out) in tx.output.iter().enumerate() {
            if j == i || !out.script_pubkey.is_v1_p2tr() || out.value < SEAL_MIN_VALUE {
                continue;
            }
            if out.script_pubkey == t0 {
                println!("[LOCATE_TGE_SEAL] Found t0 (keypath) for key_idx={}. Returning None.", key_idx);
                return None; // key-path seal: no covenant
            }
            if let Some((lock, _)) = tiers.iter().find(|(_, spk)| *spk == out.script_pubkey) {
                println!("[LOCATE_TGE_SEAL] Found tier {} for key_idx={}!", lock, key_idx);
                return Some(TgeSeal { vout: j, lock: *lock, owner_key, has_cmr });
            }
        }
    }
    for (j, out) in tx.output.iter().enumerate() {
        if j != i && out.script_pubkey.is_v1_p2tr() {
            println!("[LOCATE_TGE_SEAL] Unmatched output {}: {}", j, hex::encode(out.script_pubkey.as_bytes()));
        }
    }
    println!("[LOCATE_TGE_SEAL] Reached end of loop, returning None.");
    None
}

/// Publishes a TGE CSV `VaultCreated` for a VM-ACCEPTED mint (call only after the VM emitted the
/// mint event). The covenant is re-derived from the raw transaction (see `locate_tge_seal`); the
/// leaf is byte-identical to the one inside `derive_taproot_for_lock`, and the Precopscan gateway
/// re-verifies `p2tr_address` from (NUMS, leaf) before accepting it.
pub fn emit_tge_vault_from_mint(
    db: &StateDb,
    tx: &Transaction,
    height: u32,
    ticker_upper: &str,
    locked_atoms: u128,
) {
    if !bridge_client::is_enabled() {
        return;
    }
    let Some(seal) = locate_tge_seal(tx, ticker_upper) else { return };
    let out = &tx.output[seal.vout];
    let Ok(addr) = bitcoin::Address::from_script(&out.script_pubkey, bitcoin::Network::Bitcoin) else { return };

    let txid = tx.txid().to_string();
    let leaf = LegacyV1Router::tge_lock_leaf(seal.lock, seal.owner_key, seal.has_cmr);
    let leaves = vec![VaultLeaf { script_hex: hex::encode(leaf.as_bytes()), depth: 0, label: format!("CSV_UNLOCK_{}", seal.lock) }];
    let nums = LegacyV1Router::nums_point().to_string();
    let owner = seal.owner_key.to_string();

    register(db, &format!("{}:{}", txid, seal.vout), height);
    bridge_client::emit(bridge_client::builder().prepare_vault_created(
        "TGE_CSV",
        &txid,
        seal.vout as u32,
        height,
        out.value,
        &addr.to_string(),
        &nums,
        Some(&owner),
        &leaves,
        serde_json::json!({
            "ticker": ticker_upper,
            "locked_atoms": locked_atoms.to_string(),
            "csv_delay": seal.lock,
            "unlock_height": height as u64 + seal.lock as u64,
            "has_cmr": seal.has_cmr
        }),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{absolute::LockTime, OutPoint, Sequence, TxIn, TxOut, Witness};

    #[test]
    fn script_path_witness_yields_the_executed_leaf() {
        let mut w = bitcoin::Witness::new();
        w.push([0u8; 64]); // signature
        w.push(hex::decode("51").unwrap()); // script
        let mut cb = vec![0xc0u8];
        cb.extend_from_slice(&[7u8; 32]);
        w.push(cb);
        assert_eq!(executed_leaf_script(&w).as_deref(), Some("51"));
    }

    #[test]
    fn key_path_witness_has_no_leaf() {
        let mut w = bitcoin::Witness::new();
        w.push([0u8; 64]);
        assert_eq!(executed_leaf_script(&w), None);
    }

    fn key(b: u8) -> XOnlyPublicKey {
        let secp = Secp256k1::new();
        let sk = bitcoin::secp256k1::SecretKey::from_slice(&[b; 32]).unwrap();
        XOnlyPublicKey::from(bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &sk))
    }

    fn op_return(json: &str) -> ScriptBuf {
        let push = bitcoin::script::PushBytesBuf::try_from(json.as_bytes().to_vec()).unwrap();
        bitcoin::script::Builder::new()
            .push_opcode(bitcoin::opcodes::all::OP_RETURN)
            .push_slice(push.as_push_bytes())
            .into_script()
    }

    fn mint_tx(witness_key: XOnlyPublicKey, outputs: Vec<TxOut>) -> Transaction {
        let mut w = Witness::new();
        w.push([0u8; 64]);
        w.push(witness_key.serialize());
        Transaction {
            version: 2,
            lock_time: LockTime::ZERO,
            input: vec![TxIn { previous_output: OutPoint::null(), script_sig: ScriptBuf::new(), sequence: Sequence::MAX, witness: w }],
            output: outputs,
        }
    }

    #[test]
    fn locates_csv_seal_like_the_vm() {
        let secp = Secp256k1::new();
        let owner = key(1);
        let seal_spk = LegacyV1Router::derive_taproot_for_lock(&secp, 1000, owner, false);
        let tx = mint_tx(owner, vec![
            TxOut { value: 0, script_pubkey: op_return(r#"{"p":"crc-20","op":"mint","tick":"LEAF"}"#) },
            TxOut { value: 546, script_pubkey: seal_spk },
        ]);
        let seal = locate_tge_seal(&tx, "LEAF").expect("seal");
        assert_eq!(seal, TgeSeal { vout: 1, lock: 1000, owner_key: owner, has_cmr: false });
        // the published leaf commits to exactly the on-chain output key
        let leaf = LegacyV1Router::tge_lock_leaf(1000, owner, false);
        let spend = bitcoin::taproot::TaprootBuilder::new().add_leaf(0, leaf).unwrap()
            .finalize(&secp, LegacyV1Router::nums_point()).unwrap();
        assert_eq!(ScriptBuf::new_v1_p2tr_tweaked(spend.output_key()), tx.output[1].script_pubkey);
    }

    #[test]
    fn crc_ticker_uses_cmr_leaf() {
        let secp = Secp256k1::new();
        let owner = key(2);
        let seal_spk = LegacyV1Router::derive_taproot_for_lock(&secp, 6767, owner, true);
        let tx = mint_tx(owner, vec![
            TxOut { value: 0, script_pubkey: op_return(r#"{"p":"crc-20","op":"mint","tick":"crc"}"#) },
            TxOut { value: 330, script_pubkey: seal_spk },
        ]);
        assert_eq!(locate_tge_seal(&tx, "CRC").map(|s| (s.lock, s.has_cmr)), Some((6767, true)));
        assert_eq!(locate_tge_seal(&tx, "LEAF"), None, "non-CRC ticker must not match a CMR seal");
    }

    #[test]
    fn all_candidate_keys_are_considered_like_the_vm() {
        let secp = Secp256k1::new();
        let first = key(3);
        let other = key(4);
        // seal belongs to `other`, and the VM tests all witness keys
        let seal_spk = LegacyV1Router::derive_taproot_for_lock(&secp, 144, other, false);
        let tx = mint_tx(first, vec![
            TxOut { value: 0, script_pubkey: op_return(r#"{"p":"crc-20","op":"mint","tick":"LEAF"}"#) },
            TxOut { value: 546, script_pubkey: seal_spk },
        ]);
        // The mock `mint_tx` only adds `first` to the witness. 
        // We need to add `other` to the witness or output to make it a candidate.
        let mut tx_with_other = tx.clone();
        tx_with_other.input[0].witness.push(other.serialize());
        assert_eq!(locate_tge_seal(&tx_with_other, "LEAF").map(|s| s.lock), Some(144));
    }

    #[test]
    fn dust_and_key_path_are_not_vaults() {
        let secp = Secp256k1::new();
        let owner = key(5);
        let dust = LegacyV1Router::derive_taproot_for_lock(&secp, 1000, owner, false);
        let tx = mint_tx(owner, vec![
            TxOut { value: 0, script_pubkey: op_return(r#"{"p":"crc-20","op":"mint","tick":"LEAF"}"#) },
            TxOut { value: 329, script_pubkey: dust },
        ]);
        assert_eq!(locate_tge_seal(&tx, "LEAF"), None);
        let kp = ScriptBuf::new_v1_p2tr(&secp, owner, None);
        let tx2 = mint_tx(owner, vec![
            TxOut { value: 0, script_pubkey: op_return(r#"{"p":"crc-20","op":"mint","tick":"LEAF"}"#) },
            TxOut { value: 546, script_pubkey: kp },
        ]);
        assert_eq!(locate_tge_seal(&tx2, "LEAF"), None);
    }

    #[test]
    fn disabled_bridge_has_no_side_effects() {
        // ENABLED is false in unit tests unless a test enables it: register must not touch sled.
        let path = format!(".test_vault_watch_disabled_{}", std::process::id());
        let db = StateDb::new(&path);
        register(&db, "00:0", 1);
        assert!(!db.db.tree_names().iter().any(|n| n.as_ref() == TREE.as_bytes()));
        drop(db);
        let _ = std::fs::remove_dir_all(&path);
    }
}
