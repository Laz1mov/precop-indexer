use axum::{
    extract::{Path, State, Query},
    routing::get,
    Router, Json, http::StatusCode,
};
use serde::{Serialize, Deserialize};
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;
use std::sync::Arc;
use crate::state_db::StateDb;
use sha2::{Sha256, Digest};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<StateDb>,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        health, status,
        token_info, token_list,
        token_balance, address_tokens,
        token_holders, token_supply, token_activity, token_tx,
        crc21_collection, crc21_collection_items, crc21_item, crc21_outpoint, crc21_activity
    ),
    components(schemas(
        HealthResponse, StatusResponse, TokenResponse,
        BalanceResponse, AddressTokensResponse, Holder, HoldersResponse,
        SupplyResponse, ActivityLog, ActivityResponse,
        Crc21CollectionResponse, Crc21ItemResponse, Crc21OutpointResponse
    ))
)]
pub struct ApiDoc;

// -----------------------------------------------------------------------------
// Health and Proof of State
// -----------------------------------------------------------------------------

#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub scanned_height: u32,
    pub scanned_hash: String,
    pub tip_height: u64,
    pub tip_hash: String,
    pub lag: u64,
    pub schema: String,
    pub prevouts: String,
    pub rpc: String,
}

struct RpcHealthCache {
    last_check: std::time::Instant,
    tip_height: u64,
    tip_hash: String,
    rpc_status: String,
}

static HEALTH_CACHE: once_cell::sync::Lazy<std::sync::Mutex<Option<RpcHealthCache>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(None));

#[utoipa::path(
    get,
    path = "/v1/health",
    responses(
        (status = 200, description = "Indexer health status", body = HealthResponse)
    )
)]
pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let scanned = state.db.get_last_scanned_height();
    let scanned_hash = state.db.get_block_hash(scanned).unwrap_or_else(|| "N/A".to_string());
    
    let rpc_url = std::env::var("BITCOIN_RPC_URL").unwrap_or_else(|_| "http://127.0.0.1:8332".to_string());
    let rpc_user = std::env::var("BITCOIN_RPC_USER").unwrap_or_else(|_| "".to_string());
    let rpc_pass = std::env::var("BITCOIN_RPC_PASS").unwrap_or_else(|_| "".to_string());
    
    let (tip_height, tip_hash, rpc_status) = {
        let mut cache_guard = HEALTH_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        let needs_fetch = match *cache_guard {
            Some(ref c) => c.last_check.elapsed() >= std::time::Duration::from_secs(5),
            None => true,
        };

        if needs_fetch {
            let mut tip_h = 0;
            let mut tip_hs = "N/A".to_string();
            let mut rpc_s = "down".to_string();

            if let Ok(c) = bitcoincore_rpc::Client::new(&rpc_url, bitcoincore_rpc::Auth::UserPass(rpc_user, rpc_pass)) {
                use bitcoincore_rpc::RpcApi;
                if let Ok(tip) = c.get_block_count() {
                    tip_h = tip;
                    rpc_s = "ok".to_string();
                    if let Ok(hash) = c.get_block_hash(tip) {
                        tip_hs = hash.to_string();
                    }
                }
            }

            *cache_guard = Some(RpcHealthCache {
                last_check: std::time::Instant::now(),
                tip_height: tip_h,
                tip_hash: tip_hs.clone(),
                rpc_status: rpc_s.clone(),
            });
            (tip_h, tip_hs, rpc_s)
        } else {
            let c = cache_guard.as_ref().unwrap();
            (c.tip_height, c.tip_hash.clone(), c.rpc_status.clone())
        }
    };
    
    let lag = if tip_height >= scanned as u64 { tip_height - scanned as u64 } else { 0 };

    Json(HealthResponse {
        status: if lag > 10 { "syncing".to_string() } else if rpc_status == "ok" { "ok".to_string() } else { "rpc_down".to_string() },
        scanned_height: scanned,
        scanned_hash,
        tip_height,
        tip_hash,
        lag,
        schema: "crc20-bonding-v1".to_string(),
        prevouts: "on".to_string(),
        rpc: rpc_status,
    })
}

