use std::collections::HashMap;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
};
use serde_json::{Value, json};

use crate::{domain::common::success, errors::AppError, http::guards, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/webhook/evolution", post(evolution_webhook))
        .route("/webhook/meta", get(meta_webhook_get).post(meta_webhook_post))
        .route("/baileys/{operation}/{instance_name}", post(baileys_operation))
}

async fn evolution_webhook(Json(payload): Json<Value>) -> Result<impl axum::response::IntoResponse, AppError> {
    Ok(success(200, json!({ "status": 200, "event": payload })))
}

async fn meta_webhook_get(Query(query): Query<HashMap<String, String>>) -> Result<impl axum::response::IntoResponse, AppError> {
    let challenge = query
        .get("hub.challenge")
        .cloned()
        .unwrap_or_else(|| "ok".to_string());

    Ok(success(200, json!({ "challenge": challenge })))
}

async fn meta_webhook_post(Json(payload): Json<Value>) -> Result<impl axum::response::IntoResponse, AppError> {
    Ok(success(200, json!({ "status": 200, "event": payload })))
}

async fn baileys_operation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((operation, instance_name)): Path<(String, String)>,
    Json(payload): Json<Value>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    guards::authorize(&state, &headers, "/baileys/{operation}/{instance_name}", Some(&instance_name)).await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;

    let response = state
        .sidecar
        .send_message(&instance_name, &operation, payload.to_string())
        .await?;

    Ok(success(200, json!({ "status": 200, "message": response.message })))
}
