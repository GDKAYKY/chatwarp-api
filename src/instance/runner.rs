use std::{
    collections::HashMap,
    sync::Arc,
    time::Duration,
};

use base64::Engine;
use bytes::Bytes;
use prost::Message;
use sha2::{Digest, Sha256};
use tokio::{
    sync::{RwLock, broadcast, mpsc},
    time::Instant,
};
use url::Url;

use crate::{
    config::WaProtocolMode,
    db::auth_store::AuthStore,
    instance::handle::{ConnectionState, InstanceCommand, InstanceStatus, QrCodeStatus},
    wa::{
        auth::{AuthState, MeInfo},
        binary_node::{self, BinaryNode, NodeContent},
        error::{HandshakeError, HandshakePhase, NoiseError, TransportError},
        events::Event,
        handshake::{MdHandshakeOutcome, do_handshake, do_handshake_md},
        keys::{sign_message, verify_message},
        noise::NoiseState,
        noise_md::NoiseMdState,
        proto_md::wa as wa_proto,
        qr::{generate_qr_string, render_qr_for_terminal, render_qr_svg_data_url},
        transport::{WsConnectOptions, WsTransport},
        version::{WaVersionManager, WaWebVersion},
    },
};

#[derive(Default)]
struct RunnerSession {
    transport: Option<WsTransport>,
    noise: Option<RunnerNoise>,
    auth: Option<AuthState>,
    noise_public: Option<[u8; 32]>,
    awaiting_login: bool,
    login_deadline: Option<Instant>,
    reconnect_attempt: u32,
    auto_reconnect: bool,
}

#[derive(Debug, Clone)]
enum RunnerNoise {
    Synthetic(NoiseState),
    RealMd(NoiseMdState),
}

#[derive(Debug, Clone)]
enum RunnerHandshake {
    Synthetic(crate::wa::handshake::HandshakeOutcome),
    RealMd(MdHandshakeOutcome),
}

impl RunnerHandshake {
    fn first_qr_reference(&self) -> Option<String> {
        match self {
            Self::Synthetic(outcome) => outcome.qr_reference.clone(),
            Self::RealMd(outcome) => outcome.qr_references.first().cloned(),
        }
    }

    fn first_payload_qr_reference(&self) -> Option<String> {
        self.first_payload()
            .and_then(|payload| extract_qr_reference_from_payload(payload, self.mode()))
    }

    fn login_jid(&self) -> Option<String> {
        match self {
            Self::Synthetic(outcome) => outcome.login_jid.clone(),
            Self::RealMd(outcome) => outcome.login_jid.clone(),
        }
    }

    fn first_payload(&self) -> Option<&[u8]> {
        match self {
            Self::Synthetic(outcome) => Some(outcome.server_payload.as_slice()),
            Self::RealMd(outcome) => outcome.server_payloads.first().map(Vec::as_slice),
        }
    }

    fn mode(&self) -> WaProtocolMode {
        match self {
            Self::Synthetic(_) => WaProtocolMode::Synthetic,
            Self::RealMd(_) => WaProtocolMode::RealMd,
        }
    }
}

const LOGIN_TIMEOUT: Duration = Duration::from_secs(60);
const DEFAULT_QRCODE_LIMIT: u32 = 30;
const WA_ADV_ACCOUNT_SIG_PREFIX: [u8; 2] = [6, 0];
const WA_ADV_DEVICE_SIG_PREFIX: [u8; 2] = [6, 1];
const WA_ADV_HOSTED_ACCOUNT_SIG_PREFIX: [u8; 2] = [6, 5];

