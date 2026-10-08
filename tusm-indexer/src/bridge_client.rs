//! Precopscan Bridge — contract v2.
//!
//! The indexer is the SINGLE writer of the Precopscan PostgreSQL registry. Every state transition
//! observed by the VM is pushed, in block order, to `POST {PRECOPSCAN_API_URL}` (the
//! `/api/v1/bridge/ingest` route) as:
//!
//! ```json
//! { "schema_version": 2, "event_type": "MintCrc20", "payload": { ... } }
//! ```
//!
//! Ordering guarantee: events are produced by the scanner thread only and drained by a single
//! consumer that retries until the gateway acknowledges, so the registry is a deterministic
//! function of the chain (no silent drops).
use serde::{Deserialize, Serialize};

pub const BRIDGE_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BridgeEventType {
    DeployCrc20,
    MintCrc20,
    TransferCrc20,
    DeployCrc21,
    MintCrc21,
    TransferCrc21,
    /// A Taproot covenant (Astrolabe vault) was created on L1.
    VaultCreated,
    /// A vault UTXO was spent (script path executed).
    VaultSpent,
    /// All events of a block have been emitted.
    BlockProcessed,
    /// Chain reorganisation: every row above `common_ancestor` must be discarded.
    ChainReorg,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BridgePayload {
    #[serde(default = "default_schema")]
    pub schema_version: u32,
    pub event_type: BridgeEventType,
    pub payload: serde_json::Value,
}

fn default_schema() -> u32 { BRIDGE_SCHEMA_VERSION }

impl BridgePayload {
    pub fn new(event_type: BridgeEventType, payload: serde_json::Value) -> Self {
        Self { schema_version: BRIDGE_SCHEMA_VERSION, event_type, payload }
    }
}

/// One Taproot leaf of a covenant, as committed on-chain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VaultLeaf {
    pub script_hex: String,
    pub depth: u8,
    pub label: String,
}

pub struct BridgeClient {
    pub api_url: String,
    pub api_key: String,
}

impl BridgeClient {
    pub fn new(api_url: String, api_key: String) -> Self {
        Self { api_url, api_key }
    }

    // ───────────────────────────── CRC-21 ─────────────────────────────

    /// Prepare the JSON payload for a CRC-21 Mint (e.g. Merkel Koala)
    pub fn prepare_crc21_mint(
        &self,
        item_id: &str,
        collection_id: &str,
        owner_outpoint: &str,
        mint_block: u32,
        vault_address: &str,
        vault_commitment: &str,
        lock_tier_blocks: u32,
        traits: serde_json::Value,
    ) -> BridgePayload {
        self.prepare_crc21_mint_v2(
            item_id,
            collection_id,
            owner_outpoint,
            mint_block,
            vault_address,
            vault_commitment,
            lock_tier_blocks,
            traits,
            serde_json::Value::Null,
            None,
        )
    }

    /// Prepare CRC-21 Mint with full cryptographic proof and body hash
    pub fn prepare_crc21_mint_v2(
        &self,
        item_id: &str,
        collection_id: &str,
        owner_outpoint: &str,
        mint_block: u32,
        vault_address: &str,
        vault_commitment: &str,
        lock_tier_blocks: u32,
        traits: serde_json::Value,
        proof: serde_json::Value,
        body_sha256: Option<String>,
    ) -> BridgePayload {
        let txid = owner_outpoint.split(':').next().unwrap_or("");
        BridgePayload::new(BridgeEventType::MintCrc21, serde_json::json!({
            "item_id": item_id,
            "collection_id": collection_id,
            "owner_outpoint": owner_outpoint,
            "txid": txid,
            "mint_block": mint_block,
            "height": mint_block,
            "vault": {
                "address": vault_address,
                "commitment": vault_commitment,
                "lock_tier_blocks": lock_tier_blocks
            },
            "traits": traits,
            "proof": proof,
            "body_sha256": body_sha256,
        }))
    }

    pub fn prepare_crc21_deploy(&self, collection_id: &str, deploy_txid: &str, parent_utxo: &str) -> BridgePayload {
        self.prepare_crc21_deploy_at(collection_id, deploy_txid, parent_utxo, 0, serde_json::Value::Null)
    }

