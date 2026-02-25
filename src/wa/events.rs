/// Events emitted by WA runtime components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// QR code payload generated for login.
    QrCode(String),
}
