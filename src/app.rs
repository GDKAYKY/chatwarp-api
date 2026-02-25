use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use axum::{
    Json, Router,
    extract::{OriginalUri, State},
    response::IntoResponse,
    routing::get,
};
use http::StatusCode;
use serde::Serialize;

use crate::error::not_implemented_response;

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    ready: Arc<AtomicBool>,
}

impl AppState {
    /// Creates a new app state with readiness disabled.
    pub fn new() -> Self {
        Self {
            ready: Arc::new(AtomicBool::new(false)),
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

/// Builds the root HTTP router.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(root_handler))
        .route("/healthz", get(healthz_handler))
        .route("/readyz", get(readyz_handler))
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

async fn not_implemented_handler(uri: OriginalUri) -> impl IntoResponse {
    not_implemented_response(uri.0.path().to_owned())
}
