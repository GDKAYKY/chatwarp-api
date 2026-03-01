mod common;

use std::{
    sync::Arc,
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
async fn instance_routes_create_connect_state_delete() -> anyhow::Result<()> {
    let server = start_mock_wa_server(
        Some("2@instance-routes"),
        Some("5511888888888@s.whatsapp.net"),
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
    let connect_body = to_bytes(connect_response.into_body(), usize::MAX).await?;
    let connect_payload: serde_json::Value = serde_json::from_slice(&connect_body)?;
    let connect_state = connect_payload["status"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing connect status field"))?;
    assert!(["connecting", "qr_pending", "connected", "disconnected"].contains(&connect_state));

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

    let detailed_state_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/instance/demo/state")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(detailed_state_response.status(), StatusCode::OK);
    let detailed_state_body = to_bytes(detailed_state_response.into_body(), usize::MAX).await?;
    let detailed_state_payload: serde_json::Value = serde_json::from_slice(&detailed_state_body)?;
    let state = detailed_state_payload["state"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing state field"))?;
    assert!(["connecting", "qr_pending", "connected", "disconnected"].contains(&state));
    assert!(detailed_state_payload["connected"].is_boolean());

    let delete_response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/instance/delete/demo")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(delete_response.status(), StatusCode::OK);
    server.finish().await?;

    Ok(())
}

#[tokio::test]
async fn instance_create_rejects_blank_name() -> anyhow::Result<()> {
    let app = build_router(AppState::new());

    let create_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/instance/create")
                .header("content-type", "application/json")
                .body(Body::from("{\"name\":\"   \"}"))?,
        )
        .await?;

    assert_eq!(create_response.status(), StatusCode::BAD_REQUEST);
    Ok(())
}
