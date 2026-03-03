use crate::api_store::ApiBind;
use crate::server::AppState;
use serde_json::Value;
use std::sync::Arc;
use tokio::time::{Duration, sleep};
use uuid::Uuid;
use waproto::whatsapp as wa;
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

            let message_opt = build_message(message_type, &payload);

            if let Some(msg) = message_opt {
                if let Some(client_ref) = app_state.clients.get(session) {
                    let client = client_ref.value().clone();
                    if let Err(e) = client.send_message(jid.clone(), msg).await {
                        log::error!("Error sending message {}: {:?}", id_str, e);
                        let _ = mark_status(&app_state, uuid, "failed").await;
                    } else {
                        let _ = mark_status(&app_state, uuid, "sent").await;
                    }
                } else {
                    log::warn!(
                        "Session {} not found for queued message {}",
                        session,
                        id_str
                    );
                    // Do not fail it yet; session might be starting
                }
            } else {
                log::warn!("Could not build message for type '{}'", message_type);
                let _ = mark_status(&app_state, uuid, "failed").await;
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

fn build_message(message_type: &str, payload: &Value) -> Option<wa::Message> {
    match message_type {
        "text" => {
            let text = payload.get("text").and_then(|v| v.as_str()).unwrap_or("");
            Some(wa::Message {
                conversation: Some(text.to_string()),
                ..Default::default()
            })
        }
        "image" => {
            // Need implementing extended logic for media, for now we can just return None
            log::warn!("'image' sending not fully implemented via worker yet");
            None
        }
        _ => {
            log::warn!("Message type {} not implemented in worker", message_type);
            None
        }
    }
}
