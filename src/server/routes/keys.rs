use crate::api_store::ApiBind;
use crate::server::AppState;
use axum::{Json, extract::{Path, State}, http::StatusCode, response::IntoResponse};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

fn hash_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}

pub async fn create_key(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let label = body.get("label").and_then(|v| v.as_str()).map(|s| s.to_string());
    let raw_key = Uuid::new_v4().to_string();
    let key_hash = hash_key(&raw_key);

    let result = state
        .api_store
        .execute(
            "INSERT INTO api_keys (id, label, key_hash, created_at) VALUES ($1, $2, $3, now())",
            vec![
                ApiBind::Uuid(Uuid::parse_str(&raw_key).unwrap_or_else(|_| Uuid::new_v4())),
                ApiBind::NullableText(label),
                ApiBind::Text(key_hash),
            ],
        )
        .await;

    match result {
        Ok(_) => (StatusCode::OK, Json(json!({"key": raw_key}))),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        ),
    }
}

pub async fn list_keys(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state
        .api_store
        .query_json(
            "SELECT row_to_json(api_keys)::jsonb as value FROM api_keys ORDER BY created_at DESC",
            vec![],
        )
        .await
    {
        Ok(rows) => (StatusCode::OK, Json(json!(rows))),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        ),
    }
}

pub async fn revoke_key(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let id = Uuid::parse_str(&id).ok();
    let Some(id) = id else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_id"})),
        );
    };

    let result = state
        .api_store
        .execute(
            "UPDATE api_keys SET revoked_at = now() WHERE id = $1",
            vec![ApiBind::Uuid(id)],
        )
        .await;

    match result {
        Ok(_) => (StatusCode::OK, Json(json!({"status": "revoked"}))),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        ),
    }
}
