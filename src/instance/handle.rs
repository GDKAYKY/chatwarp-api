use std::sync::Arc;

use serde::Serialize;
use tokio::sync::{RwLock, broadcast, mpsc};

use crate::{
    instance::error::InstanceError,
    wa::events::Event,
};

/// Current connection lifecycle state of an instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ConnectionState {
    /// Instance is attempting to connect.
    Connecting,
    /// Instance is waiting for QR scan.
    QrPending,
    /// Instance is connected.
    Connected,
    /// Instance is disconnected.
    Disconnected,
}

impl ConnectionState {
    /// Stable string representation of a connection state.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Connecting => "Connecting",
            Self::QrPending => "QrPending",
            Self::Connected => "Connected",
            Self::Disconnected => "Disconnected",
        }
    }
}

/// Commands accepted by an instance runner task.
#[derive(Debug)]
pub enum InstanceCommand {
    /// Starts connection flow.
    Connect,
    /// Marks instance as disconnected.
    Disconnect,
    /// Marks instance as connected.
    MarkConnected,
    /// Queues outbound payload.
    SendMessage(Vec<u8>),
    /// Gracefully shuts down the runner.
    Shutdown,
}

/// Handle used by other modules to interact with an instance task.
#[derive(Clone)]
pub struct InstanceHandle {
    /// Command sender for this instance.
    pub tx: mpsc::Sender<InstanceCommand>,
    /// Shared current connection state.
    pub state: Arc<RwLock<ConnectionState>>,
    event_tx: broadcast::Sender<Event>,
}

impl InstanceHandle {
    /// Creates a new instance handle.
    pub fn new(
        tx: mpsc::Sender<InstanceCommand>,
        state: Arc<RwLock<ConnectionState>>,
        event_tx: broadcast::Sender<Event>,
    ) -> Self {
        Self {
            tx,
            state,
            event_tx,
        }
    }

    /// Subscribes to instance events.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.event_tx.subscribe()
    }

    /// Sends a connect command to the instance runner.
    pub async fn connect(&self) -> Result<(), InstanceError> {
        self.tx
            .send(InstanceCommand::Connect)
            .await
            .map_err(|_| InstanceError::CommandChannelClosed)
    }

    /// Returns the current connection state.
    pub async fn connection_state(&self) -> ConnectionState {
        self.state.read().await.clone()
    }
}