#[derive(Serialize, ToSchema)]
pub struct StatusResponse {
    pub height: u32,
    pub block_hash: String,
    pub state_root: String,
}

#[utoipa::path(
    get,
    path = "/v1/status/{height}",
    responses(
        (status = 200, description = "Status at height", body = StatusResponse),
        (status = 404, description = "Height not scanned yet")
    )
)]
pub async fn status(
    Path(height): Path<u32>,
    State(state): State<AppState>,
) -> Result<Json<StatusResponse>, (StatusCode, String)> {
    if height > state.db.get_last_scanned_height() {
        return Err((StatusCode::NOT_FOUND, "Height not scanned yet".to_string()));
    }
    let block_hash = state.db.get_block_hash(height).unwrap_or_else(|| "N/A".to_string());
    
    let mut hasher = Sha256::new();
    hasher.update(format!("{}:{}", height, block_hash));
    let state_root = format!("{:x}", hasher.finalize());
    
    Ok(Json(StatusResponse {
        height,
        block_hash,
        state_root,
    }))
}

// -----------------------------------------------------------------------------
// CRC-20
// -----------------------------------------------------------------------------

#[derive(Serialize, ToSchema)]
pub struct TokenResponse {
    pub ticker: String,
    pub deploy: String,
    pub height: u32,
    pub max: String,
    pub lim: String,
    pub btc_rate: String,
    pub dog_rate: String,
    pub leaf_rate: String,
    pub supply: String,
    pub holders: usize,
    pub halted: bool,
    pub block_hash: String,
}

#[utoipa::path(get, path = "/v1/crc20/tokens/{ticker}", responses((status = 200, body = TokenResponse), (status = 404)))]
pub async fn token_info(Path(ticker): Path<String>, State(state): State<AppState>) -> Result<Json<TokenResponse>, (StatusCode, String)> {
    let ticker = ticker.to_uppercase();
    if ticker.len() > 10 { return Err((StatusCode::BAD_REQUEST, "invalid".to_string())); }
    
    if let Some(sigma) = state.db.get_sigma(&ticker) {
        let count = state.db.get_holders_count(&ticker);
        let block_hash = state.db.get_block_hash(sigma.height).unwrap_or_default();
        let deploy_txid = format!("{}:{}", sigma.anchor.txid, sigma.anchor.vout);
        
        Ok(Json(TokenResponse {
            ticker,
            deploy: deploy_txid,
            height: sigma.height,
            max: sigma.max_supply.to_string(),
            lim: sigma.limit_per_mint.to_string(),
            btc_rate: sigma.btc_rate.to_string(),
            dog_rate: sigma.dog_rate.to_string(),
            leaf_rate: sigma.leaf_rate.to_string(),
            supply: sigma.supply.to_string(),
            holders: count,
            halted: sigma.halted,
            block_hash,
        }))
    } else {
        Err((StatusCode::NOT_FOUND, "Not found".to_string()))
    }
}

