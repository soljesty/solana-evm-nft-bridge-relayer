use axum::{
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use requests::AppState;
use serde_json::json;
use tower_http::cors::{Any, CorsLayer};

use crate::{
    block_explorers, completed_requests, new_brige_from_evm, new_brige_from_solana,
    pending_requests, request_data,
};

pub fn api_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route(
            "/healthcheck",
            get(|| async { (StatusCode::OK, Json(json!({"running": true}))) }),
        )
        .route("/bridge/evm-to-solana", post(new_brige_from_evm))
        .route("/bridge/solana-to-evm", post(new_brige_from_solana))
        .route("/bridge/pending-requests", get(pending_requests))
        .route("/bridge/completed-requests", get(completed_requests))
        .route("/bridge/requests/{id}", get(request_data))
        .route("/bridge/block_explorers", get(block_explorers))
        .with_state(state)
        .layer(cors);

    app
}
