use std::collections::HashSet;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{delete, get, post, put},
};
use serde_json::Value;

use crate::{errors::AppError, http::guards, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/{provider}/{operation}/{instance_name}", post(handler_post).get(handler_get))
        .route(
            "/{provider}/{operation}/{resource_id}/{instance_name}",
            get(handler_get_with_id).put(handler_put_with_id).delete(handler_delete_with_id),
        )
}

fn supported_provider(provider: &str) -> bool {
    let providers: HashSet<&str> = [
        "evolutionBot",
        "chatwoot",
        "typebot",
        "openai",
        "dify",
        "flowise",
        "n8n",
        "evoai",
    ]
    .into_iter()
    .collect();
    providers.contains(provider)
}

async fn handler_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((provider, operation, instance_name)): Path<(String, String, String)>,
    _payload: Json<Value>,
) -> Result<Json<Value>, AppError> {
    guards::authorize(
        &state,
        &headers,
        "/chatbot/{provider}/{operation}/{instance_name}",
        Some(&instance_name),
    )
    .await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;

    if !supported_provider(&provider) {
        return Err(AppError::not_found(format!("Unknown chatbot provider: {provider}")));
    }

    Err(AppError::not_implemented(format!(
        "POST /chatbot/{provider}/{operation}/{instance_name} is not implemented in Rust yet"
    )))
}

async fn handler_get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((provider, operation, instance_name)): Path<(String, String, String)>,
) -> Result<Json<Value>, AppError> {
    guards::authorize(
        &state,
        &headers,
        "/chatbot/{provider}/{operation}/{instance_name}",
        Some(&instance_name),
    )
    .await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;

    if !supported_provider(&provider) {
        return Err(AppError::not_found(format!("Unknown chatbot provider: {provider}")));
    }

    Err(AppError::not_implemented(format!(
        "GET /chatbot/{provider}/{operation}/{instance_name} is not implemented in Rust yet"
    )))
}

async fn handler_get_with_id(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((provider, operation, resource_id, instance_name)): Path<(String, String, String, String)>,
) -> Result<Json<Value>, AppError> {
    guards::authorize(
        &state,
        &headers,
        "/chatbot/{provider}/{operation}/{resource_id}/{instance_name}",
        Some(&instance_name),
    )
    .await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;

    if !supported_provider(&provider) {
        return Err(AppError::not_found(format!("Unknown chatbot provider: {provider}")));
    }

    Err(AppError::not_implemented(format!(
        "GET /chatbot/{provider}/{operation}/{resource_id}/{instance_name} is not implemented in Rust yet"
    )))
}

async fn handler_put_with_id(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((provider, operation, resource_id, instance_name)): Path<(String, String, String, String)>,
    _payload: Json<Value>,
) -> Result<Json<Value>, AppError> {
    guards::authorize(
        &state,
        &headers,
        "/chatbot/{provider}/{operation}/{resource_id}/{instance_name}",
        Some(&instance_name),
    )
    .await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;

    if !supported_provider(&provider) {
        return Err(AppError::not_found(format!("Unknown chatbot provider: {provider}")));
    }

    Err(AppError::not_implemented(format!(
        "PUT /chatbot/{provider}/{operation}/{resource_id}/{instance_name} is not implemented in Rust yet"
    )))
}

async fn handler_delete_with_id(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((provider, operation, resource_id, instance_name)): Path<(String, String, String, String)>,
) -> Result<Json<Value>, AppError> {
    guards::authorize(
        &state,
        &headers,
        "/chatbot/{provider}/{operation}/{resource_id}/{instance_name}",
        Some(&instance_name),
    )
    .await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;

    if !supported_provider(&provider) {
        return Err(AppError::not_found(format!("Unknown chatbot provider: {provider}")));
    }

    Err(AppError::not_implemented(format!(
        "DELETE /chatbot/{provider}/{operation}/{resource_id}/{instance_name} is not implemented in Rust yet"
    )))
}
