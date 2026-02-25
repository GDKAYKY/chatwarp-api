use tokio::sync::broadcast;

use crate::{
    events::error::EventPipelineError,
    wa::events::Event,
};

/// Synthetic websocket event broadcaster per instance.
#[derive(Clone)]
pub struct WebSocketHub {
    tx: broadcast::Sender<String>,
}

impl WebSocketHub {
    /// Creates a new websocket hub.
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Subscribes to websocket event stream.
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    /// Broadcasts a serialized event to all subscribers.
    pub fn broadcast_event(&self, event: &Event) -> Result<(), EventPipelineError> {
        let payload = serde_json::to_string(event)?;
        let _ = self.tx.send(payload);
        Ok(())
    }
}
