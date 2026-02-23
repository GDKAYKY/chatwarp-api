use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::{Value, json};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{error}: {message}")]
    Http {
        status: StatusCode,
        error: &'static str,
        message: Value,
    },
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("grpc transport error: {0}")]
    GrpcTransport(#[from] tonic::transport::Error),
    #[error("grpc status error: {0}")]
    GrpcStatus(#[from] tonic::Status),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("configuration error: {0}")]
    Config(String),
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    status: u16,
    error: &'static str,
    response: ErrorResponse,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    message: Value,
}

impl AppError {
    pub fn new(status: StatusCode, error: &'static str, message: Value) -> Self {
        Self::Http {
            status,
            error,
            message,
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "Bad Request", json!([message.into()]))
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "Unauthorized", json!(message.into()))
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "Forbidden", json!([message.into()]))
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "Not Found", json!([message.into()]))
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error",
            json!([message.into()]),
        )
    }

    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "Service Unavailable",
            json!([message.into()]),
        )
    }

    pub fn not_implemented(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::NOT_IMPLEMENTED,
            "Not Implemented",
            json!([message.into()]),
        )
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            Self::Http {
                status,
                error,
                message,
            } => (
                status,
                Json(ErrorEnvelope {
                    status: status.as_u16(),
                    error,
                    response: ErrorResponse { message },
                }),
            )
                .into_response(),
            Self::Database(error) => AppError::internal(error.to_string()).into_response(),
            Self::GrpcTransport(error) => AppError::service_unavailable(error.to_string()).into_response(),
            Self::GrpcStatus(error) => AppError::service_unavailable(error.to_string()).into_response(),
            Self::Io(error) => AppError::internal(error.to_string()).into_response(),
            Self::Serde(error) => AppError::bad_request(error.to_string()).into_response(),
            Self::Config(error) => AppError::internal(error).into_response(),
        }
    }
}
