use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use axum::{
    Json, Router,
    extract::{OriginalUri, Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use tokio::time::{Duration, Instant, timeout};

use crate::{
    error::not_implemented_response,
    handlers::message::post_message_handler,
    instance::{
        InstanceConfig, InstanceManager,
        error::InstanceError,
        handle::ConnectionState,
    },
    wa::events::Event,
};

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    ready: Arc<AtomicBool>,
    instance_manager: InstanceManager,
}

impl AppState {
    /// Creates a new app state with readiness disabled.
    pub fn new() -> Self {
        Self {
            ready: Arc::new(AtomicBool::new(false)),
            instance_manager: InstanceManager::new(),
        }
    }

    /// Sets readiness status.
    pub fn set_ready(&self, ready: bool) {
        self.ready.store(ready, Ordering::Relaxed);
    }

    /// Returns readiness status.
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }

    /// Returns a clone of the global instance manager.
    pub fn instance_manager(&self) -> InstanceManager {
        self.instance_manager.clone()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Serialize)]
struct RootResponse {
    name: &'static str,
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    ok: bool,
}

#[derive(Debug, Serialize)]
struct InstanceOkResponse {
    instance: String,
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct ConnectionStateResponse {
    instance: String,
    state: &'static str,
}

#[derive(Debug, Serialize)]
struct ConnectResponse {
    instance: String,
    state: &'static str,
    qr: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateInstanceRequest {
    name: String,
    auto_connect: Option<bool>,
}

#[derive(Debug, Serialize)]
struct ApiErrorResponse {
    error: &'static str,
    message: String,
}

/// Builds the root HTTP router.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(root_handler))
        .route("/healthz", get(healthz_handler))
        .route("/readyz", get(readyz_handler))
        .route("/instance/create", post(create_instance_handler))
        .route("/instance/delete/:name", delete(delete_instance_handler))
        .route(
            "/instance/connectionState/:name",
            get(connection_state_handler),
        )
        .route("/instance/connect/:name", get(connect_instance_handler))
        .route("/message/:operation/:instance_name", post(post_message_handler))
        .fallback(not_implemented_handler)
        .with_state(state)
}

async fn root_handler() -> impl IntoResponse {
    Json(RootResponse {
        name: "chatwarp-api",
        status: "ok",
    })
}

async fn healthz_handler() -> impl IntoResponse {
    Json(HealthResponse { ok: true })
}

async fn readyz_handler(State(state): State<AppState>) -> impl IntoResponse {
    if state.is_ready() {
        (StatusCode::OK, Json(HealthResponse { ok: true })).into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(HealthResponse { ok: false })).into_response()
    }
}

async fn create_instance_handler(
    State(state): State<AppState>,
    Json(request): Json<CreateInstanceRequest>,
) -> impl IntoResponse {
    let manager = state.instance_manager();
    let config = InstanceConfig {
        auto_connect: request.auto_connect.unwrap_or(false),
    };

    match manager.create(&request.name, config).await {
        Ok(()) => (
            StatusCode::CREATED,
            Json(InstanceOkResponse {
                instance: request.name,
                status: "created",
            }),
        )
            .into_response(),
        Err(error) => map_instance_error(error),
    }
}

async fn delete_instance_handler(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let manager = state.instance_manager();

    match manager.delete(&name).await {
        Ok(()) => (
            StatusCode::OK,
            Json(InstanceOkResponse {
                instance: name,
                status: "deleted",
            }),
        )
            .into_response(),
        Err(error) => map_instance_error(error),
    }
}

async fn connection_state_handler(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let manager = state.instance_manager();

    let Some(handle) = manager.get(&name).await else {
        return map_instance_error(InstanceError::NotFound);
    };

    let current_state = handle.connection_state().await;
    (
        StatusCode::OK,
        Json(ConnectionStateResponse {
            instance: name,
            state: current_state.as_str(),
        }),
    )
        .into_response()
}

async fn connect_instance_handler(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let manager = state.instance_manager();

    let Some(handle) = manager.get(&name).await else {
        return map_instance_error(InstanceError::NotFound);
    };

    let current_state = handle.connection_state().await;
    if current_state == ConnectionState::Connected {
        return (
            StatusCode::CONFLICT,
            Json(ApiErrorResponse {
                error: "instance_already_connected",
                message: format!("instance {name} is already connected"),
            }),
        )
            .into_response();
    }

    let mut events = handle.subscribe();
    if let Err(error) = handle.connect().await {
        return map_instance_error(error);
    }

    let qr = wait_for_qr_event(&mut events, Duration::from_millis(300)).await;

    let state_after = handle.connection_state().await;

    (
        StatusCode::OK,
        Json(ConnectResponse {
            instance: name,
            state: state_after.as_str(),
            qr,
        }),
    )
        .into_response()
}

fn map_instance_error(error: InstanceError) -> axum::response::Response {
    match error {
        InstanceError::AlreadyExists => (
            StatusCode::CONFLICT,
            Json(ApiErrorResponse {
                error: "instance_already_exists",
                message: "instance already exists".to_owned(),
            }),
        )
            .into_response(),
        InstanceError::NotFound => (
            StatusCode::NOT_FOUND,
            Json(ApiErrorResponse {
                error: "instance_not_found",
                message: "instance not found".to_owned(),
            }),
        )
            .into_response(),
        InstanceError::CommandChannelClosed => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiErrorResponse {
                error: "instance_unavailable",
                message: "instance command channel is unavailable".to_owned(),
            }),
        )
            .into_response(),
    }
}

async fn wait_for_qr_event(
    events: &mut tokio::sync::broadcast::Receiver<Event>,
    max_wait: Duration,
) -> Option<String> {
    let deadline = Instant::now() + max_wait;

    loop {
        let now = Instant::now();
        if now >= deadline {
            return None;
        }

        let remaining = deadline.saturating_duration_since(now);
        match timeout(remaining, events.recv()).await {
            Ok(Ok(Event::QrCode(value))) => return Some(value),
            Ok(Ok(_)) => continue,
            Ok(Err(_)) => return None,
            Err(_) => return None,
        }
    }
}

async fn not_implemented_handler(uri: OriginalUri) -> impl IntoResponse {
    not_implemented_response(uri.0.path().to_owned())
}
