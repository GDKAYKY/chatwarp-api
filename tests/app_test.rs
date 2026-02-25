use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use chatwarp_api::app::{AppState, build_router};

#[tokio::test]
async fn readyz_is_503_when_not_ready_and_200_when_ready() {
    let state = AppState::new();
    let app = build_router(state.clone());

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/readyz")
                .body(Body::empty())
                .expect("request build"),
        )
        .await
        .expect("readyz request should succeed");

    assert_eq!(first.status(), StatusCode::SERVICE_UNAVAILABLE);

    state.set_ready(true);

    let second = app
        .oneshot(
            Request::builder()
                .uri("/readyz")
                .body(Body::empty())
                .expect("request build"),
        )
        .await
        .expect("readyz request should succeed");

    assert_eq!(second.status(), StatusCode::OK);
}

#[tokio::test]
async fn unknown_route_returns_not_implemented_payload() {
    let app = build_router(AppState::new());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/chat/findMessages/demo")
                .body(Body::empty())
                .expect("request build"),
        )
        .await
        .expect("fallback request should succeed");

    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
}
