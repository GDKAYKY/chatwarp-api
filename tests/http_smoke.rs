use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;

use chatwarp_api::{
    config::AppConfig,
    http,
    state::{AppState, RuntimeInstance},
};

#[tokio::test]
async fn root_route_returns_welcome_payload() {
    let config = AppConfig::default();
    let state = AppState::new_for_tests(config).await.expect("state");
    let app = http::build_router(state);

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(json["status"], 200);
    assert_eq!(json["message"], "Welcome to the Evolution API, it is working!");
}

#[tokio::test]
async fn unknown_route_returns_evolution_not_found_shape() {
    let config = AppConfig::default();
    let state = AppState::new_for_tests(config).await.expect("state");
    let app = http::build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(json["status"], 404);
    assert_eq!(json["error"], "Not Found");
}

#[tokio::test]
async fn guarded_route_rejects_without_apikey() {
    let config = AppConfig::default();
    let state = AppState::new_for_tests(config).await.expect("state");
    state.wa_instances.write().await.insert(
        "demo".to_string(),
        RuntimeInstance {
            integration: "WHATSAPP-BAILEYS".to_string(),
            state: "open".to_string(),
        },
    );

    let app = http::build_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/instance/connectionState/demo")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
