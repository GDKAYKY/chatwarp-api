use crate::server::AppState;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde_json::json;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn health() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"ok": true})))
}

pub async fn ping() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"ok": true, "latency_ms": 0})))
}

pub async fn server_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let count = state
        .api_store
        .query_json(
            "SELECT jsonb_build_object('total_sessions', COUNT(*)) as value FROM api_sessions",
            vec![],
        )
        .await
        .ok()
        .and_then(|mut rows| rows.pop())
        .unwrap_or_else(|| json!({"total_sessions": 0}));

    let uptime = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    (StatusCode::OK, Json(json!({"status": "ok", "uptime": uptime, "stats": count})))
}
