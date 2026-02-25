use tokio::sync::mpsc;

use chatwarp_api::wa::{
    events::Event,
    qr::{emit_qr_code, generate_qr_string},
};

#[test]
fn generate_qr_string_uses_expected_format() {
    let value = generate_qr_string("ref-1", &[1, 2], &[3, 4], &[5, 6]);
    assert_eq!(value, "ref-1,AQI=,AwQ=,BQY=");
}

#[test]
fn emit_qr_code_non_blocking_sends_event() -> anyhow::Result<()> {
    let (tx, mut rx) = mpsc::channel(1);

    emit_qr_code(&tx, "qr-payload".to_string())?;

    let event = rx
        .try_recv()
        .map_err(|error| anyhow::anyhow!("missing qr event: {error}"))?;

    assert_eq!(event, Event::QrCode("qr-payload".to_string()));
    Ok(())
}
