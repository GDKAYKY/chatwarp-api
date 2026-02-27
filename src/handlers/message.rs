use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Serialize;

use crate::{
    app::AppState,
    instance::{ConnectionState, InstanceCommand, InstanceError},
    wa::{
        MessageError,
        binary_node,
        message::{
            MessageOperation,
            OutgoingMessage,
            build_message_node,
            generate_message_id,
            validate_operation,
        },
    },
};

#[derive(Debug, Serialize)]
struct MessageKeyResponse {
    id: String,
}

#[derive(Debug, Serialize)]
struct MessagePostResponse {
    key: MessageKeyResponse,
}

#[derive(Debug, Serialize)]
struct MessageErrorResponse {
    error: &'static str,
    message: String,
}

/// Handles outbound message calls for supported operations.
pub async fn post_message_handler(
    State(state): State<AppState>,
    Path((operation, instance_name)): Path<(String, String)>,
    Json(payload): Json<OutgoingMessage>,
) -> axum::response::Response {
    let operation = match MessageOperation::parse(&operation) {
        Ok(parsed) => parsed,
        Err(error) => {
            return map_message_error(error);
        }
    };

    if let Err(error) = validate_operation(operation, &payload) {
        return map_message_error(error);
    }

    let manager = state.instance_manager();
    let Some(handle) = manager.get(&instance_name).await else {
        return map_instance_error(InstanceError::NotFound).into_axum_response();
    };

    if handle.connection_state().await != ConnectionState::Connected {
        return map_instance_error(InstanceError::NotConnected).into_axum_response();
    }

    let message_id = generate_message_id();
    let node = match build_message_node(&message_id, operation, &payload, None) {
        Ok(node) => node,
        Err(error) => return map_message_error(error),
    };

    let encoded = match binary_node::encode(&node) {
        Ok(encoded) => encoded,
        Err(error) => return map_message_error(MessageError::BinaryNode(error)),
    };

    if let Err(error) = handle
        .tx
        .send(InstanceCommand::SendMessage {
            message_id: message_id.clone(),
            payload: encoded,
        })
        .await
    {
        return map_instance_error(InstanceError::CommandChannelClosed)
            .with_detail(error.to_string())
            .into_axum_response();
    }

    (
        StatusCode::OK,
        Json(MessagePostResponse {
            key: MessageKeyResponse { id: message_id },
        }),
    )
        .into_response()
}

fn map_message_error(error: MessageError) -> axum::response::Response {
    let response = match error {
        MessageError::InvalidOperation(raw) => (
            StatusCode::BAD_REQUEST,
            Json(MessageErrorResponse {
                error: "invalid_operation",
                message: format!("unsupported operation: {raw}"),
            }),
        ),
        MessageError::InvalidContentForOperation { operation } => (
            StatusCode::BAD_REQUEST,
            Json(MessageErrorResponse {
                error: "invalid_content",
                message: format!("invalid content for operation: {operation}"),
            }),
        ),
        MessageError::Serialization(inner) => (
            StatusCode::BAD_REQUEST,
            Json(MessageErrorResponse {
                error: "serialization_error",
                message: inner.to_string(),
            }),
        ),
        MessageError::BinaryNode(inner) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(MessageErrorResponse {
                error: "binary_node_error",
                message: inner.to_string(),
            }),
        ),
    };

    response.into_response()
}

struct InstanceErrorHttp {
    status: StatusCode,
    body: MessageErrorResponse,
}

impl InstanceErrorHttp {
    fn with_detail(mut self, detail: String) -> Self {
        self.body.message = format!("{}: {detail}", self.body.message);
        self
    }

    fn into_axum_response(self) -> axum::response::Response {
        (self.status, Json(self.body)).into_response()
    }
}

fn map_instance_error(error: InstanceError) -> InstanceErrorHttp {
    match error {
        InstanceError::InvalidName => InstanceErrorHttp {
            status: StatusCode::BAD_REQUEST,
            body: MessageErrorResponse {
                error: "invalid_instance_name",
                message: "instance name cannot be empty".to_owned(),
            },
        },
        InstanceError::AlreadyExists => InstanceErrorHttp {
            status: StatusCode::CONFLICT,
            body: MessageErrorResponse {
                error: "instance_already_exists",
                message: "instance already exists".to_owned(),
            },
        },
        InstanceError::NotFound => InstanceErrorHttp {
            status: StatusCode::NOT_FOUND,
            body: MessageErrorResponse {
                error: "instance_not_found",
                message: "instance not found".to_owned(),
            },
        },
        InstanceError::NotConnected => InstanceErrorHttp {
            status: StatusCode::CONFLICT,
            body: MessageErrorResponse {
                error: "instance_not_connected",
                message: "instance is not connected".to_owned(),
            },
        },
        InstanceError::CommandChannelClosed => InstanceErrorHttp {
            status: StatusCode::SERVICE_UNAVAILABLE,
            body: MessageErrorResponse {
                error: "instance_unavailable",
                message: "instance command channel is unavailable".to_owned(),
            },
        },
    }
}
