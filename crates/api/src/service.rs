use axum::{
    extract::{Path, State},
    http::Uri,
    Json,
};
use log::error;
use requests::{
    endpoints::{get_pending_requests, get_request, new_request},
    get_completed_requests, AppState,
};
use serde_json::{json, Value};
use types::{BRequest, Chains, EVMInputRequest, InputRequest, SolanaInputRequest};

pub async fn new_brige_from_solana(
    uri: Uri,
    State(state): State<AppState>,
    Json(input): Json<SolanaInputRequest>,
) -> Result<Json<BRequest>, (axum::http::StatusCode, Json<Value>)> {
    new_brige_request(uri, state, input.into()).await
}

pub async fn new_brige_from_evm(
    uri: Uri,
    State(state): State<AppState>,
    Json(input): Json<EVMInputRequest>,
) -> Result<Json<BRequest>, (axum::http::StatusCode, Json<Value>)> {
    new_brige_request(uri, state, input.into()).await
}

async fn new_brige_request(
    uri: Uri,
    state: AppState,
    input: InputRequest,
) -> Result<Json<BRequest>, (axum::http::StatusCode, Json<Value>)> {
    let is_invalid_route = match (uri.to_string().as_str(), &input.origin_network) {
        ("/bridge/evm-to-solana", Chains::SOLANA) => true,
        ("/bridge/solana-to-evm", Chains::EVM) => true,
        _ => false,
    };

    if is_invalid_route {
        let error = format!(
            "Invalid endpoint/chain combination: path={} origin_network={:?}",
            uri, &input.origin_network
        );
        error!("{}", error);
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            Json(json!({ "error": error })),
        ));
    }

    match new_request(input.clone().into(), state).await {
        Ok(request) => Ok(Json(request)),
        Err(e) => {
            error!("AppState error: {e}");
            Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            ))
        }
    }
}

pub async fn pending_requests(
    State(state): State<AppState>,
) -> Result<Json<Vec<String>>, axum::http::StatusCode> {
    match get_pending_requests(&state.db) {
        Some(requests_ids) => Ok(Json(requests_ids)),
        None => Ok(Json(vec![String::new()])),
    }
}

pub async fn request_data(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<BRequest>, axum::http::StatusCode> {
    match get_request(&id, &state.db) {
        Ok(Some(request)) => Ok(Json(request)),
        _ => Err(axum::http::StatusCode::NOT_FOUND),
    }
}

pub async fn block_explorers(
    State(state): State<AppState>,
) -> Result<Json<Value>, axum::http::StatusCode> {
    if state.evm_client.block_explorer != String::default()
        && state.solana_client.block_explorer != String::default()
    {
        Ok(
            json!({"EVM": state.evm_client.block_explorer, "SOLANA": state.solana_client.block_explorer}).into(),
        )
    } else {
        Err(axum::http::StatusCode::NOT_FOUND)
    }
}

pub async fn completed_requests(
    State(state): State<AppState>,
) -> Result<Json<Vec<String>>, axum::http::StatusCode> {
    match get_completed_requests(&state.db) {
        Some(requests_ids) => Ok(Json(requests_ids)),
        None => Ok(Json(vec![String::new()])),
    }
}
