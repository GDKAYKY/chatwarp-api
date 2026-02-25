use thiserror::Error;

/// Errors for websocket transport operations.
#[derive(Debug, Error)]
pub enum TransportError {
    #[error("websocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("failed to connect transport: {0}")]
    Connect(#[source] tokio_tungstenite::tungstenite::Error),
    #[error("invalid websocket request: {0}")]
    InvalidRequest(#[from] http::Error),
    #[error("invalid framed payload: {0}")]
    InvalidFrame(&'static str),
    #[error("payload exceeds max 24-bit frame size")]
    FrameTooLarge,
    #[error("transport closed by peer")]
    Closed,
}

/// Errors for Noise state operations.
#[derive(Debug, Error)]
pub enum NoiseError {
    #[error("cipher error")]
    Cipher,
    #[error("invalid key material")]
    InvalidKeyMaterial,
}

/// Errors for handshake operations.
#[derive(Debug, Error)]
pub enum HandshakeError {
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error(transparent)]
    Noise(#[from] NoiseError),
    #[error("handshake proto decode failed: {0}")]
    Decode(#[from] prost::DecodeError),
    #[error("handshake payload encode failed: {0}")]
    Encode(#[from] prost::EncodeError),
    #[error("missing handshake field: {0}")]
    MissingField(&'static str),
    #[error("invalid handshake key length for {0}")]
    InvalidKeyLength(&'static str),
}
