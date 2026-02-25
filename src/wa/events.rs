/// Events emitted by WA runtime components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// QR code payload generated for login.
    QrCode(String),
    /// Instance has entered connected state.
    Connected { instance_name: String },
    /// Instance has entered disconnected state.
    Disconnected { instance_name: String, reason: String },
    /// Outbound payload acknowledged by runner.
    OutboundAck { instance_name: String, bytes: usize },
    /// Reconnect backoff has been scheduled.
    ReconnectScheduled { instance_name: String, delay_secs: u64 },
}
