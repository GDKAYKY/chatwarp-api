use crate::api_store::ApiBind;
use crate::server::webhooks;
use crate::server::AppState;
use axum::{Json, extract::{Path, State}, http::StatusCode, response::IntoResponse};
use serde_json::json;
use std::sync::Arc;
use tracing::info;

pub async fn list_chats(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
) -> impl IntoResponse {
    info!(session = %session, "Listando conversas");
    let session_name = session.clone();
    let rows = state
        .api_store
        .query_json(
            "SELECT row_to_json(api_chats)::jsonb as value FROM api_chats WHERE session = $1 ORDER BY last_message_at DESC",
            vec![ApiBind::Text(session)],
        )
        .await;

    match rows {
        Ok(rows) => {
            webhooks::enqueue(&state, Some(&session_name), "CHATS_SET", json!({"count": rows.len()})).await;
            (StatusCode::OK, Json(json!(rows)))
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        ),
    }
}

pub async fn overview(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
) -> impl IntoResponse {
    let rows = state
        .api_store
        .query_json(
            "SELECT jsonb_build_object( \
                'total', COUNT(*), \
                'last_message_at', MAX(last_message_at), \
                'unread_total', COALESCE(SUM(unread_count),0) \
            ) as value FROM api_chats WHERE session = $1",
            vec![ApiBind::Text(session)],
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

pub async fn messages(
    State(state): State<Arc<AppState>>,
    Path((session, chat_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let rows = state
        .api_store
        .query_json(
            "SELECT row_to_json(api_messages)::jsonb as value \
             FROM api_messages WHERE session = $1 AND chat_id = $2 \
             ORDER BY created_at DESC",
            vec![ApiBind::Text(session), ApiBind::Text(chat_id)],
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

pub async fn read_messages(
    State(state): State<Arc<AppState>>,
    Path((session, chat_id)): Path<(String, String)>,
) -> impl IntoResponse {
    info!(session = %session, chat_id = %chat_id, "Marcando mensagens como lidas");
    let result = state
        .api_store
        .execute(
            "UPDATE api_messages SET status = 'read' WHERE session = $1 AND chat_id = $2",
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
        "MESSAGES_UPDATE",
        json!({"chat_id": chat_id}),
    )
    .await;

    (StatusCode::OK, Json(json!({"status": "read"})))
}
