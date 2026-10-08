use bitcoin::hashes::Hash;
use bitcoin::OutPoint;
use sled::Db;
use std::convert::TryInto;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SigmaLeaf {
    pub supply: u128,
    pub max_supply: u128,
    pub limit_per_mint: u128,
    pub btc_rate: u128,
    pub dog_rate: u128,
    pub leaf_rate: u128,
    pub ordi_rate: u128,
    pub anchor: OutPoint,
    pub height: u32,
    pub version: u8,
    pub halted: bool,
    pub tge_root: [u8; 32],
    pub consumed_points: u128,
}

impl SigmaLeaf {
    /// Strict Canonical Serialization (202 bytes)
    pub fn to_canonical_bytes(&self) -> [u8; 202] {
        let mut buf = [0u8; 202];
        buf[0..16].copy_from_slice(&self.supply.to_le_bytes());
        buf[16..32].copy_from_slice(&self.max_supply.to_le_bytes());
        buf[32..48].copy_from_slice(&self.limit_per_mint.to_le_bytes());
        buf[48..64].copy_from_slice(&self.btc_rate.to_le_bytes());
        buf[64..80].copy_from_slice(&self.dog_rate.to_le_bytes());
        buf[80..96].copy_from_slice(&self.leaf_rate.to_le_bytes());
        buf[96..112].copy_from_slice(&self.ordi_rate.to_le_bytes());
        buf[112..144].copy_from_slice(self.anchor.txid.as_ref());
        buf[144..148].copy_from_slice(&self.anchor.vout.to_le_bytes());
        buf[148..152].copy_from_slice(&self.height.to_le_bytes());
        buf[152] = self.version;
        buf[153] = if self.halted { 1 } else { 0 };
        buf[154..186].copy_from_slice(&self.tge_root);
        buf[186..202].copy_from_slice(&self.consumed_points.to_le_bytes());
        buf
    }

    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != 202 {
            return Err("Invalid canonical length");
        }

        let supply = u128::from_le_bytes(bytes[0..16].try_into().unwrap());
        let max_supply = u128::from_le_bytes(bytes[16..32].try_into().unwrap());
        let limit_per_mint = u128::from_le_bytes(bytes[32..48].try_into().unwrap());
        let btc_rate = u128::from_le_bytes(bytes[48..64].try_into().unwrap());
        let dog_rate = u128::from_le_bytes(bytes[64..80].try_into().unwrap());
        let leaf_rate = u128::from_le_bytes(bytes[80..96].try_into().unwrap());
        let ordi_rate = u128::from_le_bytes(bytes[96..112].try_into().unwrap());
        let txid = bitcoin::Txid::from_slice(&bytes[112..144]).unwrap();
        let vout = u32::from_le_bytes(bytes[144..148].try_into().unwrap());
        let height = u32::from_le_bytes(bytes[148..152].try_into().unwrap());
        let version = bytes[152];
        let halted = bytes[153] != 0;
        let mut tge_root = [0u8; 32];
        tge_root.copy_from_slice(&bytes[154..186]);
        let consumed_points = u128::from_le_bytes(bytes[186..202].try_into().unwrap());

        Ok(Self {
            supply,
            max_supply,
            limit_per_mint,
            btc_rate,
            dog_rate,
            leaf_rate,
            ordi_rate,
            anchor: OutPoint::new(txid, vout),
            height,
            version,
            halted,
            tge_root,
            consumed_points,
        })
    }
}

#[derive(Clone)]
pub struct StateDb {
    pub db: Db,
}

impl StateDb {
    pub fn new(path: &str) -> Self {
        Self {
            db: sled::open(path).expect("Failed to open Sled DB"),
        }
    }

    pub fn flush(&self) {
        let _ = self.db.flush();
    }

    pub fn get_sigma(&self, ticker: &str) -> Option<SigmaLeaf> {
        let key = format!("sigma:leaf:{}", ticker);
        let data = self.db.get(key).unwrap()?;
        Some(SigmaLeaf::from_canonical_bytes(&data).unwrap())
    }

    pub fn get_last_scanned_height(&self) -> u32 {
        if let Ok(Some(bytes)) = self.db.get(b"__LAST_SCANNED_HEIGHT__") {
            if bytes.len() == 4 {
                let mut arr = [0u8; 4];
                arr.copy_from_slice(&bytes);
                return u32::from_be_bytes(arr);
            }
        }
        // Epoch 0 Genesis block (or whatever default the user wants, e.g. 0 for full sync)
        0
    }

