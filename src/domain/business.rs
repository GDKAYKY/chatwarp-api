use std::collections::HashSet;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::post,
};
use serde_json::Value;

use crate::{errors::AppError, http::guards, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/:operation/:instance_name", post(handler))
}

async fn handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((operation, instance_name)): Path<(String, String)>,
    _payload: Json<Value>,
) -> Result<(), AppError> {
    guards::authorize(&state, &headers, "/business/{operation}/{instance_name}", Some(&instance_name)).await?;
    guards::ensure_instance_exists(&state, &instance_name).await?;

    let allowed: HashSet<&str> = ["getCatalog", "getCollections"].into_iter().collect();
    if !allowed.contains(operation.as_str()) {
        return Err(AppError::not_found(format!("Unknown business operation: {operation}")));
    }

    Err(AppError::not_implemented(format!(
        "POST /business/{operation}/{instance_name} is not implemented in Rust yet"
    )))
}
