#![allow(dead_code)]

use std::collections::HashMap;

use futures::{SinkExt, StreamExt};
use prost::Message;
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::Message as WsMessage,
};
use x25519_dalek::{PublicKey, StaticSecret};

use chatwarp_api::wa::{
    binary_node::{BinaryNode, NodeContent, encode},
    handshake_proto::HandshakeMessage,
    keys::generate_keypair,
    noise::NoiseState,
    types::WA_NOISE_PROLOGUE,
};

use super::ws_mock::{
    WsTestServer,
    start_single_client_server,
};

pub async fn start_mock_wa_server(
    qr_reference: Option<&str>,
    login_jid: Option<&str>,
    send_success: bool,
) -> anyhow::Result<WsTestServer> {
    let qr_reference = qr_reference.map(ToOwned::to_owned);
    let login_jid = login_jid.map(ToOwned::to_owned);

    start_single_client_server(move |mut ws| async move {
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
        let _decrypted_client_static = noise.decrypt_with_ad(&client_finish.encrypted_static, &ad2)?;

        let server_payload = HandshakeMessage {
            client_ephemeral: Vec::new(),
            server_ephemeral: Vec::new(),
            encrypted_static: Vec::new(),
            payload: Vec::new(),
            qr_reference,
            login_jid: login_jid.clone(),
        };
        let mut encoded_server_payload = Vec::new();
        server_payload.encode(&mut encoded_server_payload)?;
        ws.send(WsMessage::Binary(frame_payload(&encoded_server_payload).into()))
            .await?;

        if send_success {
            let mut attrs = HashMap::new();
            attrs.insert(
                "jid".to_owned(),
                login_jid.unwrap_or_else(|| "5511999999999@s.whatsapp.net".to_owned()),
            );
            let node = BinaryNode {
                tag: "success".to_owned(),
                attrs,
                content: NodeContent::Empty,
            };
            let encoded_node = encode(&node)?;
            let ad3 = noise.handshake_hash();
            let encrypted_success = noise.encrypt_with_ad(&encoded_node, &ad3)?;
            ws.send(WsMessage::Binary(frame_payload(&encrypted_success).into()))
                .await?;
        }

        while let Some(next) = ws.next().await {
            let message = match next {
                Ok(message) => message,
                Err(_) => break,
            };

            match message {
                WsMessage::Close(_) => break,
                WsMessage::Ping(payload) => {
                    ws.send(WsMessage::Pong(payload)).await?;
                }
                _ => {}
            }
        }

        Ok(())
    })
    .await
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
