use prost::Message;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::wa::{
    error::HandshakeError,
    handshake_proto::HandshakeMessage,
    keys::{KeyPair, generate_keypair},
    noise::NoiseState,
    transport::WsTransport,
    types::WA_NOISE_PROLOGUE,
};

/// Performs a minimal Noise XX handshake and returns an initialized state.
pub async fn do_handshake(
    transport: &mut WsTransport,
    static_keypair: &KeyPair,
) -> Result<NoiseState, HandshakeError> {
    let mut noise = NoiseState::new(WA_NOISE_PROLOGUE);

    let ephemeral = generate_keypair();
    noise.mix_hash(&ephemeral.public);

    let client_hello = HandshakeMessage {
        client_ephemeral: ephemeral.public.to_vec(),
        server_ephemeral: Vec::new(),
        encrypted_static: Vec::new(),
        payload: Vec::new(),
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
    };

    let mut encoded_finish = Vec::new();
    client_finish.encode(&mut encoded_finish)?;
    transport.send_frame(&encoded_finish).await?;

    Ok(noise)
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
