use crate::api_store::ApiBind;
use crate::server::webhooks;
use crate::server::AppState;
use axum::{Json, extract::{Path, State}, http::StatusCode, response::IntoResponse};
use serde_json::{Value, json};
use std::sync::Arc;

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
