mod common;

use futures::{SinkExt, StreamExt};
use prost::Message;
use tokio_tungstenite::{WebSocketStream, tungstenite::Message as WsMessage};
use x25519_dalek::{PublicKey, StaticSecret};

use chatwarp_api::wa::{
    handshake::do_handshake,
    handshake_proto::HandshakeMessage,
    keys::generate_keypair,
    noise::NoiseState,
    transport::WsTransport,
    types::WA_NOISE_PROLOGUE,
};
use common::ws_mock::start_single_client_server;

#[tokio::test]
async fn handshake_converges_to_shared_noise_state() -> anyhow::Result<()> {
    let client_static = generate_keypair();
    let expected_client_public = client_static.public;

    let server = start_single_client_server(move |mut ws| async move {
        let server_static = generate_keypair();
        let server_ephemeral = generate_keypair();
        let mut noise = NoiseState::new(WA_NOISE_PROLOGUE);

        let client_hello_raw = read_binary(&mut ws).await?;
        let client_hello = HandshakeMessage::decode(unframe(&client_hello_raw)?.as_slice())?;

        let client_ephemeral = fixed_key(&client_hello.client_ephemeral, "client_ephemeral")?;
        noise.mix_hash(&client_ephemeral);
        noise.mix_hash(&server_ephemeral.public);

        let dh1 = diffie_hellman(server_ephemeral.private, client_ephemeral);
        noise.mix_into_key(&dh1);

        let ad1 = noise.handshake_hash();
        let encrypted_server_static = noise.encrypt_with_ad(&server_static.public, &ad1)?;

        let server_hello = HandshakeMessage {
            client_ephemeral: Vec::new(),
            server_ephemeral: server_ephemeral.public.to_vec(),
            encrypted_static: encrypted_server_static,
            payload: Vec::new(),
            qr_reference: None,
            login_jid: None,
        };

        let mut encoded_server_hello = Vec::new();
        server_hello.encode(&mut encoded_server_hello)?;
        ws.send(WsMessage::Binary(encoded_server_hello.into()))
            .await?;

        let dh2 = diffie_hellman(server_static.private, client_ephemeral);
        noise.mix_into_key(&dh2);

        let client_finish_raw = read_binary(&mut ws).await?;
        let client_finish = HandshakeMessage::decode(unframe(&client_finish_raw)?.as_slice())?;

        let ad2 = noise.handshake_hash();
        let decrypted_client_static = noise.decrypt_with_ad(&client_finish.encrypted_static, &ad2)?;
        assert_eq!(decrypted_client_static, expected_client_public);

        let ad3 = noise.handshake_hash();
        let encrypted_server_finish = noise.encrypt_with_ad(b"2@test-reference", &ad3)?;
        ws.send(WsMessage::Binary(encrypted_server_finish.into()))
            .await?;

        let ad4 = noise.handshake_hash();
        let encrypted_payload = noise.encrypt_with_ad(b"server-proof", &ad4)?;
        ws.send(WsMessage::Binary(frame_payload(&encrypted_payload).into()))
            .await?;

        Ok(())
    })
    .await?;

    let mut transport = WsTransport::connect(&server.url).await?;
    let outcome = do_handshake(&mut transport, &client_static).await?;
    assert_eq!(outcome.qr_reference.as_deref(), Some("2@test-reference"));
    let mut client_noise = outcome.noise;

    let encrypted_server_payload = transport.next_frame().await?;
    let ad = client_noise.handshake_hash();
    let decrypted = client_noise.decrypt_with_ad(&encrypted_server_payload, &ad)?;

    assert_eq!(decrypted, b"server-proof");

    server.finish().await?;
    Ok(())
}

