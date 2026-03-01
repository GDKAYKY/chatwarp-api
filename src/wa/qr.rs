use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use qrcode::render::unicode;
use qrcode::render::svg;
use qrcode::QrCode;
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
    adv_secret_key_b64: &str,
) -> String {
    format!(
        "{reference},{},{},{}",
        STANDARD.encode(noise_pub),
        STANDARD.encode(identity_pub),
        adv_secret_key_b64
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

/// Renders a QR payload into a terminal-friendly Unicode matrix.
pub fn render_qr_for_terminal(qr_payload: &str) -> Result<String, String> {
    let code = QrCode::new(qr_payload.as_bytes()).map_err(|error| error.to_string())?;
    Ok(code.render::<unicode::Dense1x2>().build())
}

/// Encodes a QR payload as SVG data URL.
pub fn render_qr_svg_data_url(qr_payload: &str) -> Result<String, String> {
    let code = QrCode::new(qr_payload.as_bytes()).map_err(|error| error.to_string())?;
    let svg_qr = code
        .render::<svg::Color<'_>>()
        .min_dimensions(240, 240)
        .build();
    let encoded = STANDARD.encode(svg_qr.as_bytes());
    Ok(format!("data:image/svg+xml;base64,{encoded}"))
}