#[derive(Deserialize)]
pub struct CursorQuery {
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

#[utoipa::path(get, path = "/v1/crc20/tokens", responses((status = 200, body = Vec<String>)))]
pub async fn token_list(Query(q): Query<CursorQuery>, State(state): State<AppState>) -> Json<Vec<String>> {
    let mut tokens: Vec<String> = state.db.get_all_tokens().into_iter().map(|(ticker, _)| ticker).collect();
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    if let Some(ref cursor) = q.cursor {
        if let Some(pos) = tokens.iter().position(|t| t == cursor) {
            tokens = tokens.into_iter().skip(pos + 1).collect();
        }
    }
    tokens.truncate(limit);
    Json(tokens)
}

#[derive(Serialize, ToSchema)]
pub struct BalanceResponse {
    pub ticker: String,
    pub address: String,
    pub balance: String,
    pub height: u32,
    pub schema: String,
    pub block_hash: String,
}

#[utoipa::path(get, path = "/v1/crc20/balance/{ticker}/{address}", responses((status = 200, body = BalanceResponse)))]
pub async fn token_balance(Path((ticker, address)): Path<(String, String)>, State(state): State<AppState>) -> Json<BalanceResponse> {
    let ticker = ticker.to_uppercase();
    let balance = state.db.get_balance(&ticker, &address);
    let height = state.db.get_last_scanned_height();
    let block_hash = state.db.get_block_hash(height).unwrap_or_default();
    
    Json(BalanceResponse {
        ticker,
        address,
        balance: balance.to_string(),
        height,
        schema: "v1".to_string(),
        block_hash,
    })
}

#[derive(Serialize, ToSchema)]
pub struct AddressTokensResponse {
    pub address: String,
    pub tokens: Vec<BalanceResponse>,
    pub height: u32,
    pub block_hash: String,
}

#[utoipa::path(get, path = "/v1/crc20/address/{address}", responses((status = 200, body = AddressTokensResponse)))]
pub async fn address_tokens(Path(address): Path<String>, State(state): State<AppState>) -> Json<AddressTokensResponse> {
    let height = state.db.get_last_scanned_height();
    let block_hash = state.db.get_block_hash(height).unwrap_or_default();
    Json(AddressTokensResponse {
        address,
        tokens: vec![],
        height,
        block_hash,
    })
}

#[derive(Serialize, ToSchema)]
pub struct Holder {
    pub address: String,
    pub balance: String,
}

#[derive(Serialize, ToSchema)]
pub struct HoldersResponse {
    pub ticker: String,
    pub holders: Vec<Holder>,
    pub height: u32,
    pub block_hash: String,
}

#[utoipa::path(get, path = "/v1/crc20/holders/{ticker}", responses((status = 200, body = HoldersResponse)))]
pub async fn token_holders(Path(ticker): Path<String>, Query(q): Query<CursorQuery>, State(state): State<AppState>) -> Json<HoldersResponse> {
    let ticker = ticker.to_uppercase();
    let height = state.db.get_last_scanned_height();
    let block_hash = state.db.get_block_hash(height).unwrap_or_default();
    let mut raw_holders: Vec<(String, u128)> = state.db.get_holders(&ticker);
    raw_holders.sort_by(|a, b| b.1.cmp(&a.1));
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    if let Some(ref cursor) = q.cursor {
        if let Some(pos) = raw_holders.iter().position(|(addr, _)| addr == cursor) {
            raw_holders = raw_holders.into_iter().skip(pos + 1).collect();
        }
    }
    raw_holders.truncate(limit);
    let holders = raw_holders.into_iter().map(|(addr, bal)| Holder {
        address: addr,
        balance: bal.to_string(),
    }).collect();
    Json(HoldersResponse {
        ticker,
        holders, 
        height,
        block_hash,
    })
}

#[derive(Serialize, ToSchema)]
pub struct SupplyResponse {
    pub ticker: String,
    pub supply: String,
    pub max_supply: String,
    pub minted: String,
    pub sum_balances: String,
    pub consistent: bool,
    pub height: u32,
    pub block_hash: String,
}

#[utoipa::path(get, path = "/v1/crc20/supply/{ticker}", responses((status = 200, body = SupplyResponse)))]
pub async fn token_supply(Path(ticker): Path<String>, State(state): State<AppState>) -> Result<Json<SupplyResponse>, (StatusCode, String)> {
    let ticker = ticker.to_uppercase();
    if let Some(sigma) = state.db.get_sigma(&ticker) {
        let sum_bal = state.db.get_total_balance(&ticker);
        let block_hash = state.db.get_block_hash(sigma.height).unwrap_or_default();
        
        Ok(Json(SupplyResponse {
            ticker,
            supply: sigma.supply.to_string(),
            max_supply: sigma.max_supply.to_string(),
            minted: sigma.supply.to_string(),
            sum_balances: sum_bal.to_string(),
            consistent: sigma.supply == sum_bal,
            height: sigma.height,
            block_hash,
        }))
    } else {
        Err((StatusCode::NOT_FOUND, "Not found".to_string()))
    }
}

#[derive(Serialize, ToSchema)]
pub struct ActivityLog {
    pub txid: String,
    pub event: String,
    pub height: u32,
    pub amount: String,
    pub receiver: String,
    pub raw_json: serde_json::Value,
}

#[derive(Serialize, ToSchema)]
pub struct ActivityResponse {
    pub items: Vec<ActivityLog>,
    pub height: u32,
    pub block_hash: String,
}

#[utoipa::path(get, path = "/v1/crc20/activity/{ticker}", responses((status = 200, body = ActivityResponse)))]
pub async fn token_activity(Path(ticker): Path<String>, Query(q): Query<CursorQuery>, State(state): State<AppState>) -> Json<ActivityResponse> {
    let height = state.db.get_last_scanned_height();
    let block_hash = state.db.get_block_hash(height).unwrap_or_default();
    
    let ticker = ticker.to_uppercase();
    let logs_val = state.db.get_crc20_activity(&ticker);
    
    let mut items = Vec::new();
    for val in logs_val {
        if let Some(obj) = val.as_object() {
            items.push(ActivityLog {
                txid: obj.get("txid").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                event: obj.get("event").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                height: obj.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                amount: obj.get("amount").and_then(|v| v.as_str()).unwrap_or("0").to_string(),
                receiver: obj.get("receiver").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                raw_json: val.clone(),
            });
        }
    }
    
    // Sort from newest to oldest
    items.sort_by(|a, b| b.height.cmp(&a.height));
    
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    if let Some(ref cursor) = q.cursor {
        if let Some(pos) = items.iter().position(|it| &it.txid == cursor) {
            items = items.into_iter().skip(pos + 1).collect();
        }
    }
    items.truncate(limit);

    Json(ActivityResponse { items, height, block_hash })
}

#[utoipa::path(get, path = "/v1/crc20/tx/{txid}", responses((status = 200, body = ActivityResponse)))]
pub async fn token_tx(Path(txid): Path<String>, State(state): State<AppState>) -> Json<ActivityResponse> {
    let height = state.db.get_last_scanned_height();
    let block_hash = state.db.get_block_hash(height).unwrap_or_default();
    
    // Search across all tokens for a single txid activity
    let mut items = Vec::new();
    for (ticker, _) in state.db.get_all_tokens() {
        let logs_val = state.db.get_crc20_activity(&ticker);
        for val in logs_val {
            if let Some(obj) = val.as_object() {
                let log_txid = obj.get("txid").and_then(|v| v.as_str()).unwrap_or("");
                if log_txid == txid {
                    items.push(ActivityLog {
                        txid: log_txid.to_string(),
                        event: obj.get("event").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        height: obj.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                        amount: obj.get("amount").and_then(|v| v.as_str()).unwrap_or("0").to_string(),
                        receiver: obj.get("receiver").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        raw_json: val.clone(),
                    });
                }
            }
        }
    }
    
    Json(ActivityResponse { items, height, block_hash })
}

// -----------------------------------------------------------------------------
// CRC-20 Audit Endpoints
// -----------------------------------------------------------------------------

#[utoipa::path(get, path = "/v1/crc20/external_payments", responses((status = 200, body = serde_json::Value)))]
pub async fn external_payments() -> Json<serde_json::Value> {
    let mut data = serde_json::json!({});
    let path = std::env::var("TUSM_EXTERNAL_PAYMENTS_PATH").unwrap_or_else(|_| {
        if std::path::Path::new("./data/external_payments.json").exists() {
            "./data/external_payments.json".to_string()
        } else {
            "/app/data/external_payments.json".to_string()
        }
    });
    if let Ok(content) = std::fs::read_to_string(&path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            data = json;
        }
    }
    Json(data)
}

