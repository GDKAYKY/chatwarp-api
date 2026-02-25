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
