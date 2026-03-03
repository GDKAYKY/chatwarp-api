use crate::api_store::ApiBind;
use crate::server::AppState;
use crate::server::routes::helpers::{chat_id_from_body, session_from_body};
use crate::server::webhooks;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

async fn insert_message(
    state: &AppState,
    session: &str,
    chat_id: Option<String>,
    message_type: &str,
    payload: Value,
    status: &str,
) -> anyhow::Result<Value> {
    let rows = state
        .api_store
        .query_json(
            "WITH t AS ( \
                INSERT INTO api_messages (session, chat_id, from_me, message_type, payload, status) \
                VALUES ($1, $2, $3, $4, $5, $6) \
                RETURNING id, session, chat_id, message_type, status, created_at \
            ) SELECT row_to_json(t)::jsonb as value FROM t",
            vec![
                ApiBind::Text(session.to_string()),
                ApiBind::NullableText(chat_id),
                ApiBind::Bool(true),
                ApiBind::Text(message_type.to_string()),
                ApiBind::Json(payload),
                ApiBind::Text(status.to_string()),
            ],
        )
        .await?;

    Ok(rows.into_iter().next().unwrap_or_else(|| json!({})))
}

async fn list_messages(
    state: &AppState,
    session: &str,
    chat_id: Option<String>,
    limit: i64,
    offset: i64,
) -> anyhow::Result<Vec<Value>> {
    let sql = if chat_id.is_some() {
        "SELECT row_to_json(api_messages)::jsonb as value \
         FROM api_messages \
         WHERE session = $1 AND chat_id = $2 \
         ORDER BY created_at DESC \
         LIMIT $3 OFFSET $4"
    } else {
        "SELECT row_to_json(api_messages)::jsonb as value \
         FROM api_messages \
         WHERE session = $1 \
         ORDER BY created_at DESC \
         LIMIT $3 OFFSET $4"
    };

    let mut binds = vec![ApiBind::Text(session.to_string())];
    if let Some(chat_id) = chat_id {
        binds.push(ApiBind::Text(chat_id));
    }
    binds.push(ApiBind::Int(limit as i32));
    binds.push(ApiBind::Int(offset as i32));

    Ok(state.api_store.query_json(sql, binds).await?)
}

pub async fn send_text(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    send_message_type(state, body, "text", true).await
}

pub async fn send_image(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    send_message_type(state, body, "image", true).await
}

pub async fn send_file(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    send_message_type(state, body, "file", true).await
}

pub async fn send_voice(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    send_message_type(state, body, "voice", true).await
}

pub async fn send_video(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    send_message_type(state, body, "video", true).await
}

pub async fn send_buttons(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    send_message_type(state, body, "buttons", true).await
}

pub async fn send_list(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    send_message_type(state, body, "list", true).await
}

pub async fn send_poll(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    send_message_type(state, body, "poll", true).await
}

pub async fn send_poll_vote(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    send_message_type(state, body, "poll_vote", true).await
}

pub async fn send_link_custom_preview(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    send_message_type(state, body, "link_custom_preview", true).await
}

pub async fn send_contact_vcard(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    send_message_type(state, body, "contact_vcard", true).await
}

pub async fn send_location(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    send_message_type(state, body, "location", false).await
}

async fn send_message_type(
    state: Arc<AppState>,
    body: Value,
    message_type: &str,
    send_event: bool,
) -> impl IntoResponse {
    let session = session_from_body(&body);
    let chat_id = chat_id_from_body(&body);

    match insert_message(
        &state,
        &session,
        chat_id.clone(),
        message_type,
        body.clone(),
        "queued",
    )
    .await
    {
        Ok(message) => {
            webhooks::enqueue(
                &state,
                Some(&session),
                "MESSAGES_UPSERT",
                json!({"message": message.clone()}),
            )
            .await;

            if send_event {
                webhooks::enqueue(
                    &state,
                    Some(&session),
                    "SEND_MESSAGE",
                    json!({"message": message.clone()}),
                )
                .await;
            }

            (StatusCode::OK, Json(message))
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        ),
    }
}

pub async fn send_seen(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let session = session_from_body(&body);
    let message_id = body
        .get("message_id")
        .or_else(|| body.get("messageId"))
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());

    let Some(message_id) = message_id else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "message_id_required"})),
        );
    };

    let result = state
        .api_store
        .execute(
            "UPDATE api_messages SET status = 'seen' WHERE id = $1",
            vec![ApiBind::Uuid(message_id)],
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
        json!({"id": message_id}),
    )
    .await;

    (
        StatusCode::OK,
        Json(json!({"status": "seen", "id": message_id})),
    )
}

pub async fn start_typing(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    set_typing(state, body, "typing").await
}

pub async fn stop_typing(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    set_typing(state, body, "idle").await
}

async fn set_typing(state: Arc<AppState>, body: Value, presence: &str) -> impl IntoResponse {
    let session = session_from_body(&body);
    let chat_id = chat_id_from_body(&body).unwrap_or_else(|| "self".to_string());

    let result = state
        .api_store
        .execute(
            "INSERT INTO api_presence (session, chat_id, presence, updated_at) \
             VALUES ($1, $2, $3, now()) \
             ON CONFLICT (session, chat_id) DO UPDATE SET presence = EXCLUDED.presence, updated_at = now()",
            vec![
                ApiBind::Text(session.clone()),
                ApiBind::Text(chat_id.clone()),
                ApiBind::Text(presence.to_string()),
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

    (StatusCode::OK, Json(json!({"status": presence})))
}

pub async fn list_messages_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let session = params
        .get("session")
        .cloned()
        .unwrap_or_else(|| "default".to_string());
    let chat_id = params.get("chatId").cloned();
    let limit = params
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(50);
    let offset = params
        .get("offset")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);

    match list_messages(&state, &session, chat_id, limit, offset).await {
        Ok(rows) => (StatusCode::OK, Json(json!(rows))),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        ),
    }
}

pub async fn reaction(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    update_message_payload(state, body, "reaction").await
}

pub async fn star(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    update_message_payload(state, body, "starred").await
}

async fn update_message_payload(
    state: Arc<AppState>,
    body: Value,
    field: &str,
) -> impl IntoResponse {
    let session = session_from_body(&body);
    let message_id = body
        .get("message_id")
        .or_else(|| body.get("messageId"))
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());

    let Some(message_id) = message_id else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "message_id_required"})),
        );
    };

    let patch_value = body.get(field).cloned().unwrap_or(json!(true));

    let result = state
        .api_store
        .execute(
            "UPDATE api_messages \
             SET payload = COALESCE(payload, '{}'::jsonb) || jsonb_build_object($2, $3) \
             WHERE id = $1",
            vec![
                ApiBind::Uuid(message_id),
                ApiBind::Text(field.to_string()),
                ApiBind::Json(patch_value),
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
        "MESSAGES_UPDATE",
        json!({"id": message_id, "field": field}),
    )
    .await;

    (
        StatusCode::OK,
        Json(json!({"status": "updated", "id": message_id})),
    )
}

pub async fn forward_message(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    send_message_type(state, body, "forward", false).await
}

pub async fn reply_message(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    send_message_type(state, body, "reply", false).await
}
