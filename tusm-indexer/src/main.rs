use axum::{
    extract::{Path, State},
    routing::get,
    Router, Json,
};
use precop_tusm_indexer::state_db::StateDb;
use precop_tusm_indexer::scanner_engine::ScannerEngine;
use std::sync::Arc;
use tokio::signal;


#[tokio::main]
async fn main() {
    let db_path = std::env::var("TUSM_DB_PATH").unwrap_or_else(|_| "tusm_indexer.db".to_string());
    let state_db = Arc::new(StateDb::new(&db_path));
    let (pg_tx, mut pg_rx) = tokio::sync::mpsc::channel::<(String, precop_tusm_indexer::state_db::SigmaLeaf, Option<(String, String, u128, String, serde_json::Value, u64)>)>(100);
    
    let api_url = std::env::var("PRECOPSCAN_API_URL").unwrap_or_default();
    let api_key = std::env::var("PRECOPSCAN_API_KEY").unwrap_or_default();
    let bridge_client = if !api_url.is_empty() {
        let max_pending = std::env::var("PRECOPSCAN_BRIDGE_MAX_PENDING").ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(precop_tusm_indexer::bridge_client::DEFAULT_MAX_PENDING);
        precop_tusm_indexer::bridge_client::enable(max_pending);
        println!("[Bridge] Precopscan bridge ENABLED -> {} (max pending {})", api_url, max_pending);
        Some(Arc::new(precop_tusm_indexer::bridge_client::BridgeClient::new(api_url, api_key)))
    } else {
        println!("[Bridge] PRECOPSCAN_API_URL not set: bridge DISABLED (events are not pushed)");
        None
    };

    let (bridge_tx, mut bridge_rx) = tokio::sync::mpsc::unbounded_channel::<precop_tusm_indexer::bridge_client::BridgePayload>();
    let _ = precop_tusm_indexer::BRIDGE_TX.set(bridge_tx.clone());

    tokio::spawn(async move {
        let mut sent: u64 = 0;
        while let Some(payload) = bridge_rx.recv().await {
            if let Some(bridge) = &bridge_client {
                let _ = bridge.push_event(payload).await; // retries until acknowledged (ordered)
                precop_tusm_indexer::bridge_client::mark_delivered();
                sent += 1;
                if sent % 500 == 0 {
                    println!("[Bridge] {} events acknowledged by Precopscan ({} pending, {} dropped)", sent,
                        precop_tusm_indexer::bridge_client::pending(), precop_tusm_indexer::bridge_client::dropped());
                }
            }
        }
    });

    // Legacy Postgres channel: kept for backpressure only. Precopscan bridge events are emitted
    // directly (and in block order) by the scanner thread — see bridge_client::emit.
    tokio::spawn(async move {
        while let Some(_event) = pg_rx.recv().await {}
    });

    let indexer_db = state_db.clone();
    tokio::spawn(async move {
        let mut external_payments = std::collections::HashMap::new();
        let ext_payments_path = std::env::var("TUSM_EXTERNAL_PAYMENTS_PATH").unwrap_or_else(|_| {
            if std::path::Path::new("./data/external_payments.json").exists() {
                "./data/external_payments.json".to_string()
            } else {
                "/app/data/external_payments.json".to_string()
            }
        });
        if let Ok(content) = std::fs::read_to_string(&ext_payments_path) {
            if let Ok(parsed) = serde_json::from_str::<std::collections::HashMap<String, serde_json::Value>>(&content) {
                for (txid, val) in parsed {
                    let asset_opt = val.get("asset").and_then(|v| v.as_str());
                    let amount_opt = val.get("amount").and_then(|v| {
                        v.as_u64().map(|n| n as u128).or_else(|| v.as_str().and_then(|s| s.parse::<u128>().ok()))
                    });
                    if let (Some(asset), Some(amount)) = (asset_opt, amount_opt) {
                        let asset_key = if let Some(rate) = val.get("rate").and_then(|v| {
                            v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok()))
                        }) {
                            format!("{}:{}", asset, rate)
                        } else {
                            asset.to_string()
                        };
                        external_payments.insert(txid, (asset_key, amount));
                    }
                }
            }
        }

        let scanner = ScannerEngine {
            db: indexer_db,
            rpc_url: std::env::var("BITCOIN_RPC_URL").unwrap_or_else(|_| "http://127.0.0.1:8332".to_string()),
            rpc_user: std::env::var("BITCOIN_RPC_USER").unwrap_or_else(|_| "".to_string()),
            rpc_pass: std::env::var("BITCOIN_RPC_PASS").unwrap_or_else(|_| "".to_string()),
            external_payments,
            pg_tx,
            crc21_tx: None,
        };
        scanner.run_trustless_sync().await;
    });

    let app_state = precop_tusm_indexer::api::AppState {
        db: state_db.clone(),
    };

    let app = precop_tusm_indexer::api::router(app_state);

    let port = std::env::var("TUSM_API_PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);
    let server = axum::Server::bind(&addr.parse().unwrap())
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal(state_db.clone()));

    if let Err(e) = server.await { eprintln!("Server error: {}", e); }
}

async fn shutdown_signal(db: Arc<StateDb>) {
    let ctrl_c = async { signal::ctrl_c().await.unwrap_or(()) };
    tokio::select! { _ = ctrl_c => {}, }
    db.flush();
}
