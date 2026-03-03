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
use uuid::Uuid;

pub async fn list_labels(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
) -> impl IntoResponse {
    match state
        .api_store
        .query_json(
            "SELECT row_to_json(api_labels)::jsonb as value FROM api_labels WHERE session = $1",
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

pub async fn create_label(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let id = body
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let name = body
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let color = body
        .get("color")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let result = state
        .api_store
        .execute(
            "INSERT INTO api_labels (session, id, name, color) VALUES ($1, $2, $3, $4) \
             ON CONFLICT (session, id) DO UPDATE SET name = EXCLUDED.name, color = EXCLUDED.color",
            vec![
                ApiBind::Text(session.clone()),
                ApiBind::Text(id.clone()),
                ApiBind::NullableText(name),
                ApiBind::NullableText(color),
            ],
        )
        .await;

    if let Err(err) = result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        );
    }

    webhooks::enqueue(&state, Some(&session), "LABELS_EDIT", json!({"id": id})).await;

    (StatusCode::OK, Json(json!({"id": id})))
}

pub async fn apply_label(
    State(state): State<Arc<AppState>>,
    Path((session, chat_id)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let label_id = body
        .get("label_id")
        .or_else(|| body.get("labelId"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let Some(label_id) = label_id else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "label_id_required"})),
        );
    };

    let result = state
        .api_store
        .execute(
            "INSERT INTO api_label_chats (session, label_id, chat_id) VALUES ($1, $2, $3) \
             ON CONFLICT (session, label_id, chat_id) DO NOTHING",
            vec![
                ApiBind::Text(session.clone()),
                ApiBind::Text(label_id.clone()),
                ApiBind::Text(chat_id.clone()),
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
        "LABELS_ASSOCIATION",
        json!({"label_id": label_id, "chat_id": chat_id}),
    )
    .await;

    (StatusCode::OK, Json(json!({"status": "ok"})))
}

pub async fn chats_by_label(
    State(state): State<Arc<AppState>>,
    Path((session, label_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let rows = state
        .api_store
        .query_json(
            "SELECT jsonb_build_object('chat_id', chat_id) as value \
             FROM api_label_chats WHERE session = $1 AND label_id = $2",
            vec![ApiBind::Text(session), ApiBind::Text(label_id)],
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
