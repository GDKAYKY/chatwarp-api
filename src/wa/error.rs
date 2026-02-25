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

/// Errors for binary node codec operations.
#[derive(Debug, Error)]
pub enum BinaryNodeError {
    #[error("unexpected end of payload")]
    UnexpectedEof,
    #[error("invalid symbol type: {0}")]
    InvalidSymbolType(u8),
    #[error("invalid content type: {0}")]
    InvalidContentType(u8),
    #[error("invalid utf-8 symbol")]
    InvalidUtf8,
    #[error("unknown token index: {0}")]
    UnknownToken(u8),
    #[error("symbol exceeds u16 max length")]
    SymbolTooLong,
    #[error("payload exceeds u32 max length")]
    PayloadTooLarge,
    #[error("too many attributes for a single node")]
    TooManyAttributes,
    #[error("too many nested child nodes")]
    TooManyChildren,
    #[error("trailing bytes after node decode")]
    TrailingBytes,
    #[error("attribute lookup failed during encode")]
    AttributeLookupFailed,
}

/// Errors for QR helpers.
#[derive(Debug, Error)]
pub enum QrError {
    #[error("qr channel is full")]
    ChannelFull,
    #[error("qr channel is closed")]
    ChannelClosed,
}

/// Errors for synthetic Signal session operations.
#[derive(Debug, Error)]
pub enum SignalError {
    #[error("signal store poisoned: {0}")]
    StorePoisoned(&'static str),
    #[error("missing signal session")]
    MissingSession,
    #[error("invalid ciphertext payload")]
    InvalidCiphertext,
}

/// Errors for outbound message API and node construction.
#[derive(Debug, Error)]
pub enum MessageError {
    #[error("invalid operation: {0}")]
    InvalidOperation(String),
    #[error("invalid content for operation: {operation}")]
    InvalidContentForOperation { operation: String },
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error(transparent)]
    BinaryNode(#[from] BinaryNodeError),
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
