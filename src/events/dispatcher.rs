use tokio::sync::broadcast;

use crate::{
    events::{
        error::EventPipelineError,
        rabbitmq::RabbitMqPublisher,
        webhook::WebhookDispatcher,
        websocket::WebSocketHub,
    },
    wa::events::Event,
};

/// Dispatch outputs configured for a single instance.
#[derive(Clone, Default)]
pub struct DispatcherOutputs {
    /// Optional webhook dispatcher.
    pub webhook: Option<WebhookDispatcher>,
    /// Optional websocket broadcaster.
    pub websocket: Option<WebSocketHub>,
    /// Optional rabbitmq publisher.
    pub rabbitmq: Option<RabbitMqPublisher>,
}

/// Instance event dispatcher for multiple output channels.
#[derive(Clone, Default)]
pub struct EventDispatcher {
    outputs: DispatcherOutputs,
}

impl EventDispatcher {
    /// Creates dispatcher from configured outputs.
    pub fn new(outputs: DispatcherOutputs) -> Self {
        Self { outputs }
    }

    /// Dispatches a single event to all enabled outputs.
    pub async fn dispatch(
        &self,
        instance_name: &str,
        event: &Event,
    ) -> Result<(), EventPipelineError> {
        if let Some(webhook) = &self.outputs.webhook {
            webhook.dispatch(event).await?;
        }

        if let Some(websocket) = &self.outputs.websocket {
            websocket.broadcast_event(event)?;
        }

        if let Some(rabbitmq) = &self.outputs.rabbitmq {
            rabbitmq.publish(instance_name, event)?;
        }

        Ok(())
    }

    /// Runs a forwarding loop from instance broadcast receiver.
    pub async fn run(
        &self,
        instance_name: &str,
        mut rx: broadcast::Receiver<Event>,
    ) -> Result<(), EventPipelineError> {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    self.dispatch(instance_name, &event).await?;
                }
                Err(broadcast::error::RecvError::Closed) => return Ok(()),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    }
}