    pub fn prepare_crc21_deploy_at(
        &self,
        collection_id: &str,
        deploy_txid: &str,
        parent_utxo: &str,
        height: u32,
        metadata: serde_json::Value,
    ) -> BridgePayload {
        BridgePayload::new(BridgeEventType::DeployCrc21, serde_json::json!({
            "collection_id": collection_id,
            "deploy_txid": deploy_txid,
            "txid": deploy_txid,
            "parent_utxo": parent_utxo,
            "height": height,
            "metadata": metadata
        }))
    }

    pub fn prepare_crc21_transfer(&self, item_id: &str, from: &str, to: &str, txid: &str) -> BridgePayload {
        self.prepare_crc21_transfer_at(item_id, from, to, txid, 0)
    }

    pub fn prepare_crc21_transfer_at(&self, item_id: &str, from: &str, to: &str, txid: &str, height: u32) -> BridgePayload {
        BridgePayload::new(BridgeEventType::TransferCrc21, serde_json::json!({
            "item_id": item_id,
            "from": from,
            "to": to,
            "txid": txid,
            "height": height
        }))
    }

    // ───────────────────────────── CRC-20 ─────────────────────────────

    /// `atoms` is the amount ALLOWED by the VM (post cap / clipping), never the raw inscription amt.
    pub fn prepare_crc20_mint_v2(
        &self,
        txid: &str,
        height: u32,
        address: &str,
        atoms: u128,
        ticker: &str,
        seal_sats: u64,
        json: &serde_json::Value,
    ) -> BridgePayload {
        BridgePayload::new(BridgeEventType::MintCrc20, serde_json::json!({
            "txid": txid,
            "height": height,
            "address": address,
            "atoms": atoms.to_string(),
            "ticker": ticker,
            "seal_sats": seal_sats,
            "json": json
        }))
    }

    pub fn prepare_crc20_transfer_v2(
        &self,
        txid: &str,
        height: u32,
        from: &str,
        to: &str,
        atoms: u128,
        ticker: &str,
        json: &serde_json::Value,
    ) -> BridgePayload {
        BridgePayload::new(BridgeEventType::TransferCrc20, serde_json::json!({
            "txid": txid,
            "height": height,
            "from": from,
            "to": to,
            "atoms": atoms.to_string(),
            "ticker": ticker,
            "json": json
        }))
    }

    /// `max` / `lim` are in atoms (LegacyV1Router::parse_amount, 8 decimals).
    pub fn prepare_crc20_deploy_v2(
        &self,
        txid: &str,
        height: u32,
        ticker: &str,
        deployer: &str,
        max: u128,
        lim: u128,
        json: &serde_json::Value,
    ) -> BridgePayload {
        BridgePayload::new(BridgeEventType::DeployCrc20, serde_json::json!({
            "txid": txid,
            "height": height,
            "ticker": ticker,
            "deployer": deployer,
            "max": max.to_string(),
            "lim": lim.to_string(),
            "decimals": 8,
            "json": json
        }))
    }

    // ───────────────────────────── Astrolabe vaults ─────────────────────────────

