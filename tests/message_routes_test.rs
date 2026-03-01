mod common;

use std::{
    sync::Arc,
    time::Duration,
};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use chatwarp_api::{
    app::{AppState, build_router},
    db::auth_store::InMemoryAuthStore,
    instance::InstanceManager,
};
use common::wa_mock::start_mock_wa_server;

#[tokio::test]
async fn message_route_send_text_returns_message_key() -> anyhow::Result<()> {
    let server = start_mock_wa_server(
        Some("2@m7-reference"),
        Some("5511777777777@s.whatsapp.net"),
        true,
    )
    .await?;
    let manager = InstanceManager::new_with_runtime(
        Arc::new(InMemoryAuthStore::new()),
        server.url.clone(),
    );
    let app = build_router(AppState::with_instance_manager(
        256 * 1024,
        manager,
    ));

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/instance/create")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"m7"}"#))?,
        )
        .await?;
    assert_eq!(create.status(), StatusCode::CREATED);

    let connect = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/instance/connect/m7")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(connect.status(), StatusCode::OK);

    let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
    loop {
        let state_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/instance/m7/state")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(state_response.status(), StatusCode::OK);
        let body = to_bytes(state_response.into_body(), usize::MAX).await?;
        let payload: serde_json::Value = serde_json::from_slice(&body)?;
        if payload["connected"] == true {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!("instance did not connect within timeout");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let message = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/message/sendText/m7")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"to":"123@s.whatsapp.net","content":{"type":"text","text":"hello"}}"#,
                ))?,
        )
        .await?;

    assert_eq!(message.status(), StatusCode::OK);

    let delete = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/instance/delete/m7")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(delete.status(), StatusCode::OK);

    server.finish().await?;
    Ok(())
}

#[tokio::test]
async fn message_route_rejects_send_when_instance_not_connected() -> anyhow::Result<()> {
    let app = build_router(AppState::new());

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/instance/create")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"m7-offline"}"#))?,
        )
        .await?;
    assert_eq!(create.status(), StatusCode::CREATED);

    let message = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/message/sendText/m7-offline")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"to":"123@s.whatsapp.net","content":{"type":"text","text":"hello"}}"#,
                ))?,
        )
        .await?;

    assert_eq!(message.status(), StatusCode::CONFLICT);
    Ok(())
}

#[tokio::test]
async fn message_route_rejects_invalid_operation() -> anyhow::Result<()> {
    let app = build_router(AppState::new());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/message/nope/demo")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"to":"123@s.whatsapp.net","content":{"type":"text","text":"hello"}}"#,
                ))?,
        )
        .await?;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn message_route_non_text_operation_returns_not_implemented() -> anyhow::Result<()> {
    let app = build_router(AppState::new());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/message/sendButtons/demo")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"to":"123@s.whatsapp.net","content":{"type":"buttons","text":"x","buttons":["a","b"]}}"#,
                ))?,
        )
        .await?;

    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    Ok(())
}
