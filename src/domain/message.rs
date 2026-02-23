use std::collections::HashSet;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::post,
};
use serde_json::{Value, json};

use crate::{
    domain::common::success,
    errors::AppError,
    http::guards,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new().route("/:operation/:instance_name", post(send_operation))
}

async fn send_operation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((operation, instance_name)): Path<(String, String)>,
    Json(payload): Json<Value>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    guards::authorize(&state, &headers, "/message/{operation}/{instance_name}", Some(&instance_name)).await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;

    let allowed = allowed_operations();
    if !allowed.contains(operation.as_str()) {
        return Err(AppError::not_found(format!("Unknown message operation: {operation}")));
    }

    let response = state
        .sidecar
        .send_message(&instance_name, &operation, payload.to_string())
        .await?;

    Ok(success(
        201,
        json!({
            "status": 201,
            "message": response.message,
            "response": parse_payload(response.payload_json),
        }),
    ))
}

fn allowed_operations() -> HashSet<&'static str> {
    [
        "sendTemplate",
        "sendText",
        "sendMedia",
        "sendPtv",
        "sendWhatsAppAudio",
        "sendStatus",
        "sendSticker",
        "sendLocation",
        "sendContact",
        "sendReaction",
        "sendPoll",
        "sendList",
        "sendButtons",
    ]
    .into_iter()
    .collect()
}

fn parse_payload(payload_json: String) -> Value {
    if payload_json.is_empty() {
        json!({})
    } else {
        serde_json::from_str::<Value>(&payload_json).unwrap_or_else(|_| json!({ "raw": payload_json }))
    }
}
