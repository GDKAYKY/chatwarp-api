use crate::api_store::ApiBind;
use crate::server::webhooks;
use crate::server::AppState;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::{Value, json};
use std::sync::Arc;

pub async fn get_events(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let event_type = params.get("type").cloned();
    let limit = params
        .get("limit")
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(50);
    let offset = params
        .get("offset")
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0);

    let (sql, binds) = if let Some(t) = event_type {
        (
            "SELECT id, session, event, payload, created_at \
             FROM api_events \
             WHERE session = $1 AND event = $2 \
             ORDER BY created_at DESC \
             LIMIT $3 OFFSET $4",
            vec![
                ApiBind::Text(session),
                ApiBind::Text(t),
                ApiBind::Int(limit),
                ApiBind::Int(offset),
            ],
        )
    } else {
        (
            "SELECT id, session, event, payload, created_at \
             FROM api_events \
             WHERE session = $1 \
             ORDER BY created_at DESC \
             LIMIT $2 OFFSET $3",
            vec![
                ApiBind::Text(session),
                ApiBind::Int(limit),
                ApiBind::Int(offset),
            ],
        )
    };

    match state.api_store.query_json(sql, binds).await {
        Ok(rows) => (StatusCode::OK, Json(json!({ "events": rows }))),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        ),
    }
}

pub async fn post_event(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let event = body
        .get("event")
        .and_then(|v| v.as_str())
        .unwrap_or("APPLICATION_STARTUP")
        .to_string();

    let result = state
        .api_store
        .execute(
            "INSERT INTO api_events (session, event, payload, created_at) VALUES ($1, $2, $3, now())",
            vec![
                ApiBind::Text(session.clone()),
                ApiBind::Text(event.clone()),
                ApiBind::Json(body.clone()),
            ],
        )
        .await;

    if let Err(err) = result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        );
    }

    webhooks::enqueue(&state, Some(&session), &event, body).await;

    (StatusCode::OK, Json(json!({"status": "ok"})))
}
