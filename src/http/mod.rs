pub mod guards;

use std::{path::PathBuf, str::FromStr};

use axum::{
    Json, Router,
    body::Body,
    extract::{Path as AxumPath, State, WebSocketUpgrade},
    http::{HeaderMap, HeaderValue, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures::{SinkExt, StreamExt};
use serde_json::json;
use tokio::fs;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    limit::RequestBodyLimitLayer,
    trace::TraceLayer,
};
use tracing::info;

use crate::{
    domain,
    domain::common::welcome_payload,
    errors::AppError,
    manager_ui,
    metrics,
    state::AppState,
};

pub fn build_router(state: AppState) -> Router {
    let cors = build_cors_layer(&state);

    Router::new()
        .route("/", get(root_handler))
        .route("/verify-creds", post(verify_creds_handler))
        .route("/metrics", get(metrics_handler))
        .route("/ws", get(websocket_handler))
        .route("/assets/*file", get(assets_handler))
        .route("/manager", get(manager_index_handler))
        .route("/manager/*path", get(manager_handler))
        .nest("/instance", domain::instance::router())
        .nest("/message", domain::message::router())
        .nest("/call", domain::call::router())
        .nest("/chat", domain::chat::router())
        .nest("/business", domain::business::router())
        .nest("/group", domain::group::router())
        .nest("/template", domain::template::router())
        .nest("/settings", domain::settings::router())
        .nest("/proxy", domain::proxy::router())
        .nest("/label", domain::label::router())
        .merge(domain::channel::router())
        .merge(domain::event::router())
        .nest("/chatbot", domain::chatbot::router())
        .merge(domain::storage::router())
        .layer(RequestBodyLimitLayer::new(136 * 1024 * 1024))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .fallback(not_found_handler)
        .with_state(state)
}

fn build_cors_layer(state: &AppState) -> CorsLayer {
    let methods = state
        .config
        .cors
        .methods
        .iter()
        .filter_map(|method| Method::from_bytes(method.as_bytes()).ok())
        .collect::<Vec<_>>();

    let layer = CorsLayer::new().allow_methods(methods);
    let layer = if state.config.cors.credentials {
        layer.allow_credentials(true)
    } else {
        layer
    };

    if state.config.cors.origins.iter().any(|origin| origin == "*") {
        layer.allow_origin(Any)
    } else {
        let origins = state
            .config
            .cors
            .origins
            .iter()
            .filter_map(|origin| HeaderValue::from_str(origin).ok())
            .collect::<Vec<_>>();
        layer.allow_origin(origins)
    }
}

async fn root_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    telemetry_log(&state, &headers, "/").await;

    let manager = if !state.config.server.disable_manager {
        Some(format!("{}/manager", state.config.server.url))
    } else {
        None
    };

    let payload = welcome_payload(&state.config.database.client_name, manager, "2.3000.1023204200");
    Ok((StatusCode::OK, Json(payload)).into_response())
}

async fn verify_creds_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    guards::authorize(&state, &headers, "/verify-creds", None).await?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "status": 200,
            "message": "Credentials are valid",
            "facebookAppId": state.config.facebook.app_id,
            "facebookConfigId": state.config.facebook.config_id,
            "facebookUserToken": state.config.facebook.user_token,
        })),
    )
        .into_response())
}

