use std::net::SocketAddr;
use std::sync::Arc;
use axum::{
    extract::{Path, State, Json},
    http::StatusCode,
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
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/v1/payments", get(list_payments_handler))
        .route("/v1/payments/:txid", get(get_payment_handler))
        .route("/v1/verify", post(verify_payment_handler))
        .layer(cors)
        .with_state(Arc::clone(&engine));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
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
    State(engine): State<Arc<SidecarEngine>>,
    Json(req): Json<VerifyRequest>,
) -> impl IntoResponse {
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
                message: Some(format!("Transaction {} does not contain a valid DOG or ORDI payment", txid)),
            }),
        )
    }
}
