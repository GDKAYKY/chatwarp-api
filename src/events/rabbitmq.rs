use tokio::sync::mpsc;

use crate::{
    events::error::EventPipelineError,
    wa::events::Event,
};

/// Synthetic RabbitMQ publish payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RabbitMqMessage {
    /// Exchange name.
    pub exchange: String,
    /// Routing key in `{instance}.{event_type}` format.
    pub routing_key: String,
    /// Serialized JSON payload.
    pub payload: String,
}

/// Synthetic RabbitMQ publisher using bounded channel.
#[derive(Clone)]
pub struct RabbitMqPublisher {
    exchange_name: String,
    tx: mpsc::Sender<RabbitMqMessage>,
}

impl RabbitMqPublisher {
    /// Creates publisher and corresponding consumer receiver.
    pub fn new(exchange_name: String, capacity: usize) -> (Self, mpsc::Receiver<RabbitMqMessage>) {
        let (tx, rx) = mpsc::channel(capacity);
        (
            Self {
                exchange_name,
                tx,
            },
            rx,
        )
    }

    /// Publishes an event with derived routing key.
    pub fn publish(&self, instance_name: &str, event: &Event) -> Result<(), EventPipelineError> {
        let payload = serde_json::to_string(event)?;
        let message = RabbitMqMessage {
            exchange: self.exchange_name.clone(),
            routing_key: format!("{instance_name}.{}", event.event_type()),
            payload,
        };

        self.tx.try_send(message).map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => EventPipelineError::RabbitMqQueueFull,
            mpsc::error::TrySendError::Closed(_) => EventPipelineError::RabbitMqQueueClosed,
        })
    }
}
