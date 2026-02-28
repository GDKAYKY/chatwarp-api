use prost::Message;
use serde_json::Value;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::wa::{
    error::HandshakeError,
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

    let server_hello_frame = transport.next_frame().await?;
    let server_hello = HandshakeMessage::decode(server_hello_frame)?;

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

    let server_payload_frame = transport.next_frame().await?;
    let server_payload = HandshakeMessage::decode(server_payload_frame)?;
    let qr_reference = server_payload
        .qr_reference
        .clone()
        .or_else(|| parse_qr_reference(&server_payload.payload));

    Ok(HandshakeOutcome {
        noise,
        qr_reference,
        server_payload: server_payload.payload,
        login_jid: server_payload.login_jid,
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