/// Main task loop for a single instance.
pub async fn run(
    name: String,
    status: Arc<RwLock<InstanceStatus>>,
    mut command_rx: mpsc::Receiver<InstanceCommand>,
    event_tx: broadcast::Sender<Event>,
    auth_store: Arc<dyn AuthStore>,
    wa_ws_url: String,
    wa_protocol_mode: WaProtocolMode,
    wa_version_manager: Arc<WaVersionManager>,
) {
    let mut session = RunnerSession::default();

    loop {
        if session.transport.is_some() {
            let mut transport = session.transport.take().expect("transport must exist");
            tokio::select! {
                maybe_command = command_rx.recv() => {
                    session.transport = Some(transport);
                    let Some(command) = maybe_command else {
                        break;
                    };

                    if !handle_command(
                        &name,
                        &status,
                        &event_tx,
                        &auth_store,
                        &wa_ws_url,
                        wa_protocol_mode,
                        &wa_version_manager,
                        &mut session,
                        command,
                    ).await {
                        break;
                    }
                }
                frame = transport.next_raw_frame() => {
                    session.transport = Some(transport);
                    match frame {
                        Ok(frame) => {
                            if let Err(error) = handle_incoming_frame(
                                &name,
                                &status,
                                &event_tx,
                                &auth_store,
                                &mut session,
                                frame.as_ref(),
                            ).await {
                                force_disconnected(&name, &status, &event_tx, &mut session, &error).await;
                                if session.auto_reconnect {
                                    establish_connection(
                                        &name,
                                        &status,
                                        &event_tx,
                                        &auth_store,
                                        &wa_ws_url,
                                        wa_protocol_mode,
                                        &wa_version_manager,
                                        &mut session,
                                        true,
                                    ).await;
                                }
                            }
                        }
                        Err(error) => {
                            let reason = format!("transport_error: {error}");
                            force_disconnected(&name, &status, &event_tx, &mut session, &reason).await;
                            if session.auto_reconnect {
                                establish_connection(
                                    &name,
                                    &status,
                                    &event_tx,
                                    &auth_store,
                                    &wa_ws_url,
                                    wa_protocol_mode,
                                    &wa_version_manager,
                                    &mut session,
                                    true,
                                ).await;
                            }
                        }
                    }
                }
                _ = async {
                    if let Some(deadline) = session.login_deadline {
                        tokio::time::sleep_until(deadline).await;
                    }
                }, if session.awaiting_login && session.login_deadline.is_some() => {
                    session.transport = Some(transport);
                    tracing::warn!(instance = name, timeout_secs = LOGIN_TIMEOUT.as_secs(), "login timeout waiting for pair-success jid");
                    force_disconnected(&name, &status, &event_tx, &mut session, "login_timeout").await;
                    if session.auto_reconnect {
                        establish_connection(
                            &name,
                            &status,
                            &event_tx,
                            &auth_store,
                            &wa_ws_url,
                            wa_protocol_mode,
                            &wa_version_manager,
                            &mut session,
                            true,
                        ).await;
                    }
                }
            }
            continue;
        }

        let Some(command) = command_rx.recv().await else {
            break;
        };

        if !handle_command(
            &name,
            &status,
            &event_tx,
            &auth_store,
            &wa_ws_url,
            wa_protocol_mode,
            &wa_version_manager,
            &mut session,
            command,
        )
        .await
        {
            break;
        }
    }
}

async fn handle_command(
    name: &str,
    status: &Arc<RwLock<InstanceStatus>>,
    event_tx: &broadcast::Sender<Event>,
    auth_store: &Arc<dyn AuthStore>,
    wa_ws_url: &str,
    wa_protocol_mode: WaProtocolMode,
    wa_version_manager: &Arc<WaVersionManager>,
    session: &mut RunnerSession,
    command: InstanceCommand,
) -> bool {
    match command {
        InstanceCommand::Connect => {
            session.auto_reconnect = true;
            if session.transport.is_none() {
                establish_connection(
                    name,
                    status,
                    event_tx,
                    auth_store,
                    wa_ws_url,
                    wa_protocol_mode,
                    wa_version_manager,
                    session,
                    false,
                )
                .await;
            }
            true
        }
        InstanceCommand::Disconnect => {
            session.auto_reconnect = false;
            force_disconnected(name, status, event_tx, session, "manual_disconnect").await;
            true
        }
        InstanceCommand::MarkConnected => {
            if let Err(error) = mark_connected(name, status, event_tx, auth_store, session).await {
                let reason = format!("mark_connected_failed: {error}");
                force_disconnected(name, status, event_tx, session, &reason).await;
            }
            true
        }
        InstanceCommand::SendMessage {
            message_id,
            payload,
        } => {
            if status.read().await.state == ConnectionState::Connected {
                let result = send_encrypted_payload(session, &payload).await;
                if let Err(error) = result {
                    let reason = format!("send_failed: {error}");
                    force_disconnected(name, status, event_tx, session, &reason).await;
                    if session.auto_reconnect {
                        establish_connection(
                            name,
                            status,
                            event_tx,
                            auth_store,
                            wa_ws_url,
                            wa_protocol_mode,
                            wa_version_manager,
                            session,
                            true,
                        )
                        .await;
                    }
                } else {
                    let _ = event_tx.send(Event::OutboundAck {
                        instance_name: name.to_owned(),
                        message_id,
                        bytes: payload.len(),
                    });
                }
            }
            true
        }
        InstanceCommand::Shutdown => false,
    }
}