    pub fn set_last_scanned_height(&self, height: u32) {
        let _ = self.db.insert(b"__LAST_SCANNED_HEIGHT__", height.to_be_bytes().to_vec());
    }

    // CRC-21 Collections & Items State Store
    pub fn get_crc21_collection(&self, id: &str) -> Option<serde_json::Value> {
        let key = format!("crc21:col:{}", id);
        let data = self.db.get(key).unwrap()?;
        serde_json::from_slice(&data).ok()
    }

    pub fn get_crc21_collections(&self) -> Vec<String> {
        let mut collections = Vec::new();
        for res in self.db.scan_prefix(b"crc21:col:") {
            if let Ok((k, _)) = res {
                if let Ok(k_str) = String::from_utf8(k.to_vec()) {
                    if let Some(id) = k_str.strip_prefix("crc21:col:") {
                        collections.push(id.to_string());
                    }
                }
            }
        }
        collections
    }

    pub fn set_crc21_collection(&self, id: &str, data: &serde_json::Value) {
        let key = format!("crc21:col:{}", id);
        let _ = self.db.insert(key, serde_json::to_vec(data).unwrap());
    }

    pub fn get_crc21_item(&self, collection_id: &str, item_id: &str) -> Option<serde_json::Value> {
        let key = format!("crc21:item:{}:{}", collection_id, item_id);
        let data = self.db.get(key).unwrap()?;
        serde_json::from_slice(&data).ok()
    }

    pub fn set_crc21_item(&self, collection_id: &str, item_id: &str, data: &serde_json::Value) {
        let key = format!("crc21:item:{}:{}", collection_id, item_id);
        let _ = self.db.insert(key, serde_json::to_vec(data).unwrap());
    }

    pub fn get_crc21_utxo_parent(&self, outpoint: &str) -> Option<serde_json::Value> {
        let key = format!("crc21:uparent:{}", outpoint);
        let data = self.db.get(key).unwrap()?;
        serde_json::from_slice(&data).ok()
    }
    
    pub fn set_crc21_utxo_parent(&self, outpoint: &str, data: &serde_json::Value) {
        let key = format!("crc21:uparent:{}", outpoint);
        let _ = self.db.insert(key, serde_json::to_vec(data).unwrap());
    }
    
    pub fn delete_crc21_utxo_parent(&self, outpoint: &str) {
        let key = format!("crc21:uparent:{}", outpoint);
        let _ = self.db.remove(key);
    }
    
    pub fn get_crc21_utxo_item(&self, outpoint: &str) -> Option<serde_json::Value> {
        let key = format!("crc21:uitem:{}", outpoint);
        let data = self.db.get(key).unwrap()?;
        serde_json::from_slice(&data).ok()
    }

