use crate::api_store::ApiBind;
use crate::server::webhooks;
use crate::server::AppState;
use axum::{Json, extract::{Path, State}, http::StatusCode, response::IntoResponse};
use serde_json::{Value, json};
use std::sync::Arc;

pub async fn set_presence(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let chat_id = body
        .get("chatId")
        .and_then(|v| v.as_str())
        .unwrap_or("self")
        .to_string();
    let presence = body
        .get("presence")
        .and_then(|v| v.as_str())
        .unwrap_or("available")
        .to_string();

    let result = state
        .api_store
        .execute(
            "INSERT INTO api_presence (session, chat_id, presence, updated_at) \
             VALUES ($1, $2, $3, now()) \
             ON CONFLICT (session, chat_id) DO UPDATE SET presence = EXCLUDED.presence, updated_at = now()",
            vec![
                ApiBind::Text(session.clone()),
                ApiBind::Text(chat_id.clone()),
                ApiBind::Text(presence.clone()),
            ],
        )
        .await;

    if let Err(err) = result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        );
    }

    webhooks::enqueue(
        &state,
        Some(&session),
        "PRESENCE_UPDATE",
        json!({"chat_id": chat_id, "presence": presence}),
    )
    .await;

    (StatusCode::OK, Json(json!({"status": "ok"})))
}

pub async fn get_presence(
    State(state): State<Arc<AppState>>,
    Path((session, chat_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let rows = state
        .api_store
        .query_json(
            "SELECT row_to_json(api_presence)::jsonb as value FROM api_presence WHERE session = $1 AND chat_id = $2",
            vec![ApiBind::Text(session), ApiBind::Text(chat_id)],
        )
        .await;

    match rows {
        Ok(mut rows) => (
            StatusCode::OK,
            Json(rows.pop().unwrap_or_else(|| json!({}))),
        ),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        ),
    }
}

pub async fn subscribe(
    State(state): State<Arc<AppState>>,
    Path((session, chat_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let result = state
        .api_store
        .execute(
            "INSERT INTO api_presence (session, chat_id, presence, updated_at) \
             VALUES ($1, $2, 'subscribed', now()) \
             ON CONFLICT (session, chat_id) DO UPDATE SET presence = 'subscribed', updated_at = now()",
            vec![ApiBind::Text(session.clone()), ApiBind::Text(chat_id.clone())],
        )
        .await;

    if let Err(err) = result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        );
    }

    webhooks::enqueue(
        &state,
        Some(&session),
        "PRESENCE_UPDATE",
        json!({"chat_id": chat_id, "presence": "subscribed"}),
    )
    .await;

    (StatusCode::OK, Json(json!({"status": "subscribed"})))
}
