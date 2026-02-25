use serde::{Deserialize, Serialize};

/// Events emitted by WA runtime components.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    /// QR code payload generated for login.
    QrCode(String),
    /// Instance has entered connected state.
    Connected { instance_name: String },
    /// Instance has entered disconnected state.
    Disconnected { instance_name: String, reason: String },
    /// Outbound payload acknowledged by runner.
    OutboundAck { instance_name: String, message_id: String, bytes: usize },
    /// Reconnect backoff has been scheduled.
    ReconnectScheduled { instance_name: String, delay_secs: u64 },
}

impl Event {
    /// Returns a stable event-type label.
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::QrCode(_) => "qr_code",
            Self::Connected { .. } => "connected",
            Self::Disconnected { .. } => "disconnected",
            Self::OutboundAck { .. } => "outbound_ack",
            Self::ReconnectScheduled { .. } => "reconnect_scheduled",
        }
    }
}
