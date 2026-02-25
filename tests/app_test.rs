use axum::{
    body::{Body, to_bytes},
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
                .uri("/chat/unknownRoute/demo")
                .body(Body::empty())
                .expect("request build"),
        )
        .await
        .expect("fallback request should succeed");

    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
}

#[tokio::test]
async fn swagger_and_openapi_routes_are_available() {
    let app = build_router(AppState::new());

    let swagger = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/docs/swagger")
                .body(Body::empty())
                .expect("request build"),
        )
        .await
        .expect("swagger request should succeed");
    assert_eq!(swagger.status(), StatusCode::OK);

    let openapi = app
        .oneshot(
            Request::builder()
                .uri("/docs/openapi.json")
                .body(Body::empty())
                .expect("request build"),
        )
        .await
        .expect("openapi request should succeed");
    assert_eq!(openapi.status(), StatusCode::OK);

    let body = to_bytes(openapi.into_body(), usize::MAX)
        .await
        .expect("openapi body");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("valid json");
    assert_eq!(payload["openapi"], "3.0.3");
}