// -----------------------------------------------------------------------------
// CRC-21
// -----------------------------------------------------------------------------

#[derive(Serialize, ToSchema)]
pub struct Crc21CollectionResponse {
    pub collection_id: String,
    pub parent_utxo: String,
    pub deploy_txid: String,
    pub height: u32,
    pub block_hash: String,
}

#[utoipa::path(get, path = "/v1/crc21/collections", responses((status = 200, body = Vec<String>)))]
pub async fn crc21_collections(State(state): State<AppState>) -> Json<Vec<String>> {
    let collections = state.db.get_crc21_collections();
    Json(collections)
}

#[utoipa::path(get, path = "/v1/crc21/collections/{id}", responses((status = 200, body = Crc21CollectionResponse), (status = 404)))]
pub async fn crc21_collection(Path(id): Path<String>, State(state): State<AppState>) -> Result<Json<Crc21CollectionResponse>, (StatusCode, String)> {
    if let Some(col) = state.db.get_crc21_collection(&id) {
        let mut parent_utxo = "unknown".to_string();
        for res in state.db.db.scan_prefix(b"crc21:uparent:") {
            if let Ok((k, v)) = res {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&v) {
                    if json["collection_id"].as_str() == Some(&id) {
                        let k_str = String::from_utf8_lossy(&k);
                        parent_utxo = k_str.strip_prefix("crc21:uparent:").unwrap_or("").to_string();
                        break;
                    }
                }
            }
        }
        
        let height = state.db.get_last_scanned_height();
        Ok(Json(Crc21CollectionResponse {
            collection_id: id,
            parent_utxo,
            deploy_txid: col["deploy_txid"].as_str().unwrap_or("").to_string(),
            height,
            block_hash: state.db.get_block_hash(height).unwrap_or_default(),
        }))
    } else {
        Err((StatusCode::NOT_FOUND, "Not found".to_string()))
    }
}

