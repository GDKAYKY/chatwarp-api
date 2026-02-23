use axum::{
    extract::OriginalUri,
    http::Method,
    response::IntoResponse,
    Json,
};
use serde_json::{Value, json};

use crate::errors::AppError;

pub fn success(status: u16, payload: Value) -> impl IntoResponse {
    (
        axum::http::StatusCode::from_u16(status).unwrap_or(axum::http::StatusCode::OK),
        Json(payload),
    )
}

pub async fn placeholder(OriginalUri(uri): OriginalUri, method: Method) -> Result<Json<Value>, AppError> {
    Err(AppError::not_implemented(format!(
        "{} {} is not implemented in Rust yet",
        method, uri
    )))
}

pub fn welcome_payload(client_name: &str, manager: Option<String>, wa_version: &str) -> Value {
    json!({
        "status": 200,
        "message": "Welcome to the Evolution API, it is working!",
        "version": env!("CARGO_PKG_VERSION"),
        "clientName": client_name,
        "manager": manager,
        "documentation": "https://doc.evolution-api.com",
        "whatsappWebVersion": wa_version,
    })
}
