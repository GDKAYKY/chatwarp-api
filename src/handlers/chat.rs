use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use crate::{app::AppState, instance::InstanceError};

#[derive(Debug, Deserialize)]
struct FindMessagesRequest {
    remote_jid: String,
    limit: Option<u16>,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    id: String,
    from_me: bool,
    body: String,
    timestamp: u64,
}

#[derive(Debug, Serialize)]
struct FindMessagesResponse {
    instance: String,
    remote_jid: String,
    messages: Vec<ChatMessage>,
    count: usize,
}

#[derive(Debug, Serialize)]
struct ChatSummary {
    jid: String,
    name: String,
    unread: u32,
}

#[derive(Debug, Serialize)]
struct FindChatsResponse {
    instance: String,
    chats: Vec<ChatSummary>,
}

#[derive(Debug, Serialize)]
struct ChatErrorResponse {
    error: &'static str,
    message: String,
}

/// Handles synthetic lookup for chat history messages.
pub(crate) async fn find_messages_handler(
    State(state): State<AppState>,
    Path(instance_name): Path<String>,
    Json(request): Json<FindMessagesRequest>,
) -> axum::response::Response {
    let manager = state.instance_manager();
    if manager.get(&instance_name).await.is_none() {
        return map_instance_error(InstanceError::NotFound);
    }

    let remote_jid = request.remote_jid.trim();
    if remote_jid.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ChatErrorResponse {
                error: "invalid_remote_jid",
                message: "remote_jid cannot be empty".to_owned(),
            }),
        )
            .into_response();
    }

    let message_count = usize::from(request.limit.unwrap_or(20).clamp(1, 100));
    let now = unix_timestamp_secs();
    let messages = (0..message_count)
        .map(|idx| ChatMessage {
            id: format!("{}-{idx:04}", instance_name),
            from_me: idx % 2 == 0,
            body: format!("synthetic message #{idx} for {remote_jid}"),
            timestamp: now.saturating_sub(idx as u64),
        })
        .collect::<Vec<_>>();

    (
        StatusCode::OK,
        Json(FindMessagesResponse {
            instance: instance_name,
            remote_jid: remote_jid.to_owned(),
            count: messages.len(),
            messages,
        }),
    )
        .into_response()
}

/// Returns a synthetic chat roster for an existing instance.
pub(crate) async fn find_chats_handler(
    State(state): State<AppState>,
    Path(instance_name): Path<String>,
) -> axum::response::Response {
    let manager = state.instance_manager();
    if manager.get(&instance_name).await.is_none() {
        return map_instance_error(InstanceError::NotFound);
    }

    let groups = state.group_store().list(&instance_name).await;
    let mut chats = vec![
        ChatSummary {
            jid: "111@s.whatsapp.net".to_owned(),
            name: "Synthetic Contact".to_owned(),
            unread: 0,
        },
        ChatSummary {
            jid: "status@broadcast".to_owned(),
            name: "Status".to_owned(),
            unread: 2,
        },
    ];

    for group in groups {
        chats.push(ChatSummary {
            jid: group.id,
            name: group.subject,
            unread: 0,
        });
    }

    (
        StatusCode::OK,
        Json(FindChatsResponse {
            instance: instance_name,
            chats,
        }),
    )
        .into_response()
}

fn map_instance_error(error: InstanceError) -> axum::response::Response {
    match error {
        InstanceError::AlreadyExists => (
            StatusCode::CONFLICT,
            Json(ChatErrorResponse {
                error: "instance_already_exists",
                message: "instance already exists".to_owned(),
            }),
        )
            .into_response(),
        InstanceError::NotFound => (
            StatusCode::NOT_FOUND,
            Json(ChatErrorResponse {
                error: "instance_not_found",
                message: "instance not found".to_owned(),
            }),
        )
            .into_response(),
        InstanceError::CommandChannelClosed => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ChatErrorResponse {
                error: "instance_unavailable",
                message: "instance command channel is unavailable".to_owned(),
            }),
        )
            .into_response(),
    }
}

fn unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |value| value.as_secs())
}
