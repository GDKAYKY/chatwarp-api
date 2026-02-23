use axum::http::HeaderMap;
use tracing::warn;

use crate::{errors::AppError, state::AppState};

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn apikey(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = header_value(headers, "apikey") {
        return Some(value);
    }

    if let Some(value) = header_value(headers, "x-api-key") {
        return Some(value);
    }

    if let Some(auth) = header_value(headers, "authorization")
        && let Some(token) = auth.strip_prefix("Bearer ")
    {
        let token = token.trim();
        if !token.is_empty() {
            return Some(token.to_string());
        }
    }

    None
}

pub async fn authorize(
    state: &AppState,
    headers: &HeaderMap,
    original_path: &str,
    instance_name: Option<&str>,
) -> Result<String, AppError> {
    let Some(key) = apikey(headers) else {
        warn!("auth rejected: missing api key for path={original_path}");
        return Err(AppError::unauthorized("Unauthorized"));
    };

    if key == state.config.authentication.api_key {
        return Ok(key);
    }

    if let Some(instance_name) = instance_name {
        if let Some(instance) = state.repo.find_instance_by_name(instance_name).await?
            && instance.token.as_deref() == Some(key.as_str())
        {
            return Ok(key);
        }
    } else if original_path.contains("/instance/fetchInstances") && state.config.database.save_data.instance {
        if state.repo.find_instance_by_token(&key).await?.is_some() {
            return Ok(key);
        }
    }

    warn!("auth rejected: invalid api key for path={original_path}");
    Err(AppError::unauthorized("Unauthorized"))
}

pub async fn ensure_instance_exists(state: &AppState, instance_name: &str) -> Result<(), AppError> {
    let in_memory = state.wa_instances.read().await.contains_key(instance_name);
    if in_memory {
        return Ok(());
    }

    if state.repo.find_instance_by_name(instance_name).await?.is_some() {
        return Ok(());
    }

    Err(AppError::not_found(format!(
        "The \"{}\" instance does not exist",
        instance_name
    )))
}

pub async fn ensure_instance_not_exists(state: &AppState, instance_name: &str) -> Result<(), AppError> {
    if state.wa_instances.read().await.contains_key(instance_name)
        || state
            .repo
            .find_instance_by_name(instance_name)
            .await?
            .is_some()
    {
        return Err(AppError::forbidden(format!(
            "This name \"{}\" is already in use.",
            instance_name
        )));
    }

    Ok(())
}
