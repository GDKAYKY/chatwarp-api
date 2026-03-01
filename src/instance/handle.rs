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

    /// API-friendly snake_case representation.
    pub fn as_api_str(&self) -> &'static str {
        match self {
            Self::Connecting => "connecting",
            Self::QrPending => "qr_pending",
            Self::Connected => "connected",
            Self::Disconnected => "disconnected",
        }
    }
}

/// Persisted runtime status for an instance connection lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstanceStatus {
    /// Current lifecycle state.
    pub state: ConnectionState,
    /// Last observed QR metadata while pairing.
    pub qrcode: QrCodeStatus,
    /// Last disconnect/error reason observed by the runner.
    pub last_error: Option<String>,
}

/// Current QR metadata snapshot for an instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct QrCodeStatus {
    /// Number of generated QR codes since latest connect attempt.
    pub count: u32,
    /// Raw QR code payload used by WhatsApp pairing.
    pub code: Option<String>,
    /// Rendered data URL representation of the QR code.
    pub base64: Option<String>,
    /// Pairing code when phone-number pairing is used.
    pub pairing_code: Option<String>,
}

impl Default for InstanceStatus {
    fn default() -> Self {
        Self {
            state: ConnectionState::Disconnected,
            qrcode: QrCodeStatus::default(),
            last_error: None,
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
    SendMessage {
        /// Message identifier.
        message_id: String,
        /// Encoded outbound payload bytes.
        payload: Vec<u8>,
    },
    /// Gracefully shuts down the runner.
    Shutdown,
}

/// Handle used by other modules to interact with an instance task.
#[derive(Clone)]
pub struct InstanceHandle {
    /// Command sender for this instance.
    pub tx: mpsc::Sender<InstanceCommand>,
    /// Shared current instance status.
    pub status: Arc<RwLock<InstanceStatus>>,
    event_tx: broadcast::Sender<Event>,
}

impl InstanceHandle {
    /// Creates a new instance handle.
    pub fn new(
        tx: mpsc::Sender<InstanceCommand>,
        status: Arc<RwLock<InstanceStatus>>,
        event_tx: broadcast::Sender<Event>,
    ) -> Self {
        Self {
            tx,
            status,
            event_tx,
        }
    }

    /// Subscribes to instance events.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.event_tx.subscribe()
    }

    /// Sends a connect command to the instance runner.
    pub async fn connect(&self) -> Result<(), InstanceError> {
        {
            let mut guard = self.status.write().await;
            if guard.state != ConnectionState::Connected {
                guard.state = ConnectionState::Connecting;
                guard.qrcode = QrCodeStatus::default();
                guard.last_error = None;
            }
        }

        self.tx
            .send(InstanceCommand::Connect)
            .await
            .map_err(|_| InstanceError::CommandChannelClosed)
    }

    /// Returns the current connection state.
    pub async fn connection_state(&self) -> ConnectionState {
        self.status.read().await.state.clone()
    }

    /// Returns the current runtime status snapshot.
    pub async fn status(&self) -> InstanceStatus {
        self.status.read().await.clone()
    }
}
