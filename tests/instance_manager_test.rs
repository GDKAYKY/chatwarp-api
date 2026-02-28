mod common;

use std::sync::Arc;

use chatwarp_api::instance::{
    InstanceConfig,
    InstanceManager,
    handle::ConnectionState,
    runner::backoff_seconds,
};
use chatwarp_api::{
    db::auth_store::{AuthStore, InMemoryAuthStore},
    wa::auth::{AuthState, MeInfo},
    wa::events::Event,
};
use common::wa_mock::start_mock_wa_server;

#[tokio::test]
async fn manager_create_connect_delete_flow() -> anyhow::Result<()> {
    let server = start_mock_wa_server(
        Some("2@alpha-reference"),
        Some("5511999999999@s.whatsapp.net"),
        true,
    )
    .await?;
    let manager = InstanceManager::new_with_runtime(
        Arc::new(InMemoryAuthStore::new()),
        server.url.clone(),
    );

    manager
        .create(
            "alpha",
            InstanceConfig {
                auto_connect: false,
            },
        )
        .await?;

    let handle = manager
        .get("alpha")
        .await
        .ok_or_else(|| anyhow::anyhow!("missing alpha instance"))?;

    let initial_state = handle.connection_state().await;
    assert_eq!(initial_state, ConnectionState::Disconnected);

    let mut events = handle.subscribe();
    handle.connect().await?;

    let reconnect_event =
        tokio::time::timeout(std::time::Duration::from_secs(1), events.recv()).await??;
    assert_eq!(reconnect_event, Event::ReconnectScheduled {
        instance_name: "alpha".to_string(),
        delay_secs: 1,
    });

    let qr_event = tokio::time::timeout(std::time::Duration::from_secs(1), events.recv()).await??;
    let Event::QrCode(qr_payload) = qr_event else {
        anyhow::bail!("expected qr event");
    };
    assert!(qr_payload.starts_with("2@alpha-reference,"));

    let connected_event =
        tokio::time::timeout(std::time::Duration::from_secs(1), events.recv()).await??;
    assert_eq!(connected_event, Event::Connected {
        instance_name: "alpha".to_string(),
    });

    let connected_state = handle.connection_state().await;
    assert_eq!(connected_state, ConnectionState::Connected);

    manager.delete("alpha").await?;
    assert!(manager.get("alpha").await.is_none());
    server.finish().await?;

    Ok(())
}

#[test]
fn backoff_schedule_is_capped() {
    assert_eq!(backoff_seconds(0), 1);
    assert_eq!(backoff_seconds(1), 2);
    assert_eq!(backoff_seconds(2), 4);
    assert_eq!(backoff_seconds(3), 8);
    assert_eq!(backoff_seconds(4), 16);
    assert_eq!(backoff_seconds(5), 30);
    assert_eq!(backoff_seconds(9), 30);
}

#[tokio::test]
async fn manager_reconnects_with_persisted_auth_without_qr() -> anyhow::Result<()> {
    let store = Arc::new(InMemoryAuthStore::new());
    let mut saved_auth = AuthState::new();
    saved_auth.metadata.me = Some(MeInfo {
        jid: "5511666666666@s.whatsapp.net".to_owned(),
        push_name: Some("Persisted".to_owned()),
    });
    store.save("persisted", &saved_auth).await?;

    let server = start_mock_wa_server(None, Some("5511666666666@s.whatsapp.net"), false).await?;
    let manager = InstanceManager::new_with_runtime(store, server.url.clone());
    manager
        .create(
            "persisted",
            InstanceConfig {
                auto_connect: false,
            },
        )
        .await?;

    let handle = manager
        .get("persisted")
        .await
        .ok_or_else(|| anyhow::anyhow!("missing persisted instance"))?;
    let mut events = handle.subscribe();
    handle.connect().await?;

    let reconnect_event =
        tokio::time::timeout(std::time::Duration::from_secs(1), events.recv()).await??;
    assert_eq!(reconnect_event, Event::ReconnectScheduled {
        instance_name: "persisted".to_owned(),
        delay_secs: 1,
    });

    let connected_event =
        tokio::time::timeout(std::time::Duration::from_secs(1), events.recv()).await??;
    assert_eq!(connected_event, Event::Connected {
        instance_name: "persisted".to_owned(),
    });

    manager.delete("persisted").await?;
    server.finish().await?;
    Ok(())
}
