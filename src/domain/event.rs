use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use serde_json::{Value, json};

use crate::{
    domain::common::success,
    errors::AppError,
    events::EventSinkConfig,
    http::guards,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/webhook/set/{instance_name}", post(set_webhook))
        .route("/webhook/find/{instance_name}", get(find_webhook))
        .route("/websocket/set/{instance_name}", post(set_websocket))
        .route("/websocket/find/{instance_name}", get(find_websocket))
        .route("/rabbitmq/set/{instance_name}", post(set_rabbitmq))
        .route("/rabbitmq/find/{instance_name}", get(find_rabbitmq))
        .route("/{provider}/{operation}/{instance_name}", post(unsupported_set).get(unsupported_find))
}

async fn set_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(instance_name): Path<String>,
    Json(payload): Json<EventSinkConfig>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    guards::authorize(&state, &headers, "/webhook/set/{instance_name}", Some(&instance_name)).await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;
    state.events.set_webhook(instance_name, payload).await;
    Ok(success(200, json!({ "status": 200, "message": "Webhook configured" })))
}

async fn find_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(instance_name): Path<String>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    guards::authorize(&state, &headers, "/webhook/find/{instance_name}", Some(&instance_name)).await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;

    Ok(success(
        200,
        json!({ "status": 200, "webhook": state.events.webhook_config(&instance_name).await }),
    ))
}

async fn set_websocket(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(instance_name): Path<String>,
    Json(payload): Json<EventSinkConfig>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    guards::authorize(&state, &headers, "/websocket/set/{instance_name}", Some(&instance_name)).await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;
    state.events.set_websocket(instance_name, payload).await;
    Ok(success(200, json!({ "status": 200, "message": "WebSocket configured" })))
}

async fn find_websocket(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(instance_name): Path<String>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    guards::authorize(&state, &headers, "/websocket/find/{instance_name}", Some(&instance_name)).await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;

    Ok(success(
        200,
        json!({ "status": 200, "websocket": state.events.websocket_config(&instance_name).await }),
    ))
}

async fn set_rabbitmq(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(instance_name): Path<String>,
    Json(payload): Json<EventSinkConfig>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    guards::authorize(&state, &headers, "/rabbitmq/set/{instance_name}", Some(&instance_name)).await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;
    state.events.set_rabbitmq(instance_name, payload).await;
    Ok(success(200, json!({ "status": 200, "message": "RabbitMQ configured" })))
}

async fn find_rabbitmq(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(instance_name): Path<String>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    guards::authorize(&state, &headers, "/rabbitmq/find/{instance_name}", Some(&instance_name)).await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;

    Ok(success(
        200,
        json!({ "status": 200, "rabbitmq": state.events.rabbitmq_config(&instance_name).await }),
    ))
}

async fn unsupported_set(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((provider, operation, instance_name)): Path<(String, String, String)>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, AppError> {
    guards::authorize(&state, &headers, "/{provider}/{operation}/{instance_name}", Some(&instance_name)).await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;

    Err(AppError::not_implemented(format!(
        "POST /{provider}/{operation}/{instance_name} is not implemented in Rust yet"
    )))
}

async fn unsupported_find(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((provider, operation, instance_name)): Path<(String, String, String)>,
) -> Result<Json<Value>, AppError> {
    guards::authorize(&state, &headers, "/{provider}/{operation}/{instance_name}", Some(&instance_name)).await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;

    Err(AppError::not_implemented(format!(
        "GET /{provider}/{operation}/{instance_name} is not implemented in Rust yet"
    )))
}