    pub fn get_crc21_item_by_id(&self, item_id: &str) -> Option<(String, serde_json::Value)> {
        let idx_key = format!("crc21:item_idx:{}", item_id);
        if let Ok(Some(outpoint_bytes)) = self.db.get(&idx_key) {
            let outpoint = String::from_utf8_lossy(&outpoint_bytes).to_string();
            if let Some(item) = self.get_crc21_utxo_item(&outpoint) {
                return Some((outpoint, item));
            }
        }
        // Fallback for legacy items before index was populated
        for res in self.db.scan_prefix(b"crc21:uitem:") {
            if let Ok((k, v)) = res {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&v) {
                    if json["item_id"].as_str() == Some(item_id) {
                        let k_str = String::from_utf8_lossy(&k);
                        let outpoint = k_str.strip_prefix("crc21:uitem:").unwrap_or("").to_string();
                        return Some((outpoint, json));
                    }
                }
            }
        }
        None
    }
    
    pub fn set_crc21_utxo_item(&self, outpoint: &str, data: &serde_json::Value) {
        let key = format!("crc21:uitem:{}", outpoint);
        let _ = self.db.insert(key, serde_json::to_vec(data).unwrap());
        if let Some(item_id) = data.get("item_id").and_then(|v| v.as_str()) {
            let idx_key = format!("crc21:item_idx:{}", item_id);
            let _ = self.db.insert(idx_key, outpoint.as_bytes());
        }
    }
    
    pub fn delete_crc21_utxo_item(&self, outpoint: &str) {
        if let Some(data) = self.get_crc21_utxo_item(outpoint) {
            if let Some(item_id) = data.get("item_id").and_then(|v| v.as_str()) {
                let idx_key = format!("crc21:item_idx:{}", item_id);
                let _ = self.db.remove(idx_key);
            }
        }
        let key = format!("crc21:uitem:{}", outpoint);
        let _ = self.db.remove(key);
    }

    pub fn get_block_hash(&self, height: u32) -> Option<String> {
        let key = format!("block_hash:{}", height);
        let data = self.db.get(key).unwrap()?;
        Some(String::from_utf8(data.to_vec()).unwrap())
    }

    pub fn get_balance(&self, ticker: &str, address: &str) -> u128 {
        let key = format!("bal:{}:{}", ticker, address);
        if let Ok(Some(data)) = self.db.get(key) {
            let mut arr = [0u8; 16];
            arr.copy_from_slice(&data);
            u128::from_le_bytes(arr)
        } else {
            0
        }
    }

    pub fn get_total_balance(&self, ticker: &str) -> u128 {
        let prefix = format!("bal:{}:", ticker);
        let mut total = 0u128;
        for res in self.db.scan_prefix(prefix) {
            if let Ok((_key, val)) = res {
                if val.len() == 16 {
                    let mut arr = [0u8; 16];
                    arr.copy_from_slice(&val);
                    total = total.saturating_add(u128::from_le_bytes(arr));
                }
            }
        }
        total
    }

    pub fn get_holders_count(&self, ticker: &str) -> usize {
        let prefix = format!("bal:{}:", ticker);
        self.db.scan_prefix(prefix).filter_map(|res| res.ok()).filter(|(_, val)| {
            let mut arr = [0u8; 16];
            arr.copy_from_slice(val);
            u128::from_le_bytes(arr) > 0
        }).count()
    }

    pub fn push_crc21_log(&self, block_hash: &str, txid: &str, log: serde_json::Value) {
        let key = format!("crc21:log:{}:{}", block_hash, txid);
        let _ = self.db.insert(key.as_bytes(), serde_json::to_vec(&log).unwrap());
    }
    
    pub fn push_crc20_activity(&self, ticker: &str, txid: &str, height: u32, log: serde_json::Value) {
        let key = format!("crc20:log:{}:{}:{}", ticker, height, txid);
        let _ = self.db.insert(key.as_bytes(), serde_json::to_vec(&log).unwrap());
    }
    
    pub fn get_crc20_activity(&self, ticker: &str) -> Vec<serde_json::Value> {
        let prefix = format!("crc20:log:{}:", ticker);
        let mut logs = Vec::new();
        for res in self.db.scan_prefix(prefix) {
            if let Ok((_, v)) = res {
                if let Ok(log) = serde_json::from_slice(&v) {
                    logs.push(log);
                }
            }
        }
        logs
    }
    pub fn get_all_tokens(&self) -> Vec<(String, SigmaLeaf)> {
        let mut tokens = Vec::new();
        for res in self.db.scan_prefix(b"sigma:leaf:") {
            if let Ok((k, v)) = res {
                let k_str = String::from_utf8_lossy(&k);
                if let Ok(sigma) = SigmaLeaf::from_canonical_bytes(&v) {
                    tokens.push((k_str.strip_prefix("sigma:leaf:").unwrap().to_string(), sigma));
                }
            }
        }
        tokens
    }

    pub fn get_holders(&self, ticker: &str) -> Vec<(String, u128)> {
        let prefix = format!("bal:{}:", ticker);
        let mut holders = Vec::new();
        for res in self.db.scan_prefix(prefix.as_bytes()) {
            if let Ok((k, v)) = res {
                let key_str = String::from_utf8_lossy(&k);
                if let Some(addr) = key_str.strip_prefix(&prefix) {
                    let mut bytes = [0u8; 16];
                    bytes.copy_from_slice(&v);
                    let bal = u128::from_le_bytes(bytes);
                    if bal > 0 {
                        holders.push((addr.to_string(), bal));
                    }
                }
            }
        }
        holders
    }
    pub fn get_crc21_logs(&self) -> Vec<serde_json::Value> {
        let mut logs = Vec::new();
        for res in self.db.scan_prefix(b"crc21:log:") {
            if let Ok((_, val)) = res {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&val) {
                    logs.push(json);
                }
            }
        }
        logs
    }

    pub fn commit_block_batch(
        &self, 
        height: u32, 
        hash: &str, 
        sigmas: &[(String, SigmaLeaf, Option<SigmaLeaf>)], 
        balances: &[(String, String, u128, u128)]
    ) {
        let mut batch = sled::Batch::default();
        
        // 1. Block tracking
        let hash_key = format!("block_hash:{}", height);
        batch.insert(hash_key.as_bytes(), hash.as_bytes());
        batch.insert(b"__LAST_SCANNED_HEIGHT__", &height.to_be_bytes());

        let mut undo_sigmas: std::collections::HashMap<String, Option<Vec<u8>>> = std::collections::HashMap::new();
        for (ticker, new_sigma, prev_sigma_opt) in sigmas {
            let key = format!("sigma:leaf:{}", ticker);
            let snap_key = format!("sigma:leaf:{}:{}", ticker, height);
            let data = new_sigma.to_canonical_bytes();
            batch.insert(key.as_bytes(), &data[..]);
            batch.insert(snap_key.as_bytes(), &data[..]);
            
            if !undo_sigmas.contains_key(ticker) {
                undo_sigmas.insert(ticker.clone(), prev_sigma_opt.as_ref().map(|s| s.to_canonical_bytes().to_vec()));
            }
        }

        let mut undo_balances: std::collections::HashMap<String, u128> = std::collections::HashMap::new();
        for (ticker, address, new_bal, prev_bal) in balances {
            let key = format!("bal:{}:{}", ticker, address);
            batch.insert(key.as_bytes(), &new_bal.to_le_bytes());
            
            let undo_key = format!("{}:{}", ticker, address);
            if !undo_balances.contains_key(&undo_key) {
                undo_balances.insert(undo_key, *prev_bal);
            }
        }

        let undo_data = serde_json::to_vec(&(undo_sigmas, undo_balances)).unwrap();
        let undo_key = format!("undo:{}", height);
        batch.insert(undo_key.as_bytes(), undo_data);

        self.db.apply_batch(batch).unwrap();
        self.db.flush().unwrap();
    }

    pub fn rollback_block(&self, height: u32) {
        let mut batch = sled::Batch::default();
        
        let undo_key = format!("undo:{}", height);
        if let Ok(Some(data)) = self.db.get(undo_key.as_bytes()) {
            if let Ok((undo_sigmas, undo_balances)) = serde_json::from_slice::<(std::collections::HashMap<String, Option<Vec<u8>>>, std::collections::HashMap<String, u128>)>(&data) {
                for (ticker, prev_data_opt) in undo_sigmas {
                    let key = format!("sigma:leaf:{}", ticker);
                    if let Some(prev_data) = prev_data_opt {
                        batch.insert(key.as_bytes(), prev_data);
                    } else {
                        batch.remove(key.as_bytes());
                    }
                }
                for (undo_key, prev_bal) in undo_balances {
                    let mut parts = undo_key.splitn(2, ':');
                    if let (Some(ticker), Some(address)) = (parts.next(), parts.next()) {
                        let key = format!("bal:{}:{}", ticker, address);
                        if prev_bal == 0 {
                            batch.remove(key.as_bytes());
                        } else {
                            batch.insert(key.as_bytes(), &prev_bal.to_le_bytes());
                        }
                    }
                }
            }
        }

        if let Ok(Some(hash_data)) = self.db.get(format!("block_hash:{}", height).as_bytes()) {
            if let Ok(block_hash) = String::from_utf8(hash_data.to_vec()) {
                let prefix = format!("crc21:log:{}:", block_hash);
                for res in self.db.scan_prefix(prefix) {
                    if let Ok((k, _)) = res {
                        batch.remove(k);
                    }
                }
            }
        }

        batch.remove(undo_key.as_bytes());
        batch.remove(format!("block_hash:{}", height).as_bytes());
        batch.insert(b"__LAST_SCANNED_HEIGHT__", &(height.saturating_sub(1)).to_be_bytes());

        self.db.apply_batch(batch).unwrap();
        self.db.flush().unwrap();
    }
}
