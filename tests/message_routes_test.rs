use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use chatwarp_api::app::{AppState, build_router};

#[tokio::test]
async fn message_route_send_text_returns_message_key() -> anyhow::Result<()> {
    let app = build_router(AppState::new());

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

    let message = app
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