async fn establish_connection(
    name: &str,
    status: &Arc<RwLock<InstanceStatus>>,
    event_tx: &broadcast::Sender<Event>,
    auth_store: &Arc<dyn AuthStore>,
    wa_ws_url: &str,
    wa_protocol_mode: WaProtocolMode,
    wa_version_manager: &Arc<WaVersionManager>,
    session: &mut RunnerSession,
    sleep_before_first_attempt: bool,
) {
    let mut should_sleep = sleep_before_first_attempt;
    loop {
        if !session.auto_reconnect {
            return;
        }

        let delay_secs = backoff_seconds(session.reconnect_attempt);
        let _ = event_tx.send(Event::ReconnectScheduled {
            instance_name: name.to_owned(),
            delay_secs,
        });
        if should_sleep {
            tokio::time::sleep(Duration::from_secs(delay_secs)).await;
        }

        match connect_once(
            name,
            status,
            event_tx,
            auth_store,
            wa_ws_url,
            wa_protocol_mode,
            wa_version_manager,
            session,
        )
        .await
        {
            Ok(()) => {
                session.reconnect_attempt = 0;
                return;
            }
            Err(error) => {
                tracing::warn!(instance = %name, error = %error, "connect_once failed");
                force_disconnected(name, status, event_tx, session, &error).await;
                session.reconnect_attempt = session.reconnect_attempt.saturating_add(1);
                should_sleep = true;
            }
        }
    }
}

async fn connect_once(
    name: &str,
    status: &Arc<RwLock<InstanceStatus>>,
    event_tx: &broadcast::Sender<Event>,
    auth_store: &Arc<dyn AuthStore>,
    wa_ws_url: &str,
    wa_protocol_mode: WaProtocolMode,
    wa_version_manager: &Arc<WaVersionManager>,
    session: &mut RunnerSession,
) -> Result<(), String> {
    {
        let mut guard = status.write().await;
        guard.state = ConnectionState::Connecting;
        guard.qrcode = QrCodeStatus::default();
        guard.last_error = None;
    }

    let loaded_auth = auth_store
        .load(name)
        .await
        .map_err(|error| format!("auth_load_failed: {error}"))?;
    let mut auth = loaded_auth.unwrap_or_else(AuthState::new);

    let mut retries = 0_u8;
    let (transport, handshake) = loop {
        let version = wa_version_manager.get_version().await;
        let ws_url = build_ws_url(wa_ws_url, &auth, wa_protocol_mode)?;
        let options = ws_connect_options(wa_protocol_mode, version);
        let mut transport = connect_transport_with_fallback(&ws_url, options, wa_protocol_mode)
            .await
            .map_err(|error| format!("ws_connect_failed: {error}"))?;

        let upgrade = transport.upgrade_metadata();
        tracing::info!(
            instance = name,
            wa_mode = ?wa_protocol_mode,
            wa_version = format!("{}.{}.{}", version.major, version.minor, version.patch),
            upgrade_status = upgrade.status,
            "websocket upgrade completed"
        );

        let handshake_result = if wa_protocol_mode == WaProtocolMode::Synthetic {
            do_handshake(&mut transport, &auth.identity.identity_key)
                .await
                .map(RunnerHandshake::Synthetic)
        } else {
            do_handshake_md(&mut transport, &auth, version)
                .await
                .map(RunnerHandshake::RealMd)
        };

        match handshake_result {
            Ok(outcome) => break (transport, outcome),
            Err(error) if should_retry_with_refetched_version(wa_protocol_mode, retries, &error) => {
                retries = retries.saturating_add(1);
                let phase = extract_handshake_phase(&error);
                let close_code = extract_close_code(&error);
                tracing::warn!(
                    instance = name,
                    wa_version = format!("{}.{}.{}", version.major, version.minor, version.patch),
                    ?phase,
                    ?close_code,
                    "close 1011 during bootstrap, invalidating wa version cache and retrying once"
                );
                wa_version_manager.invalidate().await;
                continue;
            }
            Err(error) => return Err(format!("handshake_failed: {error}")),
        }
    };

    session.transport = Some(transport);
    session.noise = Some(match &handshake {
        RunnerHandshake::Synthetic(outcome) => RunnerNoise::Synthetic(outcome.noise.clone()),
        RunnerHandshake::RealMd(outcome) => RunnerNoise::RealMd(outcome.noise.clone()),
    });
    session.noise_public = Some(match &handshake {
        RunnerHandshake::Synthetic(outcome) => outcome.noise_public,
        RunnerHandshake::RealMd(outcome) => outcome.noise_public,
    });
    session.auth = Some(auth.clone());
    session.awaiting_login = true;
    session.login_deadline = Some(Instant::now() + LOGIN_TIMEOUT);

    if let Some(reference) = handshake
        .first_qr_reference()
        .or_else(|| handshake.first_payload_qr_reference())
    {
        let qr = generate_qr_string(
            &reference,
            session.noise_public.as_ref().expect("noise public key must exist"),
            &auth.identity.identity_key.public,
            &auth.adv_secret_key,
        );
        if !update_qr_state(name, status, event_tx, &qr).await {
            return Err("qr_code_limit_reached".to_owned());
        }
        session.awaiting_login = true;
    }

    if let Some(jid) = handshake
        .login_jid()
        .or_else(|| {
            handshake
                .first_payload()
                .and_then(|payload| extract_login_jid_from_payload(payload, wa_protocol_mode))
        })
    {
        auth.metadata.me = Some(MeInfo {
            jid,
            push_name: None,
        });
        session.auth = Some(auth);
        session.awaiting_login = false;
        session.login_deadline = None;
        mark_connected(name, status, event_tx, auth_store, session)
            .await
            .map_err(|error| format!("save_auth_failed: {error}"))?;
    }

    Ok(())
}

