use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::post,
};
use serde_json::{Value, json};

use crate::{domain::common::success, errors::AppError, http::guards, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/offer/{instance_name}", post(call_offer))
}

async fn call_offer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(instance_name): Path<String>,
    Json(payload): Json<Value>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    guards::authorize(&state, &headers, "/call/offer/{instance_name}", Some(&instance_name)).await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;

    let response = state
        .sidecar
        .send_message(&instance_name, "offer", payload.to_string())
        .await?;

    Ok(success(200, json!({ "status": 200, "message": response.message })))
}
