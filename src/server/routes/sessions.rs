use crate::api_store::ApiBind;
use crate::server::{AppState, SessionRuntime};
use crate::server::webhooks;
use axum::{Json, extract::{Path, State}, http::StatusCode, response::IntoResponse};
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{error, info};

pub async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let session = body
        .get("session")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("default")
        .to_string();

    info!(session = %session, "Solicitação para criar/atualizar sessão recebida");

    let webhook = body.get("webhook").cloned().unwrap_or(Value::Null);
    let webhook_enabled = webhook
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let webhook_url = webhook
        .get("url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let webhook_by_events = webhook
        .get("webhookByEvents")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let webhook_base64 = webhook
        .get("webhookBase64")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let webhook_headers = webhook.get("headers").cloned();
    let webhook_events = webhook.get("events").cloned();
    let phone_number = body
        .get("phone_number")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let result = state
        .api_store
        .execute(
            "INSERT INTO api_sessions (session, status, webhook_url, webhook_events, webhook_by_events, webhook_base64, webhook_headers, webhook_enabled, phone_number, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, now(), now()) \
             ON CONFLICT (session) DO UPDATE SET \
                status = EXCLUDED.status, \
                webhook_url = EXCLUDED.webhook_url, \
                webhook_events = EXCLUDED.webhook_events, \
                webhook_by_events = EXCLUDED.webhook_by_events, \
                webhook_base64 = EXCLUDED.webhook_base64, \
                webhook_headers = EXCLUDED.webhook_headers, \
                webhook_enabled = EXCLUDED.webhook_enabled, \
                phone_number = EXCLUDED.phone_number, \
                updated_at = now()",
            vec![
                ApiBind::Text(session.clone()),
                ApiBind::Text("open".to_string()),
                ApiBind::NullableText(webhook_url),
                ApiBind::NullableJson(webhook_events),
                ApiBind::Bool(webhook_by_events),
                ApiBind::Bool(webhook_base64),
                ApiBind::Json(webhook_headers.unwrap_or_else(|| json!({}))),
                ApiBind::Bool(webhook_enabled),
                ApiBind::NullableText(phone_number),
            ],
        )
        .await;

    if let Err(err) = result {
        error!(session = %session, error = %err, "Falha ao salvar sessão no banco de dados");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        );
    }

    info!(session = %session, "Sessão salva com sucesso no banco de dados");

    state
        .sessions_runtime
        .entry(session.clone())
        .or_insert_with(SessionRuntime::new);

    webhooks::enqueue(
        &state,
        Some(&session),
        "CONNECTION_UPDATE",
        json!({"status": "open"}),
    )
    .await;

    let row = state
        .api_store
        .query_json(
            "SELECT row_to_json(api_sessions)::jsonb as value FROM api_sessions WHERE session = $1",
            vec![ApiBind::Text(session.clone())],
        )
        .await
        .ok()
        .and_then(|mut rows| rows.pop());

    (
        StatusCode::CREATED,
        Json(row.unwrap_or_else(|| json!({"session": session}))),
    )
}

pub async fn list_sessions(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let rows = state
        .api_store
        .query_json(
            "SELECT row_to_json(api_sessions)::jsonb as value FROM api_sessions ORDER BY created_at DESC",
            vec![],
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

pub async fn get_session(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
) -> impl IntoResponse {
    let row = state
        .api_store
        .query_json(
            "SELECT row_to_json(api_sessions)::jsonb as value FROM api_sessions WHERE session = $1",
            vec![ApiBind::Text(session.clone())],
        )
        .await;

    match row {
        Ok(mut rows) => {
            if let Some(mut value) = rows.pop() {
                let runtime = state.sessions_runtime.get(&session).map(|entry| {
                    json!({
                        "connection_state": entry.connection_state,
                        "qr_code": entry.qr_code,
                        "pair_code": entry.pair_code,
                        "last_seen": entry.last_seen,
                    })
                });
                if let Some(runtime) = runtime {
                    if let Some(obj) = value.as_object_mut() {
                        obj.insert("runtime".to_string(), runtime);
                    }
                }
                (StatusCode::OK, Json(value))
            } else {
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "session_not_found"})),
                )
            }
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        ),
    }
}

pub async fn start_session(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
) -> impl IntoResponse {
    info!(session = %session, "Solicitação para iniciar sessão recebida");
    let result = state
        .api_store
        .execute(
            "UPDATE api_sessions SET status = $2, updated_at = now() WHERE session = $1",
            vec![ApiBind::Text(session.clone()), ApiBind::Text("started".to_string())],
        )
        .await;

    if let Err(err) = result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        );
    }

    if let Some(mut entry) = state.sessions_runtime.get_mut(&session) {
        entry.connection_state = "started".to_string();
    }

    webhooks::enqueue(
        &state,
        Some(&session),
        "CONNECTION_UPDATE",
        json!({"status": "started"}),
    )
    .await;

    (
        StatusCode::OK,
        Json(json!({"session": session, "status": "started"})),
    )
}

pub async fn stop_session(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
) -> impl IntoResponse {
    info!(session = %session, "Solicitação para parar sessão recebida");
    let result = state
        .api_store
        .execute(
            "UPDATE api_sessions SET status = $2, updated_at = now() WHERE session = $1",
            vec![ApiBind::Text(session.clone()), ApiBind::Text("stopped".to_string())],
        )
        .await;

    if let Err(err) = result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        );
    }

    if let Some(mut entry) = state.sessions_runtime.get_mut(&session) {
        entry.connection_state = "stopped".to_string();
    }

    webhooks::enqueue(
        &state,
        Some(&session),
        "CONNECTION_UPDATE",
        json!({"status": "close"}),
    )
    .await;

    (
        StatusCode::OK,
        Json(json!({"session": session, "status": "stopped"})),
    )
}

pub async fn delete_session(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
) -> impl IntoResponse {
    info!(session = %session, "Solicitação para deletar sessão recebida");
    let result = state
        .api_store
        .execute(
            "DELETE FROM api_sessions WHERE session = $1",
            vec![ApiBind::Text(session.clone())],
        )
        .await;

    if let Err(err) = result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        );
    }

    state.sessions_runtime.remove(&session);

    webhooks::enqueue(
        &state,
        Some(&session),
        "CONNECTION_UPDATE",
        json!({"status": "close"}),
    )
    .await;

    (
        StatusCode::OK,
        Json(json!({"session": session, "status": "deleted"})),
    )
}
