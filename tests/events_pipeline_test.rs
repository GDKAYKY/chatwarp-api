use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use tokio::sync::broadcast;
use tokio::time::Duration;

use chatwarp_api::{
    events::{
        DispatcherOutputs,
        EventDispatcher,
        EventPipelineError,
        RabbitMqPublisher,
        WebSocketHub,
        WebhookDispatcher,
        WebhookTransport,
    },
    wa::events::Event,
};

#[tokio::test]
async fn dispatcher_routes_event_to_ws_and_rabbitmq() -> anyhow::Result<()> {
    let websocket = WebSocketHub::new(16);
    let mut websocket_rx = websocket.subscribe();

    let (rabbitmq, mut rabbit_rx) = RabbitMqPublisher::new("evolution_exchange".to_owned(), 16);

    let dispatcher = EventDispatcher::new(DispatcherOutputs {
        webhook: None,
        websocket: Some(websocket.clone()),
        rabbitmq: Some(rabbitmq),
    });

    let event = Event::Connected {
        instance_name: "inst-a".to_owned(),
    };
    dispatcher.dispatch("inst-a", &event).await?;

    let ws_payload = websocket_rx.recv().await?;
    let ws_event: Event = serde_json::from_str(&ws_payload)?;
    assert_eq!(ws_event, event);

    let rabbit_payload = rabbit_rx
        .recv()
        .await
        .ok_or_else(|| anyhow::anyhow!("rabbit payload missing"))?;
    assert_eq!(rabbit_payload.exchange, "evolution_exchange");
    assert_eq!(rabbit_payload.routing_key, "inst-a.connected");

    Ok(())
}

#[tokio::test]
async fn webhook_dispatcher_retries_then_succeeds() -> anyhow::Result<()> {
    let transport = Arc::new(FlakyWebhookTransport::new(2));

    let dispatcher = WebhookDispatcher::new(
        "http://localhost/webhook".to_owned(),
        Duration::from_millis(100),
        3,
        Duration::from_millis(1),
        transport.clone(),
    );

    let event = Event::QrCode("qr:abc".to_owned());
    dispatcher.dispatch(&event).await?;

    assert_eq!(transport.calls(), 3);
    Ok(())
}

#[tokio::test]
async fn webhook_dispatcher_returns_failed_after_retries() -> anyhow::Result<()> {
    let transport = Arc::new(FlakyWebhookTransport::new(10));

    let dispatcher = WebhookDispatcher::new(
        "http://localhost/webhook".to_owned(),
        Duration::from_millis(100),
        2,
        Duration::from_millis(1),
        transport.clone(),
    );

    let event = Event::QrCode("qr:abc".to_owned());
    let error = dispatcher.dispatch(&event).await.expect_err("must fail");
    assert_eq!(error.to_string(), EventPipelineError::WebhookFailed.to_string());

    Ok(())
}

#[tokio::test]
async fn dispatcher_run_consumes_instance_receiver() -> anyhow::Result<()> {
    let websocket = WebSocketHub::new(16);
    let mut websocket_rx = websocket.subscribe();

    let dispatcher = EventDispatcher::new(DispatcherOutputs {
        webhook: None,
        websocket: Some(websocket),
        rabbitmq: None,
    });

    let (tx, rx) = broadcast::channel(16);

    let run_task = tokio::spawn(async move { dispatcher.run("inst-b", rx).await });

    tx.send(Event::Connected {
        instance_name: "inst-b".to_owned(),
    })?;
    drop(tx);

    let ws_payload = websocket_rx.recv().await?;
    let ws_event: Event = serde_json::from_str(&ws_payload)?;
    assert_eq!(
        ws_event,
        Event::Connected {
            instance_name: "inst-b".to_owned()
        }
    );

    run_task.await??;
    Ok(())
}

struct FlakyWebhookTransport {
    fail_times: usize,
    calls: AtomicUsize,
}

impl FlakyWebhookTransport {
    fn new(fail_times: usize) -> Self {
        Self {
            fail_times,
            calls: AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::Relaxed)
    }
}

impl WebhookTransport for FlakyWebhookTransport {
    fn post<'a>(
        &'a self,
        _url: &'a str,
        _payload: String,
    ) -> Pin<Box<dyn Future<Output = Result<(), EventPipelineError>> + Send + 'a>> {
        Box::pin(async move {
            let current = self.calls.fetch_add(1, Ordering::Relaxed);
            if current < self.fail_times {
                Err(EventPipelineError::WebhookFailed)
            } else {
                Ok(())
            }
        })
    }
}
