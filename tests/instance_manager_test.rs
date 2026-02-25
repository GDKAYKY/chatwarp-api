use chatwarp_api::instance::{
    InstanceConfig,
    InstanceManager,
    handle::ConnectionState,
    runner::backoff_seconds,
};

#[tokio::test]
async fn manager_create_connect_delete_flow() -> anyhow::Result<()> {
    let manager = InstanceManager::new();

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

    let qr_event = tokio::time::timeout(std::time::Duration::from_millis(300), events.recv()).await??;
    assert_eq!(qr_event, chatwarp_api::wa::events::Event::ReconnectScheduled {
        instance_name: "alpha".to_string(),
        delay_secs: 1,
    });

    let qr_event = tokio::time::timeout(std::time::Duration::from_millis(300), events.recv()).await??;
    assert_eq!(qr_event, chatwarp_api::wa::events::Event::QrCode("qr:alpha:synthetic".to_string()));

    let connected_state = handle.connection_state().await;
    assert_eq!(connected_state, ConnectionState::QrPending);

    manager.delete("alpha").await?;
    assert!(manager.get("alpha").await.is_none());

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
