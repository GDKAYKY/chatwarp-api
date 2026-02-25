use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use chatwarp_api::app::{AppState, build_router};

#[tokio::test]
async fn responses_include_request_id_and_metrics_track_requests() -> anyhow::Result<()> {
    let app = build_router(AppState::new());

    let health = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/healthz")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(health.status(), StatusCode::OK);
    assert!(health.headers().get("x-request-id").is_some());

    let metrics = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/metrics")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(metrics.status(), StatusCode::OK);
    let body = to_bytes(metrics.into_body(), usize::MAX).await?;
    let json: serde_json::Value = serde_json::from_slice(&body)?;

    assert!(json["requests_total"].as_u64().unwrap_or(0) >= 2);
    assert_eq!(json["instances_total"].as_u64().unwrap_or(1), 0);
    Ok(())
}

#[tokio::test]
async fn request_body_limit_rejects_oversized_payload() -> anyhow::Result<()> {
    let app = build_router(AppState::new());

    let oversized_name = "x".repeat(300 * 1024);
    let body = format!(r#"{{"name":"{oversized_name}"}}"#);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/instance/create")
                .header("content-type", "application/json")
                .body(Body::from(body))?,
        )
        .await?;

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    Ok(())
}