    /// The gateway independently recomputes `p2tr_address` from (`nums_key`, `leaves`) and
    /// rejects the event on mismatch: the registry only ever contains mathematically proven vaults.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_vault_created(
        &self,
        vault_type: &str,
        txid: &str,
        vout: u32,
        height: u32,
        value_sats: u64,
        p2tr_address: &str,
        nums_key: &str,
        owner_key: Option<&str>,
        leaves: &[VaultLeaf],
        type_data: serde_json::Value,
    ) -> BridgePayload {
        BridgePayload::new(BridgeEventType::VaultCreated, serde_json::json!({
            "vault_type": vault_type,
            "txid": txid,
            "vout": vout,
            "height": height,
            "value_sats": value_sats,
            "p2tr_address": p2tr_address,
            "nums_key": nums_key,
            "owner_key": owner_key,
            "leaves": leaves,
            "data": type_data
        }))
    }

    pub fn prepare_vault_spent(
        &self,
        outpoint: &str,
        spend_txid: &str,
        height: u32,
        leaf_script_hex: Option<&str>,
    ) -> BridgePayload {
        BridgePayload::new(BridgeEventType::VaultSpent, serde_json::json!({
            "outpoint": outpoint,
            "spend_txid": spend_txid,
            "txid": spend_txid,
            "height": height,
            "leaf_script_hex": leaf_script_hex
        }))
    }

    /// `time` = block header timestamp (consensus data) -> deterministic time windows in Precopscan.
    pub fn prepare_block_processed(&self, height: u32, block_hash: &str, time: u32) -> BridgePayload {
        BridgePayload::new(BridgeEventType::BlockProcessed, serde_json::json!({
            "height": height,
            "block_hash": block_hash,
            "time": time
        }))
    }

    pub fn prepare_chain_reorg(&self, common_ancestor: u32) -> BridgePayload {
        BridgePayload::new(BridgeEventType::ChainReorg, serde_json::json!({
            "common_ancestor": common_ancestor,
            "height": common_ancestor
        }))
    }

    // ───────────────────────────── Transport ─────────────────────────────

    /// Pushes one event. Retries with capped exponential backoff on transport errors, 5xx, 401
    /// and 429 (preserving order). Definitive 4xx rejections are logged and skipped: the gateway
    /// has already recorded them as REJECTED in `BridgeEventLog`.
    pub async fn push_event(&self, payload: BridgePayload) -> Result<(), reqwest::Error> {
        let client = reqwest::Client::new();
        let mut backoff_ms: u64 = 500;
        loop {
            let res = client
                .post(&self.api_url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .json(&payload)
                .send()
                .await;
            match res {
                Ok(r) if r.status().is_success() => return Ok(()),
                Ok(r) => {
                    let status = r.status();
                    let retryable = status.is_server_error()
                        || status == reqwest::StatusCode::UNAUTHORIZED
                        || status == reqwest::StatusCode::TOO_MANY_REQUESTS;
                    let body = r.text().await.unwrap_or_default();
                    if !retryable {
                        println!("[Bridge] REJECTED {:?} (HTTP {}): {}", payload.event_type, status, body);
                        return Ok(());
                    }
                    println!("[Bridge] HTTP {} for {:?}, retry in {}ms: {}", status, payload.event_type, backoff_ms, body);
                }
                Err(e) => println!("[Bridge] transport error for {:?}, retry in {}ms: {}", payload.event_type, backoff_ms, e),
            }
            tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
            backoff_ms = (backoff_ms * 2).min(30_000);
        }
    }
}

// ───────────────────────────── Isolation from indexing ─────────────────────────────
//
// The bridge is a pure OBSERVER of the VM. Guarantees:
//  * disabled (PRECOPSCAN_API_URL unset) => `emit` and every `vault_watch` side effect are
//    no-ops: the indexer behaves exactly as without the bridge;
//  * enabled => `emit` never blocks the scanner. The in-memory queue is bounded by
//    `PRECOPSCAN_BRIDGE_MAX_PENDING` (default 200 000 events); above it events are dropped and
//    counted (the Precopscan registry must then be re-synced) instead of slowing or OOM-ing the
//    indexer. Consensus state (sled) is never touched by this module.
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

pub const DEFAULT_MAX_PENDING: usize = 200_000;

static ENABLED: AtomicBool = AtomicBool::new(false);
static PENDING: AtomicUsize = AtomicUsize::new(0);
static MAX_PENDING: AtomicUsize = AtomicUsize::new(DEFAULT_MAX_PENDING);
static DROPPED: AtomicU64 = AtomicU64::new(0);

/// Called once by `main` when `PRECOPSCAN_API_URL` is set.
pub fn enable(max_pending: usize) {
    MAX_PENDING.store(max_pending.max(1), Ordering::SeqCst);
    ENABLED.store(true, Ordering::SeqCst);
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Number of events emitted but not yet acknowledged by the transport task.
pub fn pending() -> usize {
    PENDING.load(Ordering::Relaxed)
}

/// Number of events dropped because the queue was full (registry incomplete if > 0).
pub fn dropped() -> u64 {
    DROPPED.load(Ordering::Relaxed)
}

/// Called by the transport task after each event has been handled (acknowledged or rejected).
pub fn mark_delivered() {
    let _ = PENDING.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| v.checked_sub(1));
}

/// Events emitted while a block is being processed are staged here and only released once the
/// VM has committed the block (`end_block(true)`): an aborted block publishes nothing.
static BLOCK_STAGE: std::sync::Mutex<Option<Vec<BridgePayload>>> = std::sync::Mutex::new(None);

/// Starts staging the events of one block (no-op when disabled).
pub fn begin_block() {
    if !is_enabled() {
        return;
    }
    if let Ok(mut stage) = BLOCK_STAGE.lock() {
        *stage = Some(Vec::new());
    }
}

/// Releases (`committed == true`) or discards the staged events of the current block.
pub fn end_block(committed: bool) {
    let staged = match BLOCK_STAGE.lock() {
        Ok(mut stage) => stage.take(),
        Err(_) => None,
    };
    if committed {
        for p in staged.unwrap_or_default() {
            emit_now(p);
        }
    }
}

