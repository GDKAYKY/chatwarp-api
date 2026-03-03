use crate::api_store::ApiBind;
use crate::server::AppState;
use axum::{Json, extract::{Path, State}, http::StatusCode, response::IntoResponse};
use serde_json::{Value, json};
use std::sync::Arc;

pub async fn list_channels(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
) -> impl IntoResponse {
    match state
        .api_store
        .query_json(
            "SELECT row_to_json(api_channels)::jsonb as value FROM api_channels WHERE session = $1",
            vec![ApiBind::Text(session)],
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

pub async fn follow_channel(
    State(state): State<Arc<AppState>>,
    Path((session, id)): Path<(String, String)>,
) -> impl IntoResponse {
    let result = state
        .api_store
        .execute(
            "INSERT INTO api_channels (session, id, followed) VALUES ($1, $2, true) \
             ON CONFLICT (session, id) DO UPDATE SET followed = true",
            vec![ApiBind::Text(session), ApiBind::Text(id)],
        )
        .await;

    match result {
        Ok(_) => (StatusCode::OK, Json(json!({"status": "followed"}))),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        ),
    }
}

pub async fn search_by_text(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let query = body
        .get("query")
        .or_else(|| body.get("q"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let rows = state
        .api_store
        .query_json(
            "SELECT row_to_json(api_channels)::jsonb as value \
             FROM api_channels WHERE session = $1 AND title ILIKE $2",
            vec![
                ApiBind::Text(session),
                ApiBind::Text(format!("%{}%", query)),
            ],
        )
        .await;

    match rows {
        Ok(rows) => (StatusCode::OK, Json(json!(rows))),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        ),
    }
}