fn build_ws_url(wa_ws_url: &str, auth: &AuthState, mode: WaProtocolMode) -> Result<String, String> {
    if mode == WaProtocolMode::Synthetic {
        return Ok(wa_ws_url.to_owned());
    }

    let mut parsed = Url::parse(wa_ws_url).map_err(|error| format!("invalid_wa_ws_url: {error}"))?;
    if let Some(routing_info) = auth.metadata.routing_info.as_ref() {
        if !routing_info.is_empty() {
            let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(routing_info);
            parsed.query_pairs_mut().append_pair("ED", &encoded);
        }
    }

    Ok(parsed.to_string())
}

fn ws_connect_options(mode: WaProtocolMode, _version: WaWebVersion) -> WsConnectOptions {
    if mode == WaProtocolMode::Synthetic {
        return WsConnectOptions::default();
    }

    WsConnectOptions {
        origin: Some("https://web.whatsapp.com".to_owned()),
        user_agent: Some(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
                .to_owned(),
        ),
        subprotocol: None,
        headers: Vec::new(),
    }
}

fn should_retry_with_refetched_version(mode: WaProtocolMode, retries: u8, error: &HandshakeError) -> bool {
    if mode == WaProtocolMode::Synthetic || retries >= 1 {
        return false;
    }

    let Some(close_code) = extract_close_code(error) else {
        return false;
    };
    if close_code != 1011 {
        return false;
    }

    match extract_handshake_phase(error) {
        Some(HandshakePhase::HttpUpgrade | HandshakePhase::ClientHello | HandshakePhase::ServerHello) => true,
        Some(HandshakePhase::ClientFinish | HandshakePhase::PostFinish) => false,
        None => false,
    }
}

fn extract_handshake_phase(error: &HandshakeError) -> Option<HandshakePhase> {
    match error {
        HandshakeError::Phase { phase, .. } => Some(*phase),
        _ => None,
    }
}

fn extract_close_code(error: &HandshakeError) -> Option<u16> {
    match error {
        HandshakeError::Transport(TransportError::ClosedWithCode(code)) => Some(*code),
        HandshakeError::Phase { message, .. } => extract_close_code_from_message(message),
        _ => None,
    }
}

fn extract_close_code_from_message(message: &str) -> Option<u16> {
    let marker = "code ";
    let index = message.find(marker)?;
    let suffix = &message[index + marker.len()..];
    let digits: String = suffix.chars().take_while(|char| char.is_ascii_digit()).collect();
    digits.parse::<u16>().ok()
}

async fn connect_transport_with_fallback(
    ws_url: &str,
    options: WsConnectOptions,
    mode: WaProtocolMode,
) -> Result<WsTransport, TransportError> {
    match WsTransport::connect_with_options(ws_url, options.clone()).await {
        Ok(transport) => Ok(transport),
        Err(error) if mode != WaProtocolMode::Synthetic && options.subprotocol.is_some() => {
            tracing::warn!(
                error = %error,
                "ws upgrade with subprotocol failed; retrying once without subprotocol"
            );
            let mut fallback = options;
            fallback.subprotocol = None;
            WsTransport::connect_with_options(ws_url, fallback).await
        }
        Err(error) => Err(error),
    }
}

async fn send_encrypted_payload(session: &mut RunnerSession, payload: &[u8]) -> Result<(), String> {
    let Some(noise) = session.noise.as_mut() else {
        return Err("missing_noise_state".to_owned());
    };
    let Some(transport) = session.transport.as_mut() else {
        return Err("missing_transport".to_owned());
    };

    match noise {
        RunnerNoise::Synthetic(noise) => {
            let ad = noise.handshake_hash();
            let encrypted = noise
                .encrypt_with_ad(payload, &ad)
                .map_err(|error| format!("noise_encrypt_failed: {error}"))?;
            transport
                .send_frame(&encrypted)
                .await
                .map_err(|error| format!("transport_send_failed: {error}"))
        }
        RunnerNoise::RealMd(noise) => {
            let frame = noise
                .encode_frame(payload)
                .map_err(|error| format!("noise_frame_encode_failed: {error}"))?;
            transport
                .send_raw(&frame)
                .await
                .map_err(|error| format!("transport_send_failed: {error}"))
        }
    }
}

