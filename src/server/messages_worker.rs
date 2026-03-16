use crate::api_store::ApiBind;
use crate::client::Client;
use crate::http::HttpRequest;
use crate::server::queue::MessageQueue;
use crate::server::AppState;
use base64::Engine as _;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{Duration, sleep};
use tokio::sync::mpsc;
use uuid::Uuid;
use waproto::whatsapp as wa;
use warp_core::download::MediaType;
use warp_core_binary::jid::Jid;

pub async fn spawn_messages_worker(app_state: Arc<AppState>, mut message_rx: mpsc::Receiver<()>) {
    const SESSION_WAIT_TTL_MINUTES: i64 = 10;
    const POLL_FALLBACK_SECONDS: u64 = 5;
    let queue = MessageQueue::new(app_state.clone());

    loop {
        let processed_any = match drain_message_batch(&app_state, &queue, SESSION_WAIT_TTL_MINUTES).await {
            Ok(processed_any) => processed_any,
            Err(err) => {
                log::error!("Error processing queued messages: {}", err);
                sleep(Duration::from_secs(5)).await;
                false
            }
        };

        if processed_any {
            continue;
        }

        tokio::select! {
            _ = message_rx.recv() => {}
            _ = sleep(Duration::from_secs(POLL_FALLBACK_SECONDS)) => {}
        }
    }
}

async fn mark_status(state: &AppState, id: Uuid, status: &str) -> anyhow::Result<()> {
    state
        .api_store
        .execute(
            "UPDATE api_messages SET status = $1 WHERE id = $2",
            vec![ApiBind::Text(status.to_string()), ApiBind::Uuid(id)],
        )
        .await
        .map(|_| ())
}

fn should_fail_missing_session(created_at: Option<DateTime<Utc>>, ttl_minutes: i64) -> bool {
    let Some(created_at) = created_at else {
        return false;
    };

    Utc::now().signed_duration_since(created_at) > chrono::Duration::minutes(ttl_minutes)
}

async fn drain_message_batch(
    app_state: &Arc<AppState>,
    queue: &MessageQueue,
    session_wait_ttl_minutes: i64,
) -> anyhow::Result<bool> {
    let sessions: Vec<String> = app_state
        .clients
        .iter()
        .map(|entry| entry.key().clone())
        .collect();

    if sessions.is_empty() {
        return Ok(false);
    }

    let jobs = queue.claim_for_sessions(sessions, 50).await?;

    if jobs.is_empty() {
        return Ok(false);
    }

    let mut by_session: HashMap<String, Vec<Value>> = HashMap::new();
    for job in jobs {
        by_session
            .entry(job.session.clone())
            .or_default()
            .push(serde_json::json!({
                "id": job.id.to_string(),
                "session": job.session,
                "chat_id": job.chat_id,
                "message_type": job.message_type,
                "payload": job.payload,
                "created_at": job.created_at.map(|d| d.to_rfc3339()),
            }));
    }

    let mut join_set = tokio::task::JoinSet::new();
    for (session, rows) in by_session {
        let state = app_state.clone();
        join_set.spawn(async move {
            for row in rows {
                process_single_message(&state, &session, row, session_wait_ttl_minutes).await;
            }
        });
    }

    while let Some(result) = join_set.join_next().await {
        if let Err(err) = result {
            log::error!("Message worker task failed: {}", err);
        }
    }

    Ok(true)
}

