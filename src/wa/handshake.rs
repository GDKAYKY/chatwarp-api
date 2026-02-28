use prost::Message;
use serde_json::Value;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::wa::{
    binary_node::{self, NodeContent},
    error::{HandshakeError, NoiseError},
    handshake_proto::HandshakeMessage,
    keys::{KeyPair, generate_keypair},
    noise::NoiseState,
    transport::WsTransport,
    types::WA_NOISE_PROLOGUE,
};

/// Handshake result used by higher-level connect flows.
#[derive(Debug, Clone)]
pub struct HandshakeOutcome {
    /// Initialized Noise state ready for encrypted session traffic.
    pub noise: NoiseState,
    /// Optional QR reference for first-time login.
    pub qr_reference: Option<String>,
    /// Raw server payload returned after client finish.
    pub server_payload: Vec<u8>,
    /// Optional JID when server confirms resumed login.
    pub login_jid: Option<String>,
    /// Client Noise public key used to build QR payload.
    pub noise_public: [u8; 32],
}

/// Performs a Noise XX handshake and returns initialized session data.
pub async fn do_handshake(
    transport: &mut WsTransport,
    static_keypair: &KeyPair,
) -> Result<HandshakeOutcome, HandshakeError> {
    let mut noise = NoiseState::new(WA_NOISE_PROLOGUE);

    let ephemeral = generate_keypair();
    noise.mix_hash(&ephemeral.public);

    let client_hello = HandshakeMessage {
        client_ephemeral: ephemeral.public.to_vec(),
        server_ephemeral: Vec::new(),
        encrypted_static: Vec::new(),
        payload: Vec::new(),
        qr_reference: None,
        login_jid: None,
    };

    let mut encoded_hello = Vec::new();
    client_hello.encode(&mut encoded_hello)?;
    transport.send_frame(&encoded_hello).await?;

    let server_hello_frame = transport.next_raw_frame().await?;
    let server_hello = decode_server_hello_frame(&server_hello_frame)?;

    let server_ephemeral = fixed_key(&server_hello.server_ephemeral, "server_ephemeral")?;
    noise.mix_hash(&server_ephemeral);

    let dh1 = diffie_hellman(ephemeral.private, server_ephemeral);
    noise.mix_into_key(&dh1);

    let ad1 = noise.handshake_hash();
    let decrypted_server_static = noise.decrypt_with_ad(&server_hello.encrypted_static, &ad1)?;
    let server_static = fixed_key(&decrypted_server_static, "server_static")?;

    let dh2 = diffie_hellman(ephemeral.private, server_static);
    noise.mix_into_key(&dh2);

    let ad2 = noise.handshake_hash();
    let encrypted_client_static = noise.encrypt_with_ad(&static_keypair.public, &ad2)?;

    let client_finish = HandshakeMessage {
        client_ephemeral: Vec::new(),
        server_ephemeral: Vec::new(),
        encrypted_static: encrypted_client_static,
        payload: Vec::new(),
        qr_reference: None,
        login_jid: None,
    };

    let mut encoded_finish = Vec::new();
    client_finish.encode(&mut encoded_finish)?;
    transport.send_frame(&encoded_finish).await?;

    // Handshake transitions to encrypted Noise transport after client finish.
    let server_finish_frame = transport.next_raw_frame().await?;
    let decrypted = decrypt_server_finish_frame(&mut noise, &server_finish_frame)?;
    let qr_reference =
        extract_qr_reference_from_node(&decrypted).or_else(|| parse_qr_reference(&decrypted));

    Ok(HandshakeOutcome {
        noise,
        qr_reference,
        server_payload: decrypted,
        login_jid: None,
        noise_public: ephemeral.public,
    })
}

fn fixed_key(bytes: &[u8], field: &'static str) -> Result<[u8; 32], HandshakeError> {
    if bytes.is_empty() {
        return Err(HandshakeError::MissingField(field));
    }

    if bytes.len() != 32 {
        return Err(HandshakeError::InvalidKeyLength(field));
    }

    let mut out = [0_u8; 32];
    out.copy_from_slice(bytes);
    Ok(out)
}

fn diffie_hellman(private: [u8; 32], peer_public: [u8; 32]) -> [u8; 32] {
    let private = StaticSecret::from(private);
    let public = PublicKey::from(peer_public);
    private.diffie_hellman(&public).to_bytes()
}

fn parse_qr_reference(payload: &[u8]) -> Option<String> {
    if payload.is_empty() {
        return None;
    }

    if let Ok(value) = serde_json::from_slice::<Value>(payload) {
        for key in ["ref", "reference"] {
            if let Some(reference) = value.get(key).and_then(Value::as_str) {
                let trimmed = reference.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_owned());
                }
            }
        }
    }

    std::str::from_utf8(payload)
        .ok()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn extract_qr_reference_from_node(payload: &[u8]) -> Option<String> {
    let node = binary_node::decode(payload).ok()?;

    if node.tag == "ref" {
        if let NodeContent::Bytes(bytes) = &node.content {
            return std::str::from_utf8(bytes)
                .ok()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
        }
    }

    node.attrs
        .get("ref")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn decode_server_hello_frame(frame: &[u8]) -> Result<HandshakeMessage, HandshakeError> {
    match HandshakeMessage::decode(frame) {
        Ok(message) => Ok(message),
        Err(raw_error) => {
            if let Some(payload) = maybe_unframe(frame) {
                return HandshakeMessage::decode(payload).map_err(HandshakeError::Decode);
            }
            if frame.len() > 3 {
                if let Ok(message) = HandshakeMessage::decode(&frame[3..]) {
                    return Ok(message);
                }
            }
            tracing::debug!(
                frame_len = frame.len(),
                frame_head = %preview_hex(frame, 24),
                "server_hello raw decode failed"
            );
            Err(HandshakeError::Decode(raw_error))
        }
    }
}

fn decrypt_server_finish_frame(
    noise: &mut NoiseState,
    frame: &[u8],
) -> Result<Vec<u8>, HandshakeError> {
    let ad = noise.handshake_hash();

    // Try raw first because WA may deliver handshake payload directly in websocket binary frames.
    let mut raw_noise = noise.clone();
    if let Ok(decrypted) = raw_noise.decrypt_with_ad(frame, &ad) {
        *noise = raw_noise;
        return Ok(decrypted);
    }

    // Fallback for deployments where the same payload still carries a 3-byte frame prefix.
    if let Some(payload) = maybe_unframe(frame) {
        let mut framed_noise = noise.clone();
        if let Ok(decrypted) = framed_noise.decrypt_with_ad(payload, &ad) {
            *noise = framed_noise;
            return Ok(decrypted);
        }
    }

    Err(HandshakeError::Noise(NoiseError::Cipher))
}

fn maybe_unframe(raw: &[u8]) -> Option<&[u8]> {
    if raw.len() < 3 {
        return None;
    }

    let expected_len = ((raw[0] as usize) << 16) | ((raw[1] as usize) << 8) | raw[2] as usize;
    let payload = &raw[3..];
    if payload.len() >= expected_len {
        return Some(&payload[..expected_len]);
    }

    None
}

fn preview_hex(bytes: &[u8], max_len: usize) -> String {
    let take = bytes.len().min(max_len);
    let mut out = String::with_capacity((take * 3).saturating_sub(1));
    for (index, byte) in bytes[..take].iter().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}