async fn handle_incoming_frame(
    name: &str,
    status: &Arc<RwLock<InstanceStatus>>,
    event_tx: &broadcast::Sender<Event>,
    auth_store: &Arc<dyn AuthStore>,
    session: &mut RunnerSession,
    frame: &[u8],
) -> Result<(), String> {
    let Some(noise) = session.noise.as_mut() else {
        return Err("missing_noise_state".to_owned());
    };

    tracing::debug!(
        instance = name,
        frame_len = frame.len(),
        frame_head = %preview_hex(frame, 24),
        "received websocket binary frame"
    );

    let decrypted_frames = decrypt_incoming_payload(noise, frame)
        .map_err(|error| format!("noise_decrypt_failed: {error}"))?;
    let mode = match noise {
        RunnerNoise::Synthetic(_) => WaProtocolMode::Synthetic,
        RunnerNoise::RealMd(_) => WaProtocolMode::RealMd,
    };

    for decrypted in decrypted_frames {
        let decoded_node = match decode_node_for_mode(&decrypted, mode) {
            Ok(node) => Some(node),
            Err(error) => {
                tracing::debug!(instance = name, error = %error, "binary_node decode failed for incoming frame");
                None
            }
        };

        if let Some(node) = decoded_node.as_ref() {
            if mode != WaProtocolMode::Synthetic {
                if let Some(routing_info) = extract_edge_routing_from_node(node) {
                    if let Some(auth) = session.auth.as_mut() {
                        auth.metadata.routing_info = Some(routing_info);
                        auth_store
                            .save(name, auth)
                            .await
                            .map_err(|error| format!("save_auth_failed: {error}"))?;
                    }
                }
            }
        }

        if session.awaiting_login {
            let Some(node) = decoded_node.as_ref() else {
                continue;
            };

            if let Some(reference) = extract_qr_reference_from_node(node) {
                if let (Some(auth), Some(noise_public)) = (session.auth.as_ref(), session.noise_public.as_ref()) {
                    let qr = generate_qr_string(
                        &reference,
                        noise_public,
                        &auth.identity.identity_key.public,
                        &auth.adv_secret_key,
                    );
                    if !update_qr_state(name, status, event_tx, &qr).await {
                        return Err("qr_code_limit_reached".to_owned());
                    }
                }
            }

            if mode == WaProtocolMode::RealMd {
                let pair_reply = match session.auth.as_ref() {
                    Some(auth) => build_pair_device_sign_reply(node, auth)?,
                    None => None,
                };
                if let Some(reply) = pair_reply {
                    let encoded = binary_node::encode_real(&reply)
                        .map_err(|error| format!("pair_device_sign_encode_failed: {error}"))?;
                    send_encrypted_payload(session, &encoded).await?;
                }
            }

            if let Some(jid) = extract_login_jid(node) {
                if let Some(auth) = session.auth.as_mut() {
                    auth.metadata.me = Some(MeInfo {
                        jid,
                        push_name: None,
                    });
                }
                session.awaiting_login = false;
                session.login_deadline = None;
                mark_connected(name, status, event_tx, auth_store, session)
                    .await
                    .map_err(|error| format!("save_auth_failed: {error}"))?;
                continue;
            }

            if node.tag == "success" {
                let has_me = session
                    .auth
                    .as_ref()
                    .and_then(|auth| auth.metadata.me.as_ref())
                    .is_some();
                if has_me {
                    session.awaiting_login = false;
                    session.login_deadline = None;
                    mark_connected(name, status, event_tx, auth_store, session)
                        .await
                        .map_err(|error| format!("save_auth_failed: {error}"))?;
                    continue;
                }
            }
        }

        if is_failure_payload(&decrypted, mode) {
            return Err("server_reported_failure".to_owned());
        }
    }

    Ok(())
}

fn decrypt_incoming_payload(noise: &mut RunnerNoise, raw_frame: &[u8]) -> Result<Vec<Vec<u8>>, NoiseError> {
    match noise {
        RunnerNoise::Synthetic(noise) => {
            let ad = noise.handshake_hash();

            // Try raw first: some peers deliver encrypted payload directly in the websocket binary message.
            let mut raw_noise = noise.clone();
            if let Ok(decrypted) = raw_noise.decrypt_with_ad(raw_frame, &ad) {
                *noise = raw_noise;
                return Ok(vec![decrypted]);
            }

            // Fallback for peers that still prepend a WA 3-byte frame header.
            if let Some(payload) = maybe_unframe(raw_frame) {
                let mut framed_noise = noise.clone();
                if let Ok(decrypted) = framed_noise.decrypt_with_ad(payload, &ad) {
                    *noise = framed_noise;
                    return Ok(vec![decrypted]);
                }
            }

            Err(NoiseError::Cipher)
        }
        RunnerNoise::RealMd(noise) => noise
            .decode_frames(raw_frame)
            .map_err(|_| NoiseError::Cipher),
    }
}

