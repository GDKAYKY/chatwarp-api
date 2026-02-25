use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use chatwarp_api::app::{AppState, build_router};

#[tokio::test]
async fn instance_routes_create_connect_state_delete() -> anyhow::Result<()> {
    let app = build_router(AppState::new());

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/instance/create")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"demo"}"#))?,
        )
        .await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);

    let connect_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/instance/connect/demo")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(connect_response.status(), StatusCode::OK);

    let state_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/instance/connectionState/demo")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(state_response.status(), StatusCode::OK);

    let delete_response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/instance/delete/demo")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(delete_response.status(), StatusCode::OK);

    Ok(())
}
