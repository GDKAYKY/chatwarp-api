use crate::api_store::ApiBind;
use crate::server::AppState;
use crate::server::webhooks;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::{Value, json};
use std::sync::Arc;

pub async fn reject_call(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let result = state
        .api_store
        .execute(
            "INSERT INTO api_events (session, event, payload, created_at) VALUES ($1, $2, $3, now())",
            vec![
                ApiBind::Text(session.clone()),
                ApiBind::Text("CALL".to_string()),
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

    webhooks::enqueue(&state, Some(&session), "CALL", body).await;

    (StatusCode::OK, Json(json!({"status": "rejected"})))
}