async fn process_single_message(
    app_state: &Arc<AppState>,
    session: &str,
    row: Value,
    session_wait_ttl_minutes: i64,
) {
    let id_str = row.get("id").and_then(|v| v.as_str());
    let chat_id_str = row.get("chat_id").and_then(|v| v.as_str());
    let message_type = row.get("message_type").and_then(|v| v.as_str());
    let payload = row.get("payload").cloned().unwrap_or(Value::Null);
    let created_at = row
        .get("created_at")
        .and_then(|v| v.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let (Some(id_str), Some(chat_id_str), Some(message_type)) = (id_str, chat_id_str, message_type)
    else {
        return;
    };

    let Ok(uuid) = Uuid::parse_str(id_str) else {
        return;
    };

    let Ok(jid) = chat_id_str.parse::<Jid>() else {
        let _ = mark_status(app_state, uuid, "failed").await;
        return;
    };

    let Some(client_ref) = app_state.clients.get(session) else {
        log::warn!(
            "Session {} not found for queued message {}",
            session,
            id_str
        );
        if should_fail_missing_session(created_at, session_wait_ttl_minutes) {
            let _ = mark_status(app_state, uuid, "failed").await;
        } else {
            let _ = mark_status(app_state, uuid, "queued").await;
        }
        return;
    };

    let client = client_ref.value().clone();
    let message_opt = build_message(&client, message_type, &payload).await;

    if let Some(msg) = message_opt {
        if let Err(e) = client.send_message(jid.clone(), msg).await {
            log::error!("Error sending message {}: {:?}", id_str, e);
            let _ = mark_status(app_state, uuid, "failed").await;
        } else {
            let _ = mark_status(app_state, uuid, "sent").await;
        }
    } else {
        log::warn!("Could not build message for type '{}'", message_type);
        let _ = mark_status(app_state, uuid, "failed").await;
    }
}

async fn build_message(
    client: &Client,
    message_type: &str,
    payload: &Value,
) -> Option<wa::Message> {
    match message_type {
        "text" => {
            let text = payload.get("text").and_then(|v| v.as_str()).unwrap_or("");
            if let Some(context_info) = build_reply_context_info(payload) {
                Some(wa::Message {
                    extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                        text: Some(text.to_string()),
                        context_info: Some(context_info),
                        ..Default::default()
                    })),
                    ..Default::default()
                })
            } else {
                Some(wa::Message {
                    conversation: Some(text.to_string()),
                    ..Default::default()
                })
            }
        }
        "image" => match build_image_message(client, payload).await {
            Ok(msg) => Some(msg),
            Err(err) => {
                log::warn!("Failed to build image message: {err}");
                None
            }
        },
        "video" => match build_video_message(client, payload).await {
            Ok(msg) => Some(msg),
            Err(err) => {
                log::warn!("Failed to build video message: {err}");
                None
            }
        },
        "voice" => match build_audio_message(client, payload, true).await {
            Ok(msg) => Some(msg),
            Err(err) => {
                log::warn!("Failed to build voice message: {err}");
                None
            }
        },
        "audio" => match build_audio_message(client, payload, false).await {
            Ok(msg) => Some(msg),
            Err(err) => {
                log::warn!("Failed to build audio message: {err}");
                None
            }
        },
        "file" => match build_document_message(client, payload).await {
            Ok(msg) => Some(msg),
            Err(err) => {
                log::warn!("Failed to build file message: {err}");
                None
            }
        },
        "sticker" => match build_sticker_message(client, payload).await {
            Ok(msg) => Some(msg),
            Err(err) => {
                log::warn!("Failed to build sticker message: {err}");
                None
            }
        },
        _ => {
            log::warn!("Message type {} not implemented in worker", message_type);
            None
        }
    }
}

fn build_reply_context_info(payload: &Value) -> Option<Box<wa::ContextInfo>> {
    let reply_message_id = payload
        .get("reply")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let quoted = payload.get("quoted").and_then(|v| v.as_object());
    let quoted_message_id = quoted
        .as_ref()
        .and_then(|q| q.get("messageId").or_else(|| q.get("message_id")))
        .and_then(|v| v.as_str());

    let stanza_id = match (reply_message_id, quoted_message_id) {
        (Some(id), _) => id,
        (None, Some(id)) => id,
        _ => return None,
    };
    let remote_jid = quoted
        .as_ref()
        .and_then(|q| q.get("chatId").or_else(|| q.get("chat_id")))
        .and_then(|v| v.as_str())
        .or_else(|| payload.get("chatId").and_then(|v| v.as_str()))
        .or_else(|| payload.get("chat_id").and_then(|v| v.as_str()));
    let participant = quoted
        .and_then(|q| q.get("participant").or_else(|| q.get("sender")))
        .and_then(|v| v.as_str());

    Some(Box::new(wa::ContextInfo {
        stanza_id: Some(stanza_id.to_string()),
        participant: participant.map(|s| s.to_string()),
        remote_jid: remote_jid.map(|s| s.to_string()),
        ..Default::default()
    }))
}

async fn build_image_message(client: &Client, payload: &Value) -> anyhow::Result<wa::Message> {
    let caption = payload
        .get("caption")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut mimetype = payload
        .get("mimetype")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let data = extract_media_bytes(client, payload, &mut mimetype).await?;

    let upload = client.upload(data, MediaType::Image).await?;
    let context_info = build_reply_context_info(payload);

    Ok(wa::Message {
        image_message: Some(Box::new(wa::message::ImageMessage {
            mimetype,
            caption,
            url: Some(upload.url),
            direct_path: Some(upload.direct_path),
            media_key: Some(upload.media_key),
            file_enc_sha256: Some(upload.file_enc_sha256),
            file_sha256: Some(upload.file_sha256),
            file_length: Some(upload.file_length),
            context_info,
            ..Default::default()
        })),
        ..Default::default()
    })
}

