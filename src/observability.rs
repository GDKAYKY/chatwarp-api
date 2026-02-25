use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::Instant;

use axum::http::StatusCode;
use serde::Serialize;

#[derive(Default)]
struct MetricsInner {
    request_sequence: AtomicU64,
    requests_total: AtomicU64,
    inflight_requests: AtomicU64,
    responses_2xx: AtomicU64,
    responses_4xx: AtomicU64,
    responses_5xx: AtomicU64,
    responses_other: AtomicU64,
}

/// Snapshot exposed by `/metrics`.
#[derive(Debug, Clone, Serialize)]
pub struct MetricsSnapshot {
    pub uptime_seconds: u64,
    pub instances_total: usize,
    pub requests_total: u64,
    pub inflight_requests: u64,
    pub responses_2xx: u64,
    pub responses_4xx: u64,
    pub responses_5xx: u64,
    pub responses_other: u64,
}

/// Request metrics registry for M10 hardening/observability.
#[derive(Clone)]
pub struct RequestMetrics {
    inner: Arc<MetricsInner>,
    started_at: Instant,
}

impl RequestMetrics {
    /// Creates a new metrics registry.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MetricsInner::default()),
            started_at: Instant::now(),
        }
    }

    /// Registers a request start and returns a stable request id.
    pub fn begin_request(&self) -> u64 {
        let request_id = self.inner.request_sequence.fetch_add(1, Ordering::Relaxed) + 1;
        self.inner.requests_total.fetch_add(1, Ordering::Relaxed);
        self.inner.inflight_requests.fetch_add(1, Ordering::Relaxed);
        request_id
    }

    /// Registers a request end using final HTTP status code.
    pub fn end_request(&self, status: StatusCode) {
        let previous = self.inner.inflight_requests.fetch_sub(1, Ordering::Relaxed);
        if previous == 0 {
            self.inner.inflight_requests.store(0, Ordering::Relaxed);
        }

        if status.is_success() {
            self.inner.responses_2xx.fetch_add(1, Ordering::Relaxed);
        } else if status.is_client_error() {
            self.inner.responses_4xx.fetch_add(1, Ordering::Relaxed);
        } else if status.is_server_error() {
            self.inner.responses_5xx.fetch_add(1, Ordering::Relaxed);
        } else {
            self.inner.responses_other.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Returns the current serialized metrics snapshot.
    pub fn snapshot(&self, instances_total: usize) -> MetricsSnapshot {
        MetricsSnapshot {
            uptime_seconds: self.started_at.elapsed().as_secs(),
            instances_total,
            requests_total: self.inner.requests_total.load(Ordering::Relaxed),
            inflight_requests: self.inner.inflight_requests.load(Ordering::Relaxed),
            responses_2xx: self.inner.responses_2xx.load(Ordering::Relaxed),
            responses_4xx: self.inner.responses_4xx.load(Ordering::Relaxed),
            responses_5xx: self.inner.responses_5xx.load(Ordering::Relaxed),
            responses_other: self.inner.responses_other.load(Ordering::Relaxed),
        }
    }
}

impl Default for RequestMetrics {
    fn default() -> Self {
        Self::new()
    }
}
