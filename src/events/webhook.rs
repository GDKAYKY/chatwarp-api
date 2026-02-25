use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
};

use tokio::time::{Duration, sleep, timeout};

use crate::{
    events::error::EventPipelineError,
    wa::events::Event,
};

/// Async transport abstraction used by webhook dispatcher.
pub trait WebhookTransport: Send + Sync {
    /// Sends serialized payload to webhook URL.
    fn post<'a>(
        &'a self,
        url: &'a str,
        payload: String,
    ) -> Pin<Box<dyn Future<Output = Result<(), EventPipelineError>> + Send + 'a>>;
}

/// No-op webhook transport used as default placeholder.
#[derive(Default)]
pub struct NoopWebhookTransport;

impl WebhookTransport for NoopWebhookTransport {
    fn post<'a>(
        &'a self,
        _url: &'a str,
        _payload: String,
    ) -> Pin<Box<dyn Future<Output = Result<(), EventPipelineError>> + Send + 'a>> {
        Box::pin(async { Ok(()) })
    }
}

/// Webhook dispatcher with retries and timeout.
#[derive(Clone)]
pub struct WebhookDispatcher {
    url: String,
    timeout_per_attempt: Duration,
    max_retries: u8,
    backoff: Duration,
    transport: Arc<dyn WebhookTransport>,
}

impl WebhookDispatcher {
    /// Creates a dispatcher using provided transport.
    pub fn new(
        url: String,
        timeout_per_attempt: Duration,
        max_retries: u8,
        backoff: Duration,
        transport: Arc<dyn WebhookTransport>,
    ) -> Self {
        Self {
            url,
            timeout_per_attempt,
            max_retries,
            backoff,
            transport,
        }
    }

    /// Dispatches event to webhook endpoint with retry policy.
    pub async fn dispatch(&self, event: &Event) -> Result<(), EventPipelineError> {
        let payload = serde_json::to_string(event)?;

        for attempt in 0..=self.max_retries {
            let future = self.transport.post(&self.url, payload.clone());
            match timeout(self.timeout_per_attempt, future).await {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(_)) if attempt == self.max_retries => {
                    return Err(EventPipelineError::WebhookFailed);
                }
                Err(_) if attempt == self.max_retries => {
                    return Err(EventPipelineError::WebhookTimeout);
                }
                _ => sleep(self.backoff).await,
            }
        }

        Err(EventPipelineError::WebhookFailed)
    }
}