async fn metrics_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    if !state.config.metrics.enabled {
        return Err(AppError::not_found("Not Found"));
    }

    if let Some(allowed) = &state.config.metrics.allowed_ips {
        let client = client_ip(&headers)
            .ok_or_else(|| AppError::forbidden("Forbidden: IP not allowed"))?;
        if !allowed.contains(&client) {
            return Err(AppError::forbidden("Forbidden: IP not allowed"));
        }
    }

    if state.config.metrics.auth_required {
        let user = state
            .config
            .metrics
            .user
            .as_ref()
            .ok_or_else(|| AppError::internal("Metrics authentication not configured"))?;
        let password = state
            .config
            .metrics
            .password
            .as_ref()
            .ok_or_else(|| AppError::internal("Metrics authentication not configured"))?;

        let auth_header = headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| AppError::unauthorized("Authentication required"))?;

        if !auth_header.starts_with("Basic ") {
            return Err(AppError::unauthorized("Authentication required"));
        }

        let decoded = STANDARD
            .decode(auth_header.trim_start_matches("Basic "))
            .map_err(|_| AppError::unauthorized("Invalid credentials"))?;
        let credentials = String::from_utf8(decoded).map_err(|_| AppError::unauthorized("Invalid credentials"))?;
        let mut parts = credentials.splitn(2, ':');
        let req_user = parts.next().unwrap_or_default();
        let req_pass = parts.next().unwrap_or_default();

        if req_user != user || req_pass != password {
            return Err(AppError::unauthorized("Invalid credentials"));
        }
    }

    let output = metrics::render_metrics(&state).await;
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/plain; version=0.0.4; charset=utf-8")
        .header("cache-control", "no-cache, no-store, must-revalidate")
        .body(Body::from(output))
        .map_err(|error| AppError::internal(error.to_string()))?)
}

async fn websocket_handler(
    State(state): State<AppState>,
    websocket: WebSocketUpgrade,
) -> impl IntoResponse {
    websocket.on_upgrade(move |socket| async move {
        let mut receiver = state.events.subscribe();
        let (mut sender, _) = socket.split();

        while let Ok(payload) = receiver.recv().await {
            if sender
                .send(axum::extract::ws::Message::Text(payload.into()))
                .await
                .is_err()
            {
                break;
            }
        }
    })
}

async fn manager_index_handler() -> Result<Response, AppError> {
    serve_manager_file("index.html".to_string()).await
}

async fn manager_handler(AxumPath(path): AxumPath<String>) -> Result<Response, AppError> {
    let normalized = path.trim_start_matches('/');
    let requested = if normalized.is_empty() {
        "index.html".to_string()
    } else {
        normalized.to_string()
    };
    serve_manager_file(requested).await
}

async fn serve_manager_file(requested: String) -> Result<Response, AppError> {
    let base = manager_ui::resolve_manager_dist();
    let target = base.join(requested);

    let target = if target.exists() {
        target
    } else {
        base.join("index.html")
    };

    serve_file(target).await
}

async fn assets_handler(AxumPath(file): AxumPath<String>) -> Result<Response, AppError> {
    let base = manager_ui::resolve_manager_dist();
    let normalized = file.trim_start_matches('/');
    let path = manager_ui::secure_assets_path(base.as_path(), normalized)?;
    serve_file(path).await
}

async fn serve_file(path: PathBuf) -> Result<Response, AppError> {
    let data = fs::read(&path).await?;
    let content_type = mime_guess::from_path(&path)
        .first_or_octet_stream()
        .essence_str()
        .to_string();

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", content_type)
        .body(Body::from(data))
        .map_err(|error| AppError::internal(error.to_string()))?)
}

async fn not_found_handler(method: Method, uri: Uri) -> Result<Response, AppError> {
    Err::<Response, AppError>(AppError::new(
        StatusCode::NOT_FOUND,
        "Not Found",
        json!([format!("Cannot {} {}", method.as_str().to_uppercase(), uri.path())]),
    ))
}

fn client_ip(headers: &HeaderMap) -> Option<std::net::IpAddr> {
    let forwarded = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .and_then(|value| std::net::IpAddr::from_str(value.trim()).ok());

    if forwarded.is_some() {
        return forwarded;
    }

    headers
        .get("x-real-ip")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| std::net::IpAddr::from_str(value.trim()).ok())
}

async fn telemetry_log(state: &AppState, headers: &HeaderMap, path: &str) {
    if !state.config.telemetry.enabled {
        return;
    }

    let user_agent = headers
        .get("user-agent")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown");

    info!("telemetry path={} user_agent={}", path, user_agent);
}
