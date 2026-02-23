use std::collections::HashSet;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{delete, get, post},
};
use serde_json::Value;

use crate::{errors::AppError, http::guards, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/:operation/:instance_name",
        post(post_handler).get(get_handler).delete(delete_handler),
    )
}

async fn post_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((operation, instance_name)): Path<(String, String)>,
    _payload: Json<Value>,
) -> Result<Json<Value>, AppError> {
    guards::authorize(&state, &headers, "/group/{operation}/{instance_name}", Some(&instance_name)).await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;

    let allowed: HashSet<&str> = [
        "create",
        "updateGroupSubject",
        "updateGroupPicture",
        "updateGroupDescription",
        "sendInvite",
        "revokeInviteCode",
        "updateParticipant",
        "updateSetting",
        "toggleEphemeral",
    ]
    .into_iter()
    .collect();

    if !allowed.contains(operation.as_str()) {
        return Err(AppError::not_found(format!("Unknown group operation: {operation}")));
    }

    Err(AppError::not_implemented(format!(
        "POST /group/{operation}/{instance_name} is not implemented in Rust yet"
    )))
}

async fn get_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((operation, instance_name)): Path<(String, String)>,
) -> Result<Json<Value>, AppError> {
    guards::authorize(&state, &headers, "/group/{operation}/{instance_name}", Some(&instance_name)).await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;

    let allowed: HashSet<&str> = [
        "findGroupInfos",
        "fetchAllGroups",
        "participants",
        "inviteCode",
        "inviteInfo",
        "acceptInviteCode",
    ]
    .into_iter()
    .collect();

    if !allowed.contains(operation.as_str()) {
        return Err(AppError::not_found(format!("Unknown group operation: {operation}")));
    }

    Err(AppError::not_implemented(format!(
        "GET /group/{operation}/{instance_name} is not implemented in Rust yet"
    )))
}

async fn delete_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((operation, instance_name)): Path<(String, String)>,
) -> Result<Json<Value>, AppError> {
    guards::authorize(&state, &headers, "/group/{operation}/{instance_name}", Some(&instance_name)).await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;

    if operation != "leaveGroup" {
        return Err(AppError::not_found(format!("Unknown group operation: {operation}")));
    }

    Err(AppError::not_implemented(format!(
        "DELETE /group/{operation}/{instance_name} is not implemented in Rust yet"
    )))
}