async fn mark_connected(
    name: &str,
    status: &Arc<RwLock<InstanceStatus>>,
    event_tx: &broadcast::Sender<Event>,
    auth_store: &Arc<dyn AuthStore>,
    session: &RunnerSession,
) -> Result<(), String> {
    let Some(auth) = session.auth.as_ref() else {
        return Err("missing_auth_state".to_owned());
    };

    auth_store
        .save(name, auth)
        .await
        .map_err(|error| error.to_string())?;

    {
        let mut guard = status.write().await;
        guard.state = ConnectionState::Connected;
        guard.qrcode.code = None;
        guard.qrcode.base64 = None;
        guard.qrcode.pairing_code = None;
        guard.last_error = None;
    }
    let _ = event_tx.send(Event::Connected {
        instance_name: name.to_owned(),
    });

    Ok(())
}

async fn update_qr_state(
    name: &str,
    status: &Arc<RwLock<InstanceStatus>>,
    event_tx: &broadcast::Sender<Event>,
    qr_payload: &str,
) -> bool {
    let mut should_emit = false;
    {
        let mut guard = status.write().await;
        if guard.qrcode.count >= qrcode_limit_from_env() {
            guard.state = ConnectionState::Disconnected;
            guard.qrcode = QrCodeStatus::default();
            guard.last_error = Some("qr_code_limit_reached".to_owned());
        } else {
            guard.state = ConnectionState::QrPending;
            guard.qrcode.count = guard.qrcode.count.saturating_add(1);
            guard.qrcode.code = Some(qr_payload.to_owned());
            guard.qrcode.base64 = render_qr_svg_data_url(qr_payload).ok();
            guard.qrcode.pairing_code = None;
            guard.last_error = None;
            should_emit = true;
        }
    }

    if should_emit {
        print_qr_in_terminal(name, qr_payload);
        let _ = event_tx.send(Event::QrCode(qr_payload.to_owned()));
        return true;
    }

    let _ = event_tx.send(Event::Disconnected {
        instance_name: name.to_owned(),
        reason: "qr_code_limit_reached".to_owned(),
    });
    false
}

fn qrcode_limit_from_env() -> u32 {
    std::env::var("QRCODE_LIMIT")
        .ok()
        .and_then(|raw| raw.trim().parse::<u32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_QRCODE_LIMIT)
}

fn print_qr_in_terminal(instance_name: &str, qr_payload: &str) {
    match render_qr_for_terminal(qr_payload) {
        Ok(rendered) => {
            println!("\n[instance:{instance_name}] QR code:\n{rendered}\n");
        }
        Err(error) => {
            println!(
                "\n[instance:{instance_name}] QR payload (render failed: {error}):\n{qr_payload}\n"
            );
        }
    }
}

async fn force_disconnected(
    name: &str,
    status: &Arc<RwLock<InstanceStatus>>,
    event_tx: &broadcast::Sender<Event>,
    session: &mut RunnerSession,
    reason: &str,
) {
    session.transport = None;
    session.noise = None;
    session.noise_public = None;
    session.awaiting_login = false;
    session.login_deadline = None;
    {
        let mut guard = status.write().await;
        guard.state = ConnectionState::Disconnected;
        guard.qrcode = QrCodeStatus::default();
        guard.last_error = if reason.is_empty() {
            None
        } else {
            Some(reason.to_owned())
        };
    }
    let _ = event_tx.send(Event::Disconnected {
        instance_name: name.to_owned(),
        reason: reason.to_owned(),
    });
}

fn extract_login_jid(node: &binary_node::BinaryNode) -> Option<String> {
    if node.tag != "iq" {
        return None;
    }

    let binary_node::NodeContent::Nodes(children) = &node.content else {
        return None;
    };

    let pair = children.iter().find(|child| child.tag == "pair-success")?;
    if let Some(jid) = pair.attrs.get("jid").map(String::as_str).map(str::trim) {
        if !jid.is_empty() {
            return Some(jid.to_owned());
        }
    }

    let binary_node::NodeContent::Nodes(pair_children) = &pair.content else {
        return None;
    };
    let device = pair_children.iter().find(|child| child.tag == "device")?;
    let jid = device.attrs.get("jid")?.trim();
    if jid.is_empty() {
        return None;
    }
    Some(jid.to_owned())
}

