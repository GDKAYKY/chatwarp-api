use std::{
    sync::Arc,
    time::Duration,
};

use tokio::{
    sync::{RwLock, broadcast, mpsc},
    time::Instant,
};

use crate::{
    db::auth_store::AuthStore,
    instance::handle::{ConnectionState, InstanceCommand},
    wa::{
        NoiseState,
        auth::{AuthState, MeInfo},
        binary_node,
        error::NoiseError,
        events::Event,
        handshake::do_handshake,
        qr::generate_qr_string,
        transport::WsTransport,
    },
};

#[derive(Default)]
struct RunnerSession {
    transport: Option<WsTransport>,
    noise: Option<NoiseState>,
    auth: Option<AuthState>,
    awaiting_login: bool,
    login_deadline: Option<Instant>,
    reconnect_attempt: u32,
    auto_reconnect: bool,
}

const LOGIN_TIMEOUT: Duration = Duration::from_secs(60);

/// Main task loop for a single instance.
pub async fn run(
    name: String,
    state: Arc<RwLock<ConnectionState>>,
    mut command_rx: mpsc::Receiver<InstanceCommand>,
    event_tx: broadcast::Sender<Event>,
    auth_store: Arc<dyn AuthStore>,
    wa_ws_url: String,
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
                        &state,
                        &event_tx,
                        &auth_store,
                        &wa_ws_url,
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
                                &state,
                                &event_tx,
                                &auth_store,
                                &mut session,
                                frame.as_ref(),
                            ).await {
                                force_disconnected(&name, &state, &event_tx, &mut session, &error).await;
                                if session.auto_reconnect {
                                    establish_connection(
                                        &name,
                                        &state,
                                        &event_tx,
                                        &auth_store,
                                        &wa_ws_url,
                                        &mut session,
                                        true,
                                    ).await;
                                }
                            }
                        }
                        Err(error) => {
                            let reason = format!("transport_error: {error}");
                            force_disconnected(&name, &state, &event_tx, &mut session, &reason).await;
                            if session.auto_reconnect {
                                establish_connection(
                                    &name,
                                    &state,
                                    &event_tx,
                                    &auth_store,
                                    &wa_ws_url,
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
                    force_disconnected(&name, &state, &event_tx, &mut session, "login_timeout").await;
                    if session.auto_reconnect {
                        establish_connection(
                            &name,
                            &state,
                            &event_tx,
                            &auth_store,
                            &wa_ws_url,
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
            &state,
            &event_tx,
            &auth_store,
            &wa_ws_url,
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
    state: &Arc<RwLock<ConnectionState>>,
    event_tx: &broadcast::Sender<Event>,
    auth_store: &Arc<dyn AuthStore>,
    wa_ws_url: &str,
    session: &mut RunnerSession,
    command: InstanceCommand,
) -> bool {
    match command {
        InstanceCommand::Connect => {
            session.auto_reconnect = true;
            if session.transport.is_none() {
                establish_connection(
                    name,
                    state,
                    event_tx,
                    auth_store,
                    wa_ws_url,
                    session,
                    false,
                )
                .await;
            }
            true
        }
        InstanceCommand::Disconnect => {
            session.auto_reconnect = false;
            force_disconnected(name, state, event_tx, session, "manual_disconnect").await;
            true
        }
        InstanceCommand::MarkConnected => {
            if let Err(error) = mark_connected(name, state, event_tx, auth_store, session).await {
                let reason = format!("mark_connected_failed: {error}");
                force_disconnected(name, state, event_tx, session, &reason).await;
            }
            true
        }
        InstanceCommand::SendMessage {
            message_id,
            payload,
        } => {
            if state.read().await.clone() == ConnectionState::Connected {
                let result = send_encrypted_payload(session, &payload).await;
                if let Err(error) = result {
                    let reason = format!("send_failed: {error}");
                    force_disconnected(name, state, event_tx, session, &reason).await;
                    if session.auto_reconnect {
                        establish_connection(
                            name,
                            state,
                            event_tx,
                            auth_store,
                            wa_ws_url,
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
    state: &Arc<RwLock<ConnectionState>>,
    event_tx: &broadcast::Sender<Event>,
    auth_store: &Arc<dyn AuthStore>,
    wa_ws_url: &str,
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

        match connect_once(name, state, event_tx, auth_store, wa_ws_url, session).await {
            Ok(()) => {
                session.reconnect_attempt = 0;
                return;
            }
            Err(error) => {
                tracing::warn!(instance = %name, error = %error, "connect_once failed");
                force_disconnected(name, state, event_tx, session, &error).await;
                session.reconnect_attempt = session.reconnect_attempt.saturating_add(1);
                should_sleep = true;
            }
        }
    }
}

async fn connect_once(
    name: &str,
    state: &Arc<RwLock<ConnectionState>>,
    event_tx: &broadcast::Sender<Event>,
    auth_store: &Arc<dyn AuthStore>,
    wa_ws_url: &str,
    session: &mut RunnerSession,
) -> Result<(), String> {
    {
        let mut guard = state.write().await;
        *guard = ConnectionState::Connecting;
    }

    let loaded_auth = auth_store
        .load(name)
        .await
        .map_err(|error| format!("auth_load_failed: {error}"))?;
    let mut auth = loaded_auth.unwrap_or_else(AuthState::new);

    let mut transport = WsTransport::connect(wa_ws_url)
        .await
        .map_err(|error| format!("ws_connect_failed: {error}"))?;

    let outcome = do_handshake(&mut transport, &auth.identity.identity_key)
        .await
        .map_err(|error| format!("handshake_failed: {error}"))?;

    session.transport = Some(transport);
    session.noise = Some(outcome.noise);
    session.auth = Some(auth.clone());
    session.awaiting_login = true;
    session.login_deadline = Some(Instant::now() + LOGIN_TIMEOUT);

    if let Some(reference) = outcome.qr_reference {
        {
            let mut guard = state.write().await;
            *guard = ConnectionState::QrPending;
        }

        let adv_key = auth.identity.signed_pre_key.public;
        let qr = generate_qr_string(
            &reference,
            &outcome.noise_public,
            &auth.identity.identity_key.public,
            &adv_key,
        );
        let _ = event_tx.send(Event::QrCode(qr));
        session.awaiting_login = true;
    }

    if let Some(jid) = outcome
        .login_jid
        .or_else(|| extract_login_jid_from_payload(&outcome.server_payload))
    {
        auth.metadata.me = Some(MeInfo {
            jid,
            push_name: None,
        });
        session.auth = Some(auth);
        session.awaiting_login = false;
        session.login_deadline = None;
        mark_connected(name, state, event_tx, auth_store, session)
            .await
            .map_err(|error| format!("save_auth_failed: {error}"))?;
    }

    Ok(())
}

async fn send_encrypted_payload(session: &mut RunnerSession, payload: &[u8]) -> Result<(), String> {
    let Some(noise) = session.noise.as_mut() else {
        return Err("missing_noise_state".to_owned());
    };
    let Some(transport) = session.transport.as_mut() else {
        return Err("missing_transport".to_owned());
    };

    let ad = noise.handshake_hash();
    let encrypted = noise
        .encrypt_with_ad(payload, &ad)
        .map_err(|error| format!("noise_encrypt_failed: {error}"))?;
    transport
        .send_frame(&encrypted)
        .await
        .map_err(|error| format!("transport_send_failed: {error}"))
}

async fn handle_incoming_frame(
    name: &str,
    state: &Arc<RwLock<ConnectionState>>,
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

    let decrypted = decrypt_incoming_payload(noise, frame)
        .map_err(|error| format!("noise_decrypt_failed: {error}"))?;

    if session.awaiting_login {
        let node = match binary_node::decode(&decrypted) {
            Ok(node) => node,
            Err(error) => {
                tracing::warn!(instance = name, error = %error, "binary_node decode failed, ignoring frame");
                return Ok(());
            }
        };

        if let Some(jid) = extract_login_jid(&node) {
            if let Some(auth) = session.auth.as_mut() {
                auth.metadata.me = Some(MeInfo {
                    jid,
                    push_name: None,
                });
            }
            session.awaiting_login = false;
            session.login_deadline = None;
            mark_connected(name, state, event_tx, auth_store, session)
                .await
                .map_err(|error| format!("save_auth_failed: {error}"))?;
            return Ok(());
        }
    }

    if is_failure_payload(&decrypted) {
        return Err("server_reported_failure".to_owned());
    }

    Ok(())
}

fn decrypt_incoming_payload(noise: &mut NoiseState, raw_frame: &[u8]) -> Result<Vec<u8>, NoiseError> {
    let ad = noise.handshake_hash();

    // Try raw first: some peers deliver encrypted payload directly in the websocket binary message.
    let mut raw_noise = noise.clone();
    if let Ok(decrypted) = raw_noise.decrypt_with_ad(raw_frame, &ad) {
        *noise = raw_noise;
        return Ok(decrypted);
    }

    // Fallback for peers that still prepend a WA 3-byte frame header.
    if let Some(payload) = maybe_unframe(raw_frame) {
        let mut framed_noise = noise.clone();
        if let Ok(decrypted) = framed_noise.decrypt_with_ad(payload, &ad) {
            *noise = framed_noise;
            return Ok(decrypted);
        }
    }

    Err(NoiseError::Cipher)
}

async fn mark_connected(
    name: &str,
    state: &Arc<RwLock<ConnectionState>>,
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
        let mut guard = state.write().await;
        *guard = ConnectionState::Connected;
    }
    let _ = event_tx.send(Event::Connected {
        instance_name: name.to_owned(),
    });

    Ok(())
}

async fn force_disconnected(
    name: &str,
    state: &Arc<RwLock<ConnectionState>>,
    event_tx: &broadcast::Sender<Event>,
    session: &mut RunnerSession,
    reason: &str,
) {
    session.transport = None;
    session.noise = None;
    session.awaiting_login = false;
    session.login_deadline = None;
    {
        let mut guard = state.write().await;
        *guard = ConnectionState::Disconnected;
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
    let jid = pair.attrs.get("jid")?.trim();
    if jid.is_empty() {
        return None;
    }

    Some(jid.to_owned())
}

fn extract_login_jid_from_payload(payload: &[u8]) -> Option<String> {
    let node = binary_node::decode(payload).ok()?;
    extract_login_jid(&node)
}

fn is_failure_payload(payload: &[u8]) -> bool {
    if let Ok(node) = binary_node::decode(payload) {
        return node.tag == "failure";
    }

    std::str::from_utf8(payload)
        .map(|raw| raw.contains("failure"))
        .unwrap_or(false)
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
