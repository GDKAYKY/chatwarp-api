use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use crate::{app::AppState, group_store::GroupInfo, instance::InstanceError};

#[derive(Debug, Deserialize)]
pub(crate) struct CreateGroupRequest {
    subject: String,
    participants: Vec<String>,
}

#[derive(Debug, Serialize)]
struct GroupCreateResponse {
    instance: String,
    group: GroupInfo,
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct GroupListResponse {
    instance: String,
    groups: Vec<GroupInfo>,
}

#[derive(Debug, Serialize)]
struct GroupErrorResponse {
    error: &'static str,
    message: String,
}

/// Creates a synthetic group for an instance and persists it in memory.
pub(crate) async fn create_group_handler(
    State(state): State<AppState>,
    Path(instance_name): Path<String>,
    Json(request): Json<CreateGroupRequest>,
) -> axum::response::Response {
    let manager = state.instance_manager();
    if manager.get(&instance_name).await.is_none() {
        return map_instance_error(InstanceError::NotFound);
    }

    let subject = request.subject.trim();
    if subject.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(GroupErrorResponse {
                error: "invalid_subject",
                message: "subject cannot be empty".to_owned(),
            }),
        )
            .into_response();
    }

    let participants = request
        .participants
        .into_iter()
        .map(|jid| jid.trim().to_owned())
        .filter(|jid| !jid.is_empty())
        .collect::<Vec<_>>();

    if participants.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(GroupErrorResponse {
                error: "invalid_participants",
                message: "participants must include at least one jid".to_owned(),
            }),
        )
            .into_response();
    }

    let created = state
        .group_store()
        .create(&instance_name, subject.to_owned(), participants)
        .await;

    (
        StatusCode::CREATED,
        Json(GroupCreateResponse {
            instance: instance_name,
            group: created,
            status: "created",
        }),
    )
        .into_response()
}

/// Returns all synthetic groups previously created for an instance.
pub(crate) async fn fetch_all_groups_handler(
    State(state): State<AppState>,
    Path(instance_name): Path<String>,
) -> axum::response::Response {
    let manager = state.instance_manager();
    if manager.get(&instance_name).await.is_none() {
        return map_instance_error(InstanceError::NotFound);
    }

    let groups = state.group_store().list(&instance_name).await;
    (
        StatusCode::OK,
        Json(GroupListResponse {
            instance: instance_name,
            groups,
        }),
    )
        .into_response()
}

fn map_instance_error(error: InstanceError) -> axum::response::Response {
    match error {
        InstanceError::InvalidName => (
            StatusCode::BAD_REQUEST,
            Json(GroupErrorResponse {
                error: "invalid_instance_name",
                message: "instance name cannot be empty".to_owned(),
            }),
        )
            .into_response(),
        InstanceError::AlreadyExists => (
            StatusCode::CONFLICT,
            Json(GroupErrorResponse {
                error: "instance_already_exists",
                message: "instance already exists".to_owned(),
            }),
        )
            .into_response(),
        InstanceError::NotFound => (
            StatusCode::NOT_FOUND,
            Json(GroupErrorResponse {
                error: "instance_not_found",
                message: "instance not found".to_owned(),
            }),
        )
            .into_response(),
        InstanceError::NotConnected => (
            StatusCode::CONFLICT,
            Json(GroupErrorResponse {
                error: "instance_not_connected",
                message: "instance is not connected".to_owned(),
            }),
        )
            .into_response(),
        InstanceError::CommandChannelClosed => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(GroupErrorResponse {
                error: "instance_unavailable",
                message: "instance command channel is unavailable".to_owned(),
            }),
        )
            .into_response(),
    }
}
