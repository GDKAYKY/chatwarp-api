use crate::api_store::ApiBind;
use crate::client::Client;
use crate::http::HttpRequest;
use crate::server::AppState;
use base64::Engine as _;
use serde_json::Value;
use std::sync::Arc;
use tokio::time::{Duration, sleep};
use uuid::Uuid;
use waproto::whatsapp as wa;
use warp_core::download::MediaType;
use warp_core_binary::jid::Jid;

pub async fn spawn_messages_worker(app_state: Arc<AppState>) {
    loop {
        let rows = match app_state
            .api_store
            .query_json(
                "SELECT row_to_json(t)::jsonb as value FROM ( \
                    SELECT id, session, chat_id, message_type, payload \
                    FROM api_messages \
                    WHERE status = 'queued' \
                    ORDER BY created_at \
                    LIMIT 25 \
                ) t",
                vec![],
            )
            .await
        {
            Ok(rows) => rows,
            Err(e) => {
                log::error!("Error fetching queued messages: {}", e);
                sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        if rows.is_empty() {
            sleep(Duration::from_secs(2)).await;
            continue;
        }

        for row in rows {
            let id_str = row.get("id").and_then(|v| v.as_str());
            let session = row.get("session").and_then(|v| v.as_str());
            let chat_id_str = row.get("chat_id").and_then(|v| v.as_str());
            let message_type = row.get("message_type").and_then(|v| v.as_str());
            let payload = row.get("payload").cloned().unwrap_or(Value::Null);

            let (Some(id_str), Some(session), Some(chat_id_str), Some(message_type)) =
                (id_str, session, chat_id_str, message_type)
            else {
                continue;
            };

            let Ok(uuid) = Uuid::parse_str(id_str) else {
                continue;
            };

            let Ok(jid) = chat_id_str.parse::<Jid>() else {
                let _ = mark_status(&app_state, uuid, "failed").await;
                continue;
            };

            if let Some(client_ref) = app_state.clients.get(session) {
                let client = client_ref.value().clone();
                let message_opt = build_message(&client, message_type, &payload).await;

                if let Some(msg) = message_opt {
                    if let Err(e) = client.send_message(jid.clone(), msg).await {
                        log::error!("Error sending message {}: {:?}", id_str, e);
                        let _ = mark_status(&app_state, uuid, "failed").await;
                    } else {
                        let _ = mark_status(&app_state, uuid, "sent").await;
                    }
                } else {
                    log::warn!("Could not build message for type '{}'", message_type);
                    let _ = mark_status(&app_state, uuid, "failed").await;
                }
            } else {
                log::warn!(
                    "Session {} not found for queued message {}",
                    session,
                    id_str
                );
                // Do not fail it yet; session might be starting
            }
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

async fn build_message(
    client: &Client,
    message_type: &str,
    payload: &Value,
) -> Option<wa::Message> {
    match message_type {
        "text" => {
            let text = payload.get("text").and_then(|v| v.as_str()).unwrap_or("");
            Some(wa::Message {
                conversation: Some(text.to_string()),
                ..Default::default()
            })
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
        "file" => match build_document_message(client, payload).await {
            Ok(msg) => Some(msg),
            Err(err) => {
                log::warn!("Failed to build file message: {err}");
                None
            }
        },
        _ => {
            log::warn!("Message type {} not implemented in worker", message_type);
            None
        }
    }
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
