use crate::openapi::{openapi_document, swagger_ui};
use crate::server::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
};
use serde_json::{Value, json};
use std::sync::Arc;

pub async fn openapi_handler() -> Json<Value> {
    Json(openapi_document())
}

pub async fn swagger_handler() -> Html<&'static str> {
    swagger_ui()
}

pub async fn metrics_handler() -> Json<Value> {
    Json(json!({
        "uptime_seconds": 0,
        "instances_total": 0,
        "requests_total": 0,
        "inflight_requests": 0,
        "responses_2xx": 0,
        "responses_4xx": 0,
        "responses_5xx": 0,
        "responses_other": 0
    }))
}

pub async fn create_instance(
    State(_state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let name = payload["name"].as_str().unwrap_or("");
    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_name"})),
        );
    }

    // Logic to create instance would go here
    (
        StatusCode::CREATED,
        Json(json!({"instance": name, "status": "created"})),
    )
}

pub async fn delete_instance(
    Path(name): Path<String>,
    State(_state): State<Arc<AppState>>,
) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({"instance": name, "status": "deleted"})),
    )
}

pub async fn connection_state(
    Path(name): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if let Some(instance) = state.instances.get(&name) {
        let state_str = instance.connection_state.read().await;
        (
            StatusCode::OK,
            Json(json!({"instance": name, "state": *state_str})),
        )
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "instance_not_found"})),
        )
    }
}

pub async fn connect_instance(
    Path(_name): Path<String>,
    State(_state): State<Arc<AppState>>,
) -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"status": "connecting"})))
}

pub async fn instance_state(
    Path(name): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if let Some(instance) = state.instances.get(&name) {
        let qr = instance.qr_code.read().await;
        let connected = *instance.connection_state.read().await == "connected";
        (
            StatusCode::OK,
            Json(json!({
                "state": *instance.connection_state.read().await,
                "qr": *qr,
                "connected": connected,
                "last_error": null
            })),
        )
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "instance_not_found"})),
        )
    }
}

pub async fn send_message(
    Path((operation, instance_name)): Path<(String, String)>,
    Json(_payload): Json<Value>,
) -> impl IntoResponse {
    if operation != "sendText" {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(json!({"error": "not_implemented"})),
        );
    }

    (
        StatusCode::OK,
        Json(json!({"key": {"id": format!("msg-{}", instance_name)}})),
    )
}

pub async fn find_messages(
    Path(instance_name): Path<String>,
    Json(_payload): Json<Value>,
) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "instance": instance_name,
            "count": 0,
            "messages": []
        })),
    )
}

pub async fn find_chats(Path(instance_name): Path<String>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "instance": instance_name,
            "chats": []
        })),
    )
}

pub async fn create_group(
    Path(instance_name): Path<String>,
    Json(_payload): Json<Value>,
) -> impl IntoResponse {
    (
        StatusCode::CREATED,
        Json(json!({
            "instance": instance_name,
            "status": "created"
        })),
    )
}

pub async fn fetch_groups(Path(_instance_name): Path<String>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "instance": _instance_name,
            "groups": []
        })),
    )
}
