use crate::api_store::ApiBind;
use crate::server::AppState;
use axum::{Json, extract::{Path, State}, http::StatusCode, response::IntoResponse};
use serde_json::{Value, json};
use std::sync::Arc;

async fn convert_media(
    state: Arc<AppState>,
    session: String,
    media_type: &str,
    payload: Value,
) -> impl IntoResponse {
    let result = state
        .api_store
        .execute(
            "INSERT INTO api_events (session, event, payload, created_at) \
             VALUES ($1, $2, $3, now())",
            vec![
                ApiBind::Text(session),
                ApiBind::Text(format!("MEDIA_CONVERT_{}", media_type.to_uppercase())),
                ApiBind::Json(payload),
            ],
        )
        .await;

    match result {
        Ok(_) => (StatusCode::OK, Json(json!({"status": "queued"}))),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        ),
    }
}

pub async fn convert_voice(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    convert_media(state, session, "voice", body).await
}

pub async fn convert_video(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    convert_media(state, session, "video", body).await
}
