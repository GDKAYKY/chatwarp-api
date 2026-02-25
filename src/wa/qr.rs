use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use tokio::sync::mpsc;

use crate::wa::{
    error::QrError,
    events::Event,
};

/// Builds a WA QR payload string in the expected comma-separated format.
pub fn generate_qr_string(
    reference: &str,
    noise_pub: &[u8],
    identity_pub: &[u8],
    adv_key: &[u8],
) -> String {
    format!(
        "{reference},{},{},{}",
        STANDARD.encode(noise_pub),
        STANDARD.encode(identity_pub),
        STANDARD.encode(adv_key)
    )
}

/// Emits a QR event without blocking the caller loop.
pub fn emit_qr_code(tx: &mpsc::Sender<Event>, qr_payload: String) -> Result<(), QrError> {
    tx.try_send(Event::QrCode(qr_payload))
        .map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => QrError::ChannelFull,
            mpsc::error::TrySendError::Closed(_) => QrError::ChannelClosed,
        })
}