fn extract_qr_reference_from_node(node: &binary_node::BinaryNode) -> Option<String> {
    if node.tag == "ref" {
        let binary_node::NodeContent::Bytes(bytes) = &node.content else {
            return None;
        };
        return std::str::from_utf8(bytes)
            .ok()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
    }

    if node.tag != "iq" {
        return None;
    }

    let binary_node::NodeContent::Nodes(children) = &node.content else {
        return None;
    };
    let pair_device = children.iter().find(|child| child.tag == "pair-device")?;
    let binary_node::NodeContent::Nodes(pair_children) = &pair_device.content else {
        return None;
    };
    let reference = pair_children.iter().find(|child| child.tag == "ref")?;
    let binary_node::NodeContent::Bytes(bytes) = &reference.content else {
        return None;
    };
    std::str::from_utf8(bytes)
        .ok()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn extract_edge_routing_from_node(node: &binary_node::BinaryNode) -> Option<Vec<u8>> {
    if node.tag != "ib" {
        return None;
    }

    let binary_node::NodeContent::Nodes(children) = &node.content else {
        return None;
    };

    let edge = children.iter().find(|child| child.tag == "edge_routing")?;
    let binary_node::NodeContent::Bytes(bytes) = &edge.content else {
        return None;
    };
    if bytes.is_empty() {
        return None;
    }

    Some(bytes.to_vec())
}

fn decode_node_for_mode(
    payload: &[u8],
    mode: WaProtocolMode,
) -> Result<binary_node::BinaryNode, crate::wa::BinaryNodeError> {
    match mode {
        WaProtocolMode::Synthetic => binary_node::decode(payload),
        WaProtocolMode::RealMd | WaProtocolMode::Auto => binary_node::decode_real(payload),
    }
}

fn extract_qr_reference_from_payload(payload: &[u8], mode: WaProtocolMode) -> Option<String> {
    let node = decode_node_for_mode(payload, mode).ok()?;
    extract_qr_reference_from_node(&node)
}

fn extract_login_jid_from_payload(payload: &[u8], mode: WaProtocolMode) -> Option<String> {
    let node = decode_node_for_mode(payload, mode).ok()?;
    extract_login_jid(&node)
}

fn is_failure_payload(payload: &[u8], mode: WaProtocolMode) -> bool {
    if let Ok(node) = decode_node_for_mode(payload, mode) {
        return node.tag == "failure" || node.tag == "stream:error";
    }

    std::str::from_utf8(payload)
        .map(|raw| raw.contains("failure") || raw.contains("stream:error"))
        .unwrap_or(false)
}

fn build_pair_device_sign_reply(node: &BinaryNode, auth: &AuthState) -> Result<Option<BinaryNode>, String> {
    if node.tag != "iq" {
        return Ok(None);
    }

    let NodeContent::Nodes(children) = &node.content else {
        return Ok(None);
    };
    let Some(pair_success) = children.iter().find(|child| child.tag == "pair-success") else {
        return Ok(None);
    };
    let NodeContent::Nodes(pair_children) = &pair_success.content else {
        return Ok(None);
    };
    let device_identity_node = pair_children
        .iter()
        .find(|child| child.tag == "device-identity")
        .ok_or_else(|| "pair-success missing device-identity".to_owned())?;
    let NodeContent::Bytes(device_identity_raw) = &device_identity_node.content else {
        return Err("pair-success device-identity has invalid content".to_owned());
    };

    let signed_hmac = wa_proto::AdvSignedDeviceIdentityHmac::decode(device_identity_raw.as_ref())
        .map_err(|error| format!("invalid pair-success device-identity: {error}"))?;
    let details = signed_hmac
        .details
        .ok_or_else(|| "pair-success missing signed identity details".to_owned())?;
    let expected_hmac = signed_hmac
        .hmac
        .ok_or_else(|| "pair-success missing signed identity hmac".to_owned())?;

    let mut hmac_input = Vec::with_capacity(2 + details.len());
    if signed_hmac.account_type == Some(wa_proto::AdvEncryptionType::Hosted as i32) {
        hmac_input.extend_from_slice(&WA_ADV_HOSTED_ACCOUNT_SIG_PREFIX);
    }
    hmac_input.extend_from_slice(&details);

    let adv_secret = base64::engine::general_purpose::STANDARD
        .decode(auth.adv_secret_key.trim())
        .map_err(|error| format!("invalid adv_secret_key encoding: {error}"))?;
    let calculated_hmac = hmac_sha256(&adv_secret, &hmac_input);
    if calculated_hmac.as_slice() != expected_hmac.as_slice() {
        return Err("pair-success account hmac signature mismatch".to_owned());
    }

    let mut account = wa_proto::AdvSignedDeviceIdentity::decode(details.as_slice())
        .map_err(|error| format!("invalid signed device identity payload: {error}"))?;
    let account_signature_key = account
        .account_signature_key
        .as_ref()
        .ok_or_else(|| "pair-success missing account_signature_key".to_owned())?;
    let account_signature = account
        .account_signature
        .as_ref()
        .ok_or_else(|| "pair-success missing account_signature".to_owned())?;
    let device_details = account
        .details
        .as_ref()
        .ok_or_else(|| "pair-success missing account details".to_owned())?;
    let device_identity = wa_proto::AdvDeviceIdentity::decode(device_details.as_slice())
        .map_err(|error| format!("invalid ADVDeviceIdentity payload: {error}"))?;

    let account_signature_key = to_fixed_32(account_signature_key, "account_signature_key")?;
    let mut account_message = Vec::with_capacity(2 + device_details.len() + 32);
    if device_identity.device_type == Some(wa_proto::AdvEncryptionType::Hosted as i32) {
        account_message.extend_from_slice(&WA_ADV_HOSTED_ACCOUNT_SIG_PREFIX);
    } else {
        account_message.extend_from_slice(&WA_ADV_ACCOUNT_SIG_PREFIX);
    }
    account_message.extend_from_slice(device_details);
    account_message.extend_from_slice(&auth.identity.identity_key.public);

    if !verify_message(account_signature_key, &account_message, account_signature) {
        return Err("pair-success account signature verification failed".to_owned());
    }

    let mut device_message = Vec::with_capacity(2 + device_details.len() + 32 + 32);
    device_message.extend_from_slice(&WA_ADV_DEVICE_SIG_PREFIX);
    device_message.extend_from_slice(device_details);
    device_message.extend_from_slice(&auth.identity.identity_key.public);
    device_message.extend_from_slice(&account_signature_key);

    let device_signature = sign_message(
        auth.identity.identity_key.private,
        auth.identity.identity_key.public,
        &device_message,
    );
    account.device_signature = Some(device_signature.to_vec());
    account.account_signature_key = None;

    let key_index = device_identity
        .key_index
        .ok_or_else(|| "pair-success missing key_index".to_owned())?;
    let mut encoded_identity = Vec::new();
    account
        .encode(&mut encoded_identity)
        .map_err(|error| format!("failed to encode signed device identity: {error}"))?;

    let message_id = node
        .attrs
        .get("id")
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "pair-success iq missing id".to_owned())?;

    let device_identity_reply = BinaryNode {
        tag: "device-identity".to_owned(),
        attrs: HashMap::from([("key-index".to_owned(), key_index.to_string())]),
        content: NodeContent::Bytes(Bytes::from(encoded_identity)),
    };
    let pair_device_sign = BinaryNode {
        tag: "pair-device-sign".to_owned(),
        attrs: HashMap::new(),
        content: NodeContent::Nodes(vec![device_identity_reply]),
    };

    Ok(Some(BinaryNode {
        tag: "iq".to_owned(),
        attrs: HashMap::from([
            ("to".to_owned(), "s.whatsapp.net".to_owned()),
            ("type".to_owned(), "result".to_owned()),
            ("id".to_owned(), message_id.to_owned()),
        ]),
        content: NodeContent::Nodes(vec![pair_device_sign]),
    }))
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    const BLOCK_SIZE: usize = 64;

    let mut key_block = [0_u8; BLOCK_SIZE];
    if key.len() > BLOCK_SIZE {
        let mut hasher = Sha256::new();
        hasher.update(key);
        let digest = hasher.finalize();
        key_block[..32].copy_from_slice(&digest);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut o_key_pad = [0_u8; BLOCK_SIZE];
    let mut i_key_pad = [0_u8; BLOCK_SIZE];
    for (index, value) in key_block.iter().copied().enumerate() {
        o_key_pad[index] = value ^ 0x5c;
        i_key_pad[index] = value ^ 0x36;
    }

    let mut inner = Sha256::new();
    inner.update(i_key_pad);
    inner.update(data);
    let inner_digest = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(o_key_pad);
    outer.update(inner_digest);
    let digest = outer.finalize();

    let mut out = [0_u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn to_fixed_32(value: &[u8], field: &str) -> Result<[u8; 32], String> {
    if value.len() != 32 {
        return Err(format!("invalid {field} length: {}", value.len()));
    }

    let mut out = [0_u8; 32];
    out.copy_from_slice(value);
    Ok(out)
}

fn maybe_unframe(raw: &[u8]) -> Option<&[u8]> {
    if raw.len() < 3 {
        return None;
    }

    let expected_len = ((raw[0] as usize) << 16) | ((raw[1] as usize) << 8) | raw[2] as usize;
    let payload = &raw[3..];

    if payload.len() < expected_len {
        return None;
    }

    Some(&payload[..expected_len])
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

/// Returns reconnection delay using capped exponential backoff.
pub fn backoff_seconds(attempt: u32) -> u64 {
    match attempt {
        0 => 1,
        1 => 2,
        2 => 4,
        3 => 8,
        4 => 16,
        _ => 30,
    }
}
