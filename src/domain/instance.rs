use std::collections::{HashMap, HashSet};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{delete, get, post},
};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{
    domain::common::success,
    errors::AppError,
    events::EventData,
    http::guards,
    state::{AppState, RuntimeInstance},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateInstanceRequest {
    instance_name: String,
    #[serde(default)]
    integration: Option<String>,
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    number: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetPresenceRequest {
    presence: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/create", post(create_instance))
        .route("/restart/:instance_name", post(restart_instance))
        .route("/connect/:instance_name", get(connect_instance))
        .route("/connectionState/:instance_name", get(connection_state))
        .route("/fetchInstances", get(fetch_instances))
        .route("/setPresence/:instance_name", post(set_presence))
        .route("/logout/:instance_name", delete(logout_instance))
        .route("/delete/:instance_name", delete(delete_instance))
}

async fn create_instance(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateInstanceRequest>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    guards::authorize(&state, &headers, "/instance/create", None).await?;
    guards::ensure_instance_not_exists(&state, &body.instance_name).await?;

    let instance_name = body.instance_name;
    let integration = body
        .integration
        .unwrap_or_else(|| "WHATSAPP-BAILEYS".to_string());
    let token = body
        .token
        .clone()
        .filter(|token| !token.trim().is_empty())
        .unwrap_or_else(|| generated_token(&instance_name));

    state
        .repo
        .upsert_instance(&instance_name, Some(token.as_str()), Some(integration.as_str()))
        .await?;

    state.wa_instances.write().await.insert(
        instance_name.clone(),
        RuntimeInstance {
            id: instance_name.clone(),
            integration: integration.clone(),
            state: "close".to_string(),
            token: Some(token.clone()),
            number: body.number.clone(),
            owner_jid: None,
            profile_name: None,
            profile_pic_url: None,
        },
    );

    state
        .events
        .emit(EventData {
            instance_name: instance_name.clone(),
            origin: "instance".to_string(),
            event: "INSTANCE_CREATE".to_string(),
            data: json!({ "instance": instance_name.clone() }),
            server_url: state.config.server.url.clone(),
            date_time: chrono_time(),
            sender: "rust-api".to_string(),
            api_key: None,
        })
        .await?;

    Ok(success(
        201,
        json!({
            "instance": {
                "instanceName": instance_name.clone(),
                "instanceId": instance_name,
                "integration": integration,
                "status": "close",
            },
            "hash": token,
        }),
    ))
}

async fn restart_instance(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(instance_name): Path<String>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    guards::authorize(
        &state,
        &headers,
        "/instance/restart/{instance_name}",
        Some(&instance_name),
    )
    .await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;

    let response = state.sidecar.restart_instance(&instance_name).await?;
    let payload = parse_payload(response.payload_json)?;

    Ok(success(200, json!({ "status": 200, "response": payload, "message": response.message })))
}

async fn connect_instance(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(instance_name): Path<String>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    guards::authorize(
        &state,
        &headers,
        "/instance/connect/{instance_name}",
        Some(&instance_name),
    )
    .await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;

    let response = state.sidecar.connect_instance(&instance_name).await?;
    let payload = normalize_connect_payload(parse_payload(response.payload_json)?);
    if let Some(connection_state) = infer_connection_state(&payload) {
        set_runtime_connection_state(&state, &instance_name, connection_state).await;
    }

    Ok(success(200, connect_success_payload(payload, response.message)))
}

async fn connection_state(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(instance_name): Path<String>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    guards::authorize(
        &state,
        &headers,
        "/instance/connectionState/{instance_name}",
        Some(&instance_name),
    )
    .await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;

    let response = state.sidecar.connection_state(&instance_name).await?;
    let payload = parse_payload(response.payload_json)?;
    if let Some(connection_state) = infer_connection_state(&payload) {
        set_runtime_connection_state(&state, &instance_name, connection_state).await;
    }
    Ok(success(200, json!({ "status": 200, "response": payload, "message": response.message })))
}

async fn fetch_instances(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    let _ = guards::authorize(&state, &headers, "/instance/fetchInstances", None).await?;

    let filter_id = query.get("instanceId").map(String::as_str);
    let filter_name = query.get("instanceName").map(String::as_str);
    let filter_number = query.get("number").map(String::as_str);

    let persisted = state.repo.list_instances().await?;
    let runtime = state.wa_instances.read().await;

    let mut seen = HashSet::new();
    let mut response = Vec::new();

    for record in persisted {
        let runtime_instance = runtime.get(&record.name);
        let payload = manager_instance_payload(record.name.clone(), Some(&record), runtime_instance);
        if matches_filters(&payload, filter_id, filter_name, filter_number) {
            response.push(payload);
        }
        seen.insert(record.name);
    }

    for (name, instance) in runtime.iter() {
        if seen.contains(name) {
            continue;
        }
        let payload = manager_instance_payload(name.clone(), None, Some(instance));
        if matches_filters(&payload, filter_id, filter_name, filter_number) {
            response.push(payload);
        }
    }

    Ok(success(200, Value::Array(response)))
}

async fn set_presence(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(instance_name): Path<String>,
    Json(body): Json<SetPresenceRequest>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    guards::authorize(
        &state,
        &headers,
        "/instance/setPresence/{instance_name}",
        Some(&instance_name),
    )
    .await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;

    let payload = json!({ "presence": body.presence });
    let response = state
        .sidecar
        .send_message(&instance_name, "setPresence", payload.to_string())
        .await?;
    Ok(success(201, json!({ "status": 201, "message": response.message })))
}

async fn logout_instance(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(instance_name): Path<String>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    guards::authorize(
        &state,
        &headers,
        "/instance/logout/{instance_name}",
        Some(&instance_name),
    )
    .await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;

    let response = state.sidecar.logout_instance(&instance_name).await?;
    let instance_id = instance_name.clone();
    state
        .wa_instances
        .write()
        .await
        .entry(instance_name)
        .and_modify(|instance| instance.state = "close".to_string())
        .or_insert_with(|| RuntimeInstance {
            id: instance_id,
            integration: "WHATSAPP-BAILEYS".to_string(),
            state: "close".to_string(),
            token: None,
            number: None,
            owner_jid: None,
            profile_name: None,
            profile_pic_url: None,
        });

    Ok(success(200, json!({ "status": 200, "message": response.message })))
}

async fn delete_instance(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(instance_name): Path<String>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    guards::authorize(
        &state,
        &headers,
        "/instance/delete/{instance_name}",
        Some(&instance_name),
    )
    .await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;

    state.wa_instances.write().await.remove(&instance_name);
    let _ = state.repo.delete_instance(&instance_name).await;
    state
        .events
        .emit(EventData {
            instance_name,
            origin: "instance".to_string(),
            event: "INSTANCE_DELETE".to_string(),
            data: json!({}),
            server_url: state.config.server.url.clone(),
            date_time: chrono_time(),
            sender: "rust-api".to_string(),
            api_key: None,
        })
        .await?;

    Ok(success(200, json!({ "status": 200, "message": "Instance deleted" })))
}

fn parse_payload(payload_json: String) -> Result<Value, AppError> {
    if payload_json.is_empty() {
        return Ok(json!({}));
    }

    let value = serde_json::from_str::<Value>(&payload_json).unwrap_or_else(|_| json!({ "raw": payload_json }));
    Ok(value)
}

fn normalize_connect_payload(payload: Value) -> Value {
    match payload {
        Value::String(code) => json!({ "code": code }),
        Value::Object(mut object) => {
            if !object.contains_key("code") {
                if let Some(raw) = object.get("raw").and_then(Value::as_str) {
                    if !object.contains_key("pairingCode") && looks_like_pairing_code(raw) {
                        object.insert("pairingCode".to_string(), Value::String(raw.to_string()));
                    } else if looks_like_qr_code(raw) {
                        object.insert("code".to_string(), Value::String(raw.to_string()));
                    }
                }
            }
            Value::Object(object)
        }
        other => other,
    }
}

fn looks_like_qr_code(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty() && trimmed.len() >= 16 && !trimmed.chars().any(char::is_whitespace)
}

fn looks_like_pairing_code(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.len() == 8 && trimmed.chars().all(|character| character.is_ascii_alphanumeric())
}

fn connect_success_payload(payload: Value, message: String) -> Value {
    let mut body = Map::new();
    body.insert("status".to_string(), Value::from(200));
    body.insert("message".to_string(), Value::String(message));
    if let Value::Object(fields) = &payload {
        for (key, value) in fields {
            body.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }
    body.insert("response".to_string(), payload);
    Value::Object(body)
}

fn infer_connection_state(payload: &Value) -> Option<&'static str> {
    extract_raw_connection_state(payload)
        .or_else(|| payload.get("instance").and_then(extract_raw_connection_state))
        .or_else(|| payload.get("response").and_then(extract_raw_connection_state))
        .and_then(normalize_connection_state)
}

fn extract_raw_connection_state(payload: &Value) -> Option<&str> {
    payload
        .get("connectionStatus")
        .and_then(Value::as_str)
        .or_else(|| payload.get("connectionState").and_then(Value::as_str))
        .or_else(|| payload.get("state").and_then(Value::as_str))
        .or_else(|| payload.get("status").and_then(Value::as_str))
}

fn normalize_connection_state(state: &str) -> Option<&'static str> {
    match state.trim().to_ascii_lowercase().as_str() {
        "open" | "connected" | "online" => Some("open"),
        "close" | "closed" | "disconnected" | "disconnect" | "offline" | "logout" => Some("close"),
        "connecting" | "pending" | "qrcode" | "qr" | "pairing" => Some("connecting"),
        _ => None,
    }
}

async fn set_runtime_connection_state(state: &AppState, instance_name: &str, connection_state: &str) {
    state
        .wa_instances
        .write()
        .await
        .entry(instance_name.to_string())
        .and_modify(|instance| instance.state = connection_state.to_string())
        .or_insert_with(|| RuntimeInstance {
            id: instance_name.to_string(),
            integration: "WHATSAPP-BAILEYS".to_string(),
            state: connection_state.to_string(),
            token: None,
            number: None,
            owner_jid: None,
            profile_name: None,
            profile_pic_url: None,
        });
}

fn manager_instance_payload(
    name: String,
    record: Option<&crate::repo::InstanceRecord>,
    runtime: Option<&RuntimeInstance>,
) -> Value {
    let id = runtime
        .map(|instance| instance.id.clone())
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| name.clone());
    let integration = runtime
        .map(|instance| instance.integration.clone())
        .or_else(|| record.and_then(|entry| entry.integration.clone()))
        .unwrap_or_else(|| "WHATSAPP-BAILEYS".to_string());
    let token = runtime
        .and_then(|instance| instance.token.clone())
        .or_else(|| record.and_then(|entry| entry.token.clone()));
    let number = runtime.and_then(|instance| instance.number.clone());
    let connection_status = runtime
        .map(|instance| instance.state.clone())
        .unwrap_or_else(|| "close".to_string());
    let owner_jid = runtime.and_then(|instance| instance.owner_jid.clone());
    let profile_name = runtime.and_then(|instance| instance.profile_name.clone());
    let profile_pic_url = runtime.and_then(|instance| instance.profile_pic_url.clone());

    json!({
        "id": id,
        "name": name,
        "number": number,
        "token": token,
        "integration": integration,
        "connectionStatus": connection_status,
        "ownerJid": owner_jid,
        "profileName": profile_name,
        "profilePicUrl": profile_pic_url,
        "_count": {
            "Message": 0,
            "Contact": 0,
            "Chat": 0
        }
    })
}

fn matches_filters(
    payload: &Value,
    instance_id: Option<&str>,
    instance_name: Option<&str>,
    number: Option<&str>,
) -> bool {
    let id_matches = match instance_id {
        Some(requested) => payload
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|value| value == requested),
        None => true,
    };
    let name_matches = match instance_name {
        Some(requested) => payload
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|value| value == requested),
        None => true,
    };
    let number_matches = match number {
        Some(requested) => payload
            .get("number")
            .and_then(Value::as_str)
            .is_some_and(|value| value == requested),
        None => true,
    };

    id_matches && name_matches && number_matches
}

fn chrono_time() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    now.to_string()
}

fn generated_token(instance_name: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{}-{:X}", instance_name.to_uppercase(), timestamp)
}
