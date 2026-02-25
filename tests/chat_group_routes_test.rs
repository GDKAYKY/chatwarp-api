use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use chatwarp_api::app::{AppState, build_router};

#[tokio::test]
async fn chat_and_group_routes_work_for_existing_instance() -> anyhow::Result<()> {
    let app = build_router(AppState::new());

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/instance/create")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"m9"}"#))?,
        )
        .await?;
    assert_eq!(create.status(), StatusCode::CREATED);

    let find_messages = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/chat/findMessages/m9")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"remote_jid":"5511999999999@s.whatsapp.net","limit":2}"#,
                ))?,
        )
        .await?;
    assert_eq!(find_messages.status(), StatusCode::OK);

    let create_group = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/group/create/m9")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"subject":"Squad","participants":["5511888888888@s.whatsapp.net"]}"#,
                ))?,
        )
        .await?;
    assert_eq!(create_group.status(), StatusCode::CREATED);

    let fetch_groups = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/group/fetchAllGroups/m9")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(fetch_groups.status(), StatusCode::OK);
    let fetch_groups_body = to_bytes(fetch_groups.into_body(), usize::MAX).await?;
    let fetch_groups_json: serde_json::Value = serde_json::from_slice(&fetch_groups_body)?;
    assert_eq!(fetch_groups_json["groups"].as_array().map_or(0, |v| v.len()), 1);

    let find_chats = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/chat/findChats/m9")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(find_chats.status(), StatusCode::OK);
    let find_chats_body = to_bytes(find_chats.into_body(), usize::MAX).await?;
    let find_chats_json: serde_json::Value = serde_json::from_slice(&find_chats_body)?;
    assert!(find_chats_json["chats"]
        .as_array()
        .is_some_and(|chats| chats.len() >= 3));

    Ok(())
}

#[tokio::test]
async fn chat_and_group_routes_fail_for_missing_instance() -> anyhow::Result<()> {
    let app = build_router(AppState::new());

    let missing_chat = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/chat/findMessages/missing")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"remote_jid":"5511999999999@s.whatsapp.net"}"#,
                ))?,
        )
        .await?;
    assert_eq!(missing_chat.status(), StatusCode::NOT_FOUND);

    let missing_group = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/group/fetchAllGroups/missing")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(missing_group.status(), StatusCode::NOT_FOUND);

    Ok(())
}
