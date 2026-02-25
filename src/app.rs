use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, OriginalUri, Path, State},
    http::{HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use tokio::time::{Duration, Instant, timeout};

use crate::{
    error::not_implemented_response,
    group_store::GroupStore,
    handlers::{
        chat::{find_chats_handler, find_messages_handler},
        group::{create_group_handler, fetch_all_groups_handler},
        message::post_message_handler,
    },
    instance::{
        InstanceConfig, InstanceManager,
        error::InstanceError,
        handle::ConnectionState,
    },
    openapi::{openapi_document, swagger_ui},
    observability::{MetricsSnapshot, RequestMetrics},
    wa::events::Event,
};

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    ready: Arc<AtomicBool>,
    instance_manager: InstanceManager,
    group_store: GroupStore,
    metrics: RequestMetrics,
    connect_wait_timeout: Duration,
    max_body_bytes: usize,
}

impl AppState {
    /// Creates a new app state with readiness disabled.
    pub fn new() -> Self {
        Self::with_runtime_tuning(Duration::from_millis(300), 256 * 1024)
    }

    /// Creates state using explicit hardening/timeout tuning.
    pub fn with_runtime_tuning(connect_wait_timeout: Duration, max_body_bytes: usize) -> Self {
        Self {
            ready: Arc::new(AtomicBool::new(false)),
            instance_manager: InstanceManager::new(),
            group_store: GroupStore::new(),
            metrics: RequestMetrics::new(),
            connect_wait_timeout,
            max_body_bytes: max_body_bytes.max(1024),
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

    /// Returns the in-memory group store.
    pub(crate) fn group_store(&self) -> GroupStore {
        self.group_store.clone()
    }

    /// Returns request metrics registry.
    pub(crate) fn metrics(&self) -> RequestMetrics {
        self.metrics.clone()
    }

    /// Returns max wait for QR event while handling connect route.
    pub(crate) fn connect_wait_timeout(&self) -> Duration {
        self.connect_wait_timeout
    }

    /// Returns max accepted request body size in bytes.
    pub(crate) fn max_body_bytes(&self) -> usize {
        self.max_body_bytes
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
    let body_limit = state.max_body_bytes();

    Router::new()
        .route("/", get(root_handler))
        .route("/swagger", get(swagger_handler))
        .route("/openapi.json", get(openapi_handler))
        .route("/healthz", get(healthz_handler))
        .route("/readyz", get(readyz_handler))
        .route("/metrics", get(metrics_handler))
        .route("/instance/create", post(create_instance_handler))
        .route("/instance/delete/:name", delete(delete_instance_handler))
        .route(
            "/instance/connectionState/:name",
            get(connection_state_handler),
        )
        .route("/instance/connect/:name", get(connect_instance_handler))
        .route("/message/:operation/:instance_name", post(post_message_handler))
        .route("/chat/findMessages/:instance_name", post(find_messages_handler))
        .route("/chat/findChats/:instance_name", get(find_chats_handler))
        .route("/group/create/:instance_name", post(create_group_handler))
        .route(
            "/group/fetchAllGroups/:instance_name",
            get(fetch_all_groups_handler),
        )
        .fallback(not_implemented_handler)
        .layer(DefaultBodyLimit::max(body_limit))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            request_observability_middleware,
        ))
        .with_state(state)
}

async fn root_handler() -> impl IntoResponse {
    Json(RootResponse {
        name: "chatwarp-api",
        status: "ok",
    })
}

async fn swagger_handler() -> impl IntoResponse {
    swagger_ui()
}

async fn openapi_handler() -> impl IntoResponse {
    Json(openapi_document())
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

async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    let instances_total = state.instance_manager().count().await;
    let snapshot: MetricsSnapshot = state.metrics().snapshot(instances_total);
    Json(snapshot)
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

    let qr = wait_for_qr_event(&mut events, state.connect_wait_timeout()).await;

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

async fn request_observability_middleware(
    State(state): State<AppState>,
    request: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Response {
    let metrics = state.metrics();
    let request_id = metrics.begin_request();
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let started = Instant::now();

    let mut response = next.run(request).await;
    metrics.end_request(response.status());

    if let Ok(value) = HeaderValue::from_str(&request_id.to_string()) {
        response.headers_mut().insert("x-request-id", value);
    }

    tracing::info!(
        request_id,
        %method,
        %path,
        status = response.status().as_u16(),
        elapsed_ms = started.elapsed().as_millis(),
        "http_request"
    );

    response
}