/// Emits an event: staged if a block is in progress, sent otherwise. Never blocks; no-op when
/// disabled.
pub fn emit(payload: BridgePayload) {
    if !is_enabled() {
        return;
    }
    if let Ok(mut stage) = BLOCK_STAGE.lock() {
        if let Some(buf) = stage.as_mut() {
            buf.push(payload);
            return;
        }
    }
    emit_now(payload);
}

/// Sends an event immediately, bypassing block staging (used for `ChainReorg`, which must be
/// published even though the block that detected it is aborted).
pub fn emit_now(payload: BridgePayload) {
    if !is_enabled() {
        return;
    }
    let Some(tx) = crate::BRIDGE_TX.get() else { return };
    if PENDING.load(Ordering::SeqCst) >= MAX_PENDING.load(Ordering::Relaxed) {
        let n = DROPPED.fetch_add(1, Ordering::SeqCst) + 1;
        if n == 1 || n % 1000 == 0 {
            println!("[Bridge] DEGRADED: queue full ({} pending), {} event(s) dropped — Precopscan registry must be re-synced", pending(), n);
        }
        return;
    }
    PENDING.fetch_add(1, Ordering::SeqCst);
    if tx.send(payload).is_err() {
        mark_delivered();
    }
}

/// Stateless builder (the URL/key are only needed by the transport task).
pub fn builder() -> BridgeClient {
    BridgeClient::new(String::new(), String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_crc21_payload_formatting() {
        let client = BridgeClient::new("http://localhost:3000".to_string(), "secret".to_string());

        let traits = serde_json::json!({
            "Background": "Light Brown",
            "Body": "Ash",
            "Shirt": "Dungarees",
            "Face": "Maxi",
            "Hats": "Daisy Flower",
            "Item": "None",
            "Attributes": "Cigar"
        });

        let payload = client.prepare_crc21_mint(
            "116f5736d68aef805656c2fb3b2740ebef9cd9a428bd5f62611588550f27a28ai0",
            "merkel_koala",
            "116f5736d68aef805656c2fb3b2740ebef9cd9a428bd5f62611588550f27a28a:0",
            969164,
            "bc1pz2nre6mau9smd02q6kskdrwn87xq0hc0gpn0rf55php3nvwccavqqh5y72",
            "6c4f39bb3240dd0295de04eed4a70bddb6ad150f362617f8bcb48a2b670c1f2e",
            5000,
            traits
        );

        assert_eq!(payload.event_type, BridgeEventType::MintCrc21);
        assert_eq!(payload.schema_version, BRIDGE_SCHEMA_VERSION);
        assert_eq!(payload.payload["item_id"], "116f5736d68aef805656c2fb3b2740ebef9cd9a428bd5f62611588550f27a28ai0");
        assert_eq!(payload.payload["vault"]["lock_tier_blocks"], 5000);
        assert_eq!(payload.payload["traits"]["Hats"], "Daisy Flower");
        assert_eq!(payload.payload["mint_block"], 969164);
        assert_eq!(payload.payload["txid"], "116f5736d68aef805656c2fb3b2740ebef9cd9a428bd5f62611588550f27a28a");
    }

    #[test]
    fn test_wire_format_is_the_gateway_contract() {
        let p = builder().prepare_block_processed(969864, "00ab", 1791100000);
        let wire = serde_json::to_value(&p).unwrap();
        assert_eq!(wire["schema_version"], 2);
        assert_eq!(wire["event_type"], "BlockProcessed");
        assert_eq!(wire["payload"]["height"], 969864);
        assert_eq!(wire["payload"]["time"], 1791100000u32);
    }

    #[test]
    fn test_vault_created_carries_full_topology() {
        let leaves = vec![VaultLeaf { script_hex: "51".into(), depth: 0, label: "CSV_UNLOCK_1000".into() }];
        let p = builder().prepare_vault_created(
            "TGE_CSV", &"ab".repeat(32), 2, 967500, 546, "bc1p…", "50929b74", Some("f9"), &leaves,
            serde_json::json!({"ticker": "LEAF", "locked_atoms": "100", "csv_delay": 1000}),
        );
        assert_eq!(p.event_type, BridgeEventType::VaultCreated);
        assert_eq!(p.payload["leaves"][0]["depth"], 0);
        assert_eq!(p.payload["data"]["csv_delay"], 1000);
        assert_eq!(p.payload["vout"], 2);
    }
}
