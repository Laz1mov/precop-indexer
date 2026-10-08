use std::net::SocketAddr;
use std::sync::Arc;
use axum::{
    extract::{Path, State, Json},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde_json::json;
use tower_http::cors::{Any, CorsLayer};
use tusm_sidecar::{SidecarConfig, SidecarEngine, VerifyRequest, VerifyResponse};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = SidecarConfig::default();
    let port = config.http_port;
    let engine = Arc::new(SidecarEngine::new(config));

    // Spawn background streaming / pending mints poller
    let engine_loop = Arc::clone(&engine);
    tokio::spawn(async move {
        engine_loop.run_loop().await;
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/v1/payments", get(list_payments_handler))
        .route("/v1/payments/:txid", get(get_payment_handler))
        .route("/v1/verify", post(verify_payment_handler))
        .layer(cors)
        .with_state(Arc::clone(&engine));

    // PRECOP-06 FIX: Bind to 127.0.0.1 by default (configurable via SIDECAR_HOST)
    let host_str = std::env::var("SIDECAR_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let ip: std::net::IpAddr = host_str.parse().unwrap_or_else(|_| std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)));
    let addr = SocketAddr::from((ip, port));
    println!("[Rust Sidecar] HTTP API listening on http://{}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

async fn health_handler(State(engine): State<Arc<SidecarEngine>>) -> impl IntoResponse {
    let count = engine.payments.read().len();
    Json(json!({
        "status": "ok",
        "service": "precop-sidecar-rust",
        "payments_count": count,
        "treasury": engine.config.treasury_address,
    }))
}

async fn list_payments_handler(State(engine): State<Arc<SidecarEngine>>) -> impl IntoResponse {
    let payments = engine.payments.read().clone();
    Json(payments)
}

async fn get_payment_handler(
    State(engine): State<Arc<SidecarEngine>>,
    Path(txid): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let payments = engine.payments.read();
    if let Some(payment) = payments.get(&txid) {
        Ok(Json(payment.clone()))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn verify_payment_handler(
    headers: HeaderMap,
    State(engine): State<Arc<SidecarEngine>>,
    Json(req): Json<VerifyRequest>,
) -> impl IntoResponse {
    // PRECOP-06 FIX: Enforce optional Bearer token auth if SIDECAR_AUTH_TOKEN is configured
    if let Ok(expected_token) = std::env::var("SIDECAR_AUTH_TOKEN") {
        if !expected_token.is_empty() {
            let auth_header = headers.get("authorization").and_then(|h| h.to_str().ok());
            let authorized = match auth_header {
                Some(header_val) if header_val.starts_with("Bearer ") => {
                    let token = &header_val[7..];
                    token == expected_token
                }
                _ => false,
            };
            if !authorized {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(VerifyResponse {
                        verified: false,
                        entry: None,
                        message: Some("Unauthorized: Invalid or missing Bearer token".to_string()),
                    }),
                );
            }
        }
    }

    let txid = req.txid.trim().to_string();
    if txid.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(VerifyResponse {
                verified: false,
                entry: None,
                message: Some("Empty txid provided".to_string()),
            }),
        );
    }

    // Check cache first
    if let Some(cached) = engine.payments.read().get(&txid).cloned() {
        return (
            StatusCode::OK,
            Json(VerifyResponse {
                verified: true,
                entry: Some(cached),
                message: Some("Payment already verified (cache)".to_string()),
            }),
        );
    }

    // Run verification on-demand
    if let Some(verified) = engine.verify_polymorphic_payment(&txid).await {
        engine.payments.write().insert(txid.clone(), verified.clone());
        engine.save_files();
        (
            StatusCode::OK,
            Json(VerifyResponse {
                verified: true,
                entry: Some(verified),
                message: Some("Payment successfully verified and persisted".to_string()),
            }),
        )
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(VerifyResponse {
                verified: false,
                entry: None,
                message: Some(format!("Transaction {} does not deliver valid DOG or ORDI payment to treasury", txid)),
            }),
        )
    }
}