#[utoipa::path(get, path = "/v1/crc21/collections/{id}/items", responses((status = 200, body = Vec<String>)))]
pub async fn crc21_collection_items(Path(id): Path<String>, Query(q): Query<CursorQuery>, State(state): State<AppState>) -> Json<Vec<String>> {
    let mut items = Vec::new();
    let prefix = format!("crc21:item:{}:", id);
    for res in state.db.db.scan_prefix(prefix.as_bytes()) {
        if let Ok((_k, v)) = res {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&v) {
                if let Some(item_id) = json["item_id"].as_str() {
                    items.push(item_id.to_string());
                }
            }
        }
    }
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    if let Some(ref cursor) = q.cursor {
        if let Some(pos) = items.iter().position(|it| it == cursor) {
            items = items.into_iter().skip(pos + 1).collect();
        }
    }
    items.truncate(limit);
    Json(items)
}

#[derive(Serialize, ToSchema)]
pub struct Crc21ItemResponse {
    pub item_id: String,
    pub collection_id: String,
    pub owner_outpoint: String,
    pub height: u32,
    pub block_hash: String,
}

#[utoipa::path(get, path = "/v1/crc21/items/{id}", responses((status = 200, body = Crc21ItemResponse)))]
pub async fn crc21_item(Path(id): Path<String>, State(state): State<AppState>) -> Result<Json<Crc21ItemResponse>, (StatusCode, String)> {
    if let Some((owner_outpoint, item)) = state.db.get_crc21_item_by_id(&id) {
        let height = state.db.get_last_scanned_height();
        let block_hash = state.db.get_block_hash(height).unwrap_or_default();
        Ok(Json(Crc21ItemResponse {
            item_id: id,
            collection_id: item["collection_id"].as_str().unwrap_or("").to_string(),
            owner_outpoint,
            height,
            block_hash,
        }))
    } else {
        Err((StatusCode::NOT_FOUND, "Item not found".to_string()))
    }
}

#[derive(Serialize, ToSchema)]
pub struct Crc21OutpointResponse {
    pub txid: String,
    pub vout: u32,
    pub kind: String,
    pub data: serde_json::Value,
    pub height: u32,
    pub block_hash: String,
}

