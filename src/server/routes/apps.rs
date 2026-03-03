use crate::api_store::ApiBind;
use crate::server::AppState;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde_json::{Value, json};
use std::sync::Arc;

pub async fn list_apps(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state
        .api_store
        .query_json("SELECT row_to_json(api_apps)::jsonb as value FROM api_apps", vec![])
        .await
    {
        Ok(rows) => (StatusCode::OK, Json(json!(rows))),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        ),
    }
}

pub async fn create_app(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let name = body
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let config = body.get("config").cloned().unwrap_or(json!({}));

    let result = state
        .api_store
        .execute(
            "INSERT INTO api_apps (name, config, created_at) VALUES ($1, $2, now())",
            vec![ApiBind::NullableText(name), ApiBind::Json(config)],
        )
        .await;

    match result {
        Ok(_) => (StatusCode::OK, Json(json!({"status": "created"}))),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        ),
    }
}