#[tokio::test]
async fn handshake_accepts_framed_server_handshake_messages() -> anyhow::Result<()> {
    let client_static = generate_keypair();
    let expected_client_public = client_static.public;

    let server = start_single_client_server(move |mut ws| async move {
        let server_static = generate_keypair();
        let server_ephemeral = generate_keypair();
        let mut noise = NoiseState::new(WA_NOISE_PROLOGUE);

        let client_hello_raw = read_binary(&mut ws).await?;
        let client_hello = HandshakeMessage::decode(unframe(&client_hello_raw)?.as_slice())?;

        let client_ephemeral = fixed_key(&client_hello.client_ephemeral, "client_ephemeral")?;
        noise.mix_hash(&client_ephemeral);
        noise.mix_hash(&server_ephemeral.public);

        let dh1 = diffie_hellman(server_ephemeral.private, client_ephemeral);
        noise.mix_into_key(&dh1);

        let ad1 = noise.handshake_hash();
        let encrypted_server_static = noise.encrypt_with_ad(&server_static.public, &ad1)?;

        let server_hello = HandshakeMessage {
            client_ephemeral: Vec::new(),
            server_ephemeral: server_ephemeral.public.to_vec(),
            encrypted_static: encrypted_server_static,
            payload: Vec::new(),
            qr_reference: None,
            login_jid: None,
        };

        let mut encoded_server_hello = Vec::new();
        server_hello.encode(&mut encoded_server_hello)?;
        ws.send(WsMessage::Binary(frame_payload(&encoded_server_hello).into()))
            .await?;

        let dh2 = diffie_hellman(server_static.private, client_ephemeral);
        noise.mix_into_key(&dh2);

        let client_finish_raw = read_binary(&mut ws).await?;
        let client_finish = HandshakeMessage::decode(unframe(&client_finish_raw)?.as_slice())?;

        let ad2 = noise.handshake_hash();
        let decrypted_client_static = noise.decrypt_with_ad(&client_finish.encrypted_static, &ad2)?;
        assert_eq!(decrypted_client_static, expected_client_public);

        let ad3 = noise.handshake_hash();
        let encrypted_server_finish = noise.encrypt_with_ad(b"2@test-reference-framed", &ad3)?;
        ws.send(WsMessage::Binary(frame_payload(&encrypted_server_finish).into()))
            .await?;

        let ad4 = noise.handshake_hash();
        let encrypted_payload = noise.encrypt_with_ad(b"server-proof-framed", &ad4)?;
        ws.send(WsMessage::Binary(frame_payload(&encrypted_payload).into()))
            .await?;

        Ok(())
    })
    .await?;

    let mut transport = WsTransport::connect(&server.url).await?;
    let outcome = do_handshake(&mut transport, &client_static).await?;
    assert_eq!(outcome.qr_reference.as_deref(), Some("2@test-reference-framed"));
    let mut client_noise = outcome.noise;

    let encrypted_server_payload = transport.next_frame().await?;
    let ad = client_noise.handshake_hash();
    let decrypted = client_noise.decrypt_with_ad(&encrypted_server_payload, &ad)?;

    assert_eq!(decrypted, b"server-proof-framed");

    server.finish().await?;
    Ok(())
}

fn fixed_key(input: &[u8], label: &'static str) -> anyhow::Result<[u8; 32]> {
    if input.len() != 32 {
        anyhow::bail!("{label} must be 32 bytes, got {}", input.len());
    }

    let mut out = [0_u8; 32];
    out.copy_from_slice(input);
    Ok(out)
}

fn diffie_hellman(private: [u8; 32], peer_public: [u8; 32]) -> [u8; 32] {
    let private = StaticSecret::from(private);
    let public = PublicKey::from(peer_public);
    private.diffie_hellman(&public).to_bytes()
}

fn frame_payload(payload: &[u8]) -> Vec<u8> {
    let len = payload.len();
    let mut framed = Vec::with_capacity(3 + len);
    framed.push(((len >> 16) & 0xFF) as u8);
    framed.push(((len >> 8) & 0xFF) as u8);
    framed.push((len & 0xFF) as u8);
    framed.extend_from_slice(payload);
    framed
}

fn unframe(raw: &[u8]) -> anyhow::Result<Vec<u8>> {
    if raw.len() < 3 {
        anyhow::bail!("frame too short");
    }

    let expected_len = ((raw[0] as usize) << 16) | ((raw[1] as usize) << 8) | raw[2] as usize;
    let payload = &raw[3..];

    if payload.len() != expected_len {
        anyhow::bail!(
            "invalid frame length, expected {expected_len} bytes and got {}",
            payload.len()
        );
    }

    Ok(payload.to_vec())
}

async fn read_binary<S>(ws: &mut WebSocketStream<S>) -> anyhow::Result<Vec<u8>>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let next = ws
        .next()
        .await
        .ok_or_else(|| anyhow::anyhow!("websocket closed unexpectedly"))??;

    match next {
        WsMessage::Binary(binary) => Ok(binary.to_vec()),
        other => anyhow::bail!("expected binary message, got {other:?}"),
    }
}