#[utoipa::path(get, path = "/v1/crc21/outpoint/{txid}/{vout}", responses((status = 200, body = Crc21OutpointResponse)))]
pub async fn crc21_outpoint(Path((txid, vout)): Path<(String, u32)>, State(state): State<AppState>) -> Result<Json<Crc21OutpointResponse>, (StatusCode, String)> {
    let outpoint = format!("{}:{}", txid, vout);
    let height = state.db.get_last_scanned_height();
    let block_hash = state.db.get_block_hash(height).unwrap_or_default();
    
    if let Some(parent) = state.db.get_crc21_utxo_parent(&outpoint) {
        return Ok(Json(Crc21OutpointResponse {
            txid, vout,
            kind: "parent".to_string(),
            data: parent,
            height, block_hash
        }));
    }
    
    if let Some(item) = state.db.get_crc21_utxo_item(&outpoint) {
        return Ok(Json(Crc21OutpointResponse {
            txid, vout,
            kind: "item".to_string(),
            data: item,
            height, block_hash
        }));
    }

    Ok(Json(Crc21OutpointResponse {
        txid, vout,
        kind: "empty".to_string(),
        data: serde_json::json!({}),
        height, block_hash
    }))
}

#[utoipa::path(get, path = "/v1/crc21/activity", responses((status = 200, body = ActivityResponse)))]
pub async fn crc21_activity(Query(q): Query<CursorQuery>, State(state): State<AppState>) -> Json<ActivityResponse> {
    let height = state.db.get_last_scanned_height();
    let block_hash = state.db.get_block_hash(height).unwrap_or_default();
    
    let logs = state.db.get_crc21_logs();
    let mut items: Vec<ActivityLog> = logs.into_iter().map(|log| ActivityLog {
        txid: log["txid"].as_str().unwrap_or("").to_string(),
        event: log["event"].as_str().unwrap_or("").to_string(),
        height: log["height"].as_u64().unwrap_or(0) as u32,
        amount: "0".to_string(),
        receiver: "".to_string(),
        raw_json: log.clone(),
    }).collect();

    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    if let Some(ref cursor) = q.cursor {
        if let Some(pos) = items.iter().position(|it| &it.txid == cursor) {
            items = items.into_iter().skip(pos + 1).collect();
        }
    }
    items.truncate(limit);

    Json(ActivityResponse { items, height, block_hash })
}

// -----------------------------------------------------------------------------
// Router
// -----------------------------------------------------------------------------

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/status/:height", get(status))
        .route("/v1/crc20/tokens/:ticker", get(token_info))
        .route("/v1/crc20/tokens", get(token_list))
        .route("/v1/crc20/balance/:ticker/:address", get(token_balance))
        .route("/v1/crc20/address/:address", get(address_tokens))
        .route("/v1/crc20/holders/:ticker", get(token_holders))
        .route("/v1/crc20/supply/:ticker", get(token_supply))
        .route("/v1/crc20/activity/:ticker", get(token_activity))
        .route("/v1/crc20/tx/:txid", get(token_tx))
        .route("/v1/crc20/external_payments", get(external_payments))
        .route("/v1/crc21/collections", get(crc21_collections))
        .route("/v1/crc21/collections/:id", get(crc21_collection))
        .route("/v1/crc21/collections/:id/items", get(crc21_collection_items))
        .route("/v1/crc21/items/:id", get(crc21_item))
        .route("/v1/crc21/outpoint/:txid/:vout", get(crc21_outpoint))
        .route("/v1/crc21/activity", get(crc21_activity))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use hyper::Body;
    use tower::ServiceExt;
    
    #[tokio::test]
    async fn test_health_route() {
        let db_path = "test_api_health.db";
        let _ = std::fs::remove_dir_all(db_path);
        let db = Arc::new(StateDb::new(db_path));
        db.set_last_scanned_height(100);
        
        let app = router(AppState { db: db.clone() });
        let req = Request::builder().uri("/v1/health").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        
        assert_eq!(res.status(), StatusCode::OK);
        
        let _ = std::fs::remove_dir_all(db_path);
    }
}