async fn build_video_message(client: &Client, payload: &Value) -> anyhow::Result<wa::Message> {
    let caption = payload
        .get("caption")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut mimetype = payload
        .get("mimetype")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let data = extract_media_bytes(client, payload, &mut mimetype).await?;
    let upload = client.upload(data, MediaType::Video).await?;
    let context_info = build_reply_context_info(payload);

    Ok(wa::Message {
        video_message: Some(Box::new(wa::message::VideoMessage {
            mimetype,
            caption,
            url: Some(upload.url),
            direct_path: Some(upload.direct_path),
            media_key: Some(upload.media_key),
            file_enc_sha256: Some(upload.file_enc_sha256),
            file_sha256: Some(upload.file_sha256),
            file_length: Some(upload.file_length),
            context_info,
            ..Default::default()
        })),
        ..Default::default()
    })
}

async fn build_audio_message(
    client: &Client,
    payload: &Value,
    ptt: bool,
) -> anyhow::Result<wa::Message> {
    let mut mimetype = payload
        .get("mimetype")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let data = extract_media_bytes(client, payload, &mut mimetype).await?;
    let upload = client.upload(data, MediaType::Audio).await?;
    let context_info = build_reply_context_info(payload);

    Ok(wa::Message {
        audio_message: Some(Box::new(wa::message::AudioMessage {
            mimetype,
            url: Some(upload.url),
            direct_path: Some(upload.direct_path),
            media_key: Some(upload.media_key),
            file_enc_sha256: Some(upload.file_enc_sha256),
            file_sha256: Some(upload.file_sha256),
            file_length: Some(upload.file_length),
            ptt: Some(ptt),
            context_info,
            ..Default::default()
        })),
        ..Default::default()
    })
}

async fn build_document_message(client: &Client, payload: &Value) -> anyhow::Result<wa::Message> {
    let caption = payload
        .get("caption")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let filename = payload
        .get("filename")
        .or_else(|| payload.get("fileName"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "file".to_string());

    let mut mimetype = payload
        .get("mimetype")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let data = extract_media_bytes(client, payload, &mut mimetype).await?;
    let upload = client.upload(data, MediaType::Document).await?;
    let context_info = build_reply_context_info(payload);

    Ok(wa::Message {
        document_message: Some(Box::new(wa::message::DocumentMessage {
            mimetype,
            caption,
            file_name: Some(filename),
            url: Some(upload.url),
            direct_path: Some(upload.direct_path),
            media_key: Some(upload.media_key),
            file_enc_sha256: Some(upload.file_enc_sha256),
            file_sha256: Some(upload.file_sha256),
            file_length: Some(upload.file_length),
            context_info,
            ..Default::default()
        })),
        ..Default::default()
    })
}

async fn build_sticker_message(client: &Client, payload: &Value) -> anyhow::Result<wa::Message> {
    let mut mimetype = payload
        .get("mimetype")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let is_animated = payload
        .get("isAnimated")
        .or_else(|| payload.get("is_animated"))
        .and_then(|v| v.as_bool());

    let data = extract_media_bytes(client, payload, &mut mimetype).await?;
    let upload = client.upload(data, MediaType::Sticker).await?;
    let context_info = build_reply_context_info(payload);
    let mimetype = mimetype.or_else(|| Some("image/webp".to_string()));

    Ok(wa::Message {
        sticker_message: Some(Box::new(wa::message::StickerMessage {
            mimetype,
            url: Some(upload.url),
            direct_path: Some(upload.direct_path),
            media_key: Some(upload.media_key),
            file_enc_sha256: Some(upload.file_enc_sha256),
            file_sha256: Some(upload.file_sha256),
            file_length: Some(upload.file_length),
            is_animated,
            context_info,
            ..Default::default()
        })),
        ..Default::default()
    })
}

async fn extract_media_bytes(
    client: &Client,
    payload: &Value,
    mimetype: &mut Option<String>,
) -> anyhow::Result<Vec<u8>> {
    let base64_input = payload.get("base64").and_then(|v| v.as_str());
    let url_input = payload.get("url").and_then(|v| v.as_str());

    let data = if let Some(b64) = base64_input {
        let (from_data_url, raw_b64) = split_data_url(b64);
        if mimetype.is_none() {
            *mimetype = from_data_url;
        }
        base64::engine::general_purpose::STANDARD
            .decode(raw_b64)
            .map_err(|e| anyhow::anyhow!("invalid base64: {e}"))?
    } else if let Some(url) = url_input {
        let response = client.http_client.execute(HttpRequest::get(url)).await?;
        if response.status_code >= 300 {
            return Err(anyhow::anyhow!(
                "download failed with status {}",
                response.status_code
            ));
        }
        response.body
    } else {
        return Err(anyhow::anyhow!("missing url or base64"));
    };

    Ok(data)
}

fn split_data_url(input: &str) -> (Option<String>, &str) {
    let Some(rest) = input.strip_prefix("data:") else {
        return (None, input);
    };

    let Some((meta, data)) = rest.split_once(',') else {
        return (None, input);
    };

    let mime = meta
        .split(';')
        .next()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    (mime, data)
}
