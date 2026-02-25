use thiserror::Error;

/// Errors for event pipeline modules.
#[derive(Debug, Error)]
pub enum EventPipelineError {
    #[error("event serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("webhook transport timeout")]
    WebhookTimeout,
    #[error("webhook transport failed after retries")]
    WebhookFailed,
    #[error("rabbitmq queue is full")]
    RabbitMqQueueFull,
    #[error("rabbitmq queue is closed")]
    RabbitMqQueueClosed,
}
