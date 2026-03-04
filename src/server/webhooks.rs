use crate::api_store::ApiBind;
use crate::models::webhook_model::WebhookConfig;
use crate::server::AppState;
use chatwarp_api_ureq_http_client::UreqHttpClient;
use chrono::Utc;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;
use warp_core::net::{HttpClient, HttpRequest};
use std::collections::HashMap;

pub async fn enqueue(state: &AppState, session: Option<&str>, event: &str, data: Value) {
    let payload = json!({
        "event": event,
        "instance": session.unwrap_or(""),
        "data": data,
        "sender": null,
        "apikey": null
    });

    let _ = state
        .api_store
        .execute(
            "INSERT INTO webhook_outbox (session, event, payload) VALUES ($1, $2, $3)",
            vec![
                ApiBind::NullableText(session.map(|s| s.to_string())),
                ApiBind::Text(event.to_string()),
                ApiBind::Json(payload),
            ],
        )
        .await;
}

pub fn spawn_worker(state: Arc<AppState>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let client = UreqHttpClient::new();
        loop {
            if let Err(err) = process_outbox(&state, &client).await {
                log::warn!("webhook worker error: {err}");
            }
            sleep(Duration::from_secs(5)).await;
        }
    })
}

async fn process_outbox(state: &AppState, client: &UreqHttpClient) -> anyhow::Result<()> {
    let rows = state
        .api_store
        .query_json(
            "SELECT row_to_json(t)::jsonb as value FROM ( \
                SELECT id, session, event, payload, attempts \
                FROM webhook_outbox \
                WHERE status = 'pending' AND next_attempt_at <= now() \
                ORDER BY created_at \
                LIMIT 25 \
            ) t",
            vec![],
        )
        .await?;

    for row in rows {
        let id = row
            .get("id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());
        let event = row.get("event").and_then(|v| v.as_str()).unwrap_or("");
        let session = row.get("session").and_then(|v| v.as_str());
        let payload = row.get("payload").cloned().unwrap_or(Value::Null);
        let attempts = row.get("attempts").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

        let Some(id) = id else { continue };

        let mut targets = Vec::new();

        if let Some(sess) = session {
            if let Some(cfg) = load_instance_webhook(state, sess).await? {
                if cfg.enabled && event_allowed(&cfg.events, event) {
                    targets.push(cfg);
                }
            }
        }

        if let Some(cfg) = load_global_webhook(event) {
            targets.push(cfg);
        }

        if targets.is_empty() {
            let _ = mark_sent(state, id).await;
            continue;
        }

        let mut all_ok = true;
        let mut last_error: Option<String> = None;

        for target in targets {
            let url = if target.by_events {
                format!("{}/{}", target.url.trim_end_matches('/'), event_path(event))
            } else {
                target.url.clone()
            };

            let enriched = enrich_payload(&payload, &url, target.base64);
            let mut req = HttpRequest::post(url)
                .with_header("Content-Type", "application/json")
                .with_body(serde_json::to_vec(&enriched)?);

            for (k, v) in target.headers.iter() {
                req = req.with_header(k, v);
            }

            match client.execute(req).await {
                Ok(resp) if (200..300).contains(&resp.status_code) => {}
                Ok(resp) => {
                    all_ok = false;
                    last_error = Some(format!("http {}", resp.status_code));
                }
                Err(err) => {
                    all_ok = false;
                    last_error = Some(err.to_string());
                }
            }
        }

        if all_ok {
            let _ = mark_sent(state, id).await;
        } else {
            let _ = mark_retry(state, id, attempts + 1, last_error.unwrap_or_default()).await;
        }
    }

    Ok(())
}

fn enrich_payload(payload: &Value, destination: &str, base64_enabled: bool) -> Value {
    let mut obj = payload.as_object().cloned().unwrap_or_default();
    if !base64_enabled {
        if let Some(Value::Object(data)) = obj.get_mut("data") {
            if let Some(Value::Object(message)) = data.get_mut("message") {
                message.remove("base64");
            }
            if let Some(Value::Array(messages)) = data.get_mut("messages") {
                for entry in messages.iter_mut() {
                    if let Value::Object(entry_obj) = entry {
                        if let Some(Value::Object(message)) = entry_obj.get_mut("message") {
                            message.remove("base64");
                        }
                    }
                }
            }
        }
    }
    obj.insert("destination".to_string(), json!(destination));
    obj.insert("date_time".to_string(), json!(Utc::now().to_rfc3339()));
    obj.insert(
        "server_url".to_string(),
        json!(std::env::var("SERVER_URL").unwrap_or_else(|_| "http://localhost:8080".to_string())),
    );
    Value::Object(obj)
}

async fn mark_sent(state: &AppState, id: Uuid) -> anyhow::Result<()> {
    state
        .api_store
        .execute(
            "UPDATE webhook_outbox SET status = 'sent', last_error = NULL WHERE id = $1",
            vec![ApiBind::Uuid(id)],
        )
        .await?;
    Ok(())
}

async fn mark_retry(
    state: &AppState,
    id: Uuid,
    attempts: i32,
    error: String,
) -> anyhow::Result<()> {
    let (status, delay_seconds) = if attempts >= 5 {
        ("failed", 600)
    } else {
        ("pending", backoff_seconds(attempts))
    };

    state
        .api_store
        .execute(
            "UPDATE webhook_outbox \
             SET status = $2, attempts = $3, last_error = $4, \
                 next_attempt_at = now() + ($5 || ' seconds')::interval \
             WHERE id = $1",
            vec![
                ApiBind::Uuid(id),
                ApiBind::Text(status.to_string()),
                ApiBind::Int(attempts),
                ApiBind::Text(error),
                ApiBind::Int(delay_seconds),
            ],
        )
        .await?;
    Ok(())
}

fn backoff_seconds(attempts: i32) -> i32 {
    match attempts {
        1 => 5,
        2 => 30,
        3 => 120,
        _ => 600,
    }
}

fn event_path(event: &str) -> String {
    event.to_lowercase().replace('_', "-")
}

fn event_allowed(events: &Option<Vec<String>>, event: &str) -> bool {
    match events {
        None => true,
        Some(list) if list.is_empty() => true,
        Some(list) => list.iter().any(|e| e == event),
    }
}

pub async fn load_instance_webhook(
    state: &AppState,
    session: &str,
) -> anyhow::Result<Option<WebhookConfig>> {
    let rows = state
        .api_store
        .query_json(
            "SELECT row_to_json(t)::jsonb as value FROM ( \
                SELECT webhook_enabled, webhook_url, webhook_by_events, webhook_base64, \
                       webhook_headers, webhook_events \
                FROM api_sessions WHERE session = $1 \
            ) t",
            vec![ApiBind::Text(session.to_string())],
        )
        .await?;

    let Some(row) = rows.into_iter().next() else {
        return Ok(None);
    };

    let enabled = row
        .get("webhook_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let url = row
        .get("webhook_url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let by_events = row
        .get("webhook_by_events")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let base64 = row
        .get("webhook_base64")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let headers = row
        .get("webhook_headers")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|val| (k.clone(), val.to_string())))
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();

    let events = row
        .get("webhook_events")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        });

    if url.is_empty() {
        return Ok(None);
    }

    Ok(Some(WebhookConfig {
        enabled,
        url,
        by_events,
        base64,
        headers,
        events,
    }))
}

fn load_global_webhook(event: &str) -> Option<WebhookConfig> {
    let enabled = std::env::var("WEBHOOK_GLOBAL_ENABLED")
        .ok()
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);
    if !enabled {
        return None;
    }

    if !event_env_enabled(event) {
        return None;
    }

    let url = std::env::var("WEBHOOK_GLOBAL_URL").ok()?;
    let by_events = std::env::var("WEBHOOK_GLOBAL_WEBHOOK_BY_EVENTS")
        .ok()
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    Some(WebhookConfig {
        enabled: true,
        url,
        by_events,
        base64: false,
        headers: HashMap::new(),
        events: None,
    })
}

fn event_env_enabled(event: &str) -> bool {
    let key = format!("WEBHOOK_EVENTS_{}", event);
    std::env::var(key)
        .ok()
        .map(|v| v == "true" || v == "1")
        .unwrap_or(true)
}
