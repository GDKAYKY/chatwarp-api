use std::{
    process::Stdio,
    sync::Arc,
    time::Duration,
};

use tokio::{
    process::{Child, Command},
    sync::{RwLock, broadcast, mpsc},
    time::{Instant, MissedTickBehavior},
};

use crate::{
    db::auth_store::AuthStore,
    instance::handle::{ConnectionState, InstanceCommand, InstanceStatus, QrCodeStatus},
    wa::events::Event,
};

/// Main task loop for a wa-rs-backed instance runner.
pub async fn run(
    name: String,
    status: Arc<RwLock<InstanceStatus>>,
    mut command_rx: mpsc::Receiver<InstanceCommand>,
    event_tx: broadcast::Sender<Event>,
    auth_store: Arc<dyn AuthStore>,
    wa_rs_bot_command: Option<String>,
    auth_poll_interval: Duration,
) {
    let mut auto_reconnect = false;
    let mut reconnect_attempt = 0_u32;
    let mut child: Option<Child> = None;
    let mut next_spawn_at = Instant::now();

    let mut tick = tokio::time::interval(auth_poll_interval.max(Duration::from_millis(250)));
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            maybe_command = command_rx.recv() => {
                let Some(command) = maybe_command else {
                    break;
                };

                match command {
                    InstanceCommand::Connect => {
                        auto_reconnect = true;
                        reconnect_attempt = 0;
                        next_spawn_at = Instant::now();
                        transition_to_connecting(&status).await;
                        let _ = event_tx.send(Event::ReconnectScheduled {
                            instance_name: name.clone(),
                            delay_secs: 0,
                        });
                        sync_state_with_auth_store(&name, &status, &event_tx, &auth_store).await;
                        if child.is_none() {
                            match spawn_wa_rs_bot(&name, wa_rs_bot_command.as_deref()) {
                                Ok(spawned) => {
                                    child = spawned;
                                }
                                Err(error) => {
                                    let reason = format!("wa_rs_bot_spawn_failed: {error}");
                                    set_last_error(&status, reason).await;
                                    reconnect_attempt = reconnect_attempt.saturating_add(1);
                                    let delay_secs = backoff_seconds(reconnect_attempt);
                                    next_spawn_at = Instant::now() + Duration::from_secs(delay_secs);
                                    let _ = event_tx.send(Event::ReconnectScheduled {
                                        instance_name: name.clone(),
                                        delay_secs,
                                    });
                                }
                            }
                        }
                    }
                    InstanceCommand::Disconnect => {
                        auto_reconnect = false;
                        reconnect_attempt = 0;
                        stop_child(&mut child).await;
                        transition_to_disconnected(&name, &status, &event_tx, "manual_disconnect").await;
                    }
                    InstanceCommand::MarkConnected => {
                        mark_connected(&name, &status, &event_tx).await;
                    }
                    InstanceCommand::SendMessage { message_id, payload } => {
                        if !is_connected(&status).await {
                            continue;
                        }

                        let queue_result = auth_store.queue_outbound(&name, &message_id, &payload).await;
                        if let Err(error) = queue_result {
                            let reason = format!("wa_rs_outbox_error: {error}");
                            set_last_error(&status, reason).await;
                            continue;
                        }

                        let _ = event_tx.send(Event::OutboundAck {
                            instance_name: name.clone(),
                            message_id,
                            bytes: payload.len(),
                        });
                    }
                    InstanceCommand::Shutdown => {
                        stop_child(&mut child).await;
                        break;
                    }
                }
            }
            _ = tick.tick() => {
                if !auto_reconnect {
                    continue;
                }

                if let Some(active_child) = child.as_mut() {
                    match active_child.try_wait() {
                        Ok(Some(exit_status)) => {
                            child = None;
                            let reason = format!("wa_rs_bot_exited: {exit_status}");
                            transition_to_disconnected(&name, &status, &event_tx, &reason).await;
                            reconnect_attempt = reconnect_attempt.saturating_add(1);
                            next_spawn_at = Instant::now() + Duration::from_secs(backoff_seconds(reconnect_attempt));
                            let _ = event_tx.send(Event::ReconnectScheduled {
                                instance_name: name.clone(),
                                delay_secs: backoff_seconds(reconnect_attempt),
                            });
                        }
                        Ok(None) => {}
                        Err(error) => {
                            child = None;
                            let reason = format!("wa_rs_bot_wait_failed: {error}");
                            transition_to_disconnected(&name, &status, &event_tx, &reason).await;
                            reconnect_attempt = reconnect_attempt.saturating_add(1);
                            next_spawn_at = Instant::now() + Duration::from_secs(backoff_seconds(reconnect_attempt));
                            let _ = event_tx.send(Event::ReconnectScheduled {
                                instance_name: name.clone(),
                                delay_secs: backoff_seconds(reconnect_attempt),
                            });
                        }
                    }
                }

                if child.is_none() && Instant::now() >= next_spawn_at {
                    match spawn_wa_rs_bot(&name, wa_rs_bot_command.as_deref()) {
                        Ok(spawned) => {
                            child = spawned;
                            reconnect_attempt = 0;
                            next_spawn_at = Instant::now();
                        }
                        Err(error) => {
                            let reason = format!("wa_rs_bot_spawn_failed: {error}");
                            set_last_error(&status, reason).await;
                            reconnect_attempt = reconnect_attempt.saturating_add(1);
                            let delay_secs = backoff_seconds(reconnect_attempt);
                            next_spawn_at = Instant::now() + Duration::from_secs(delay_secs);
                            let _ = event_tx.send(Event::ReconnectScheduled {
                                instance_name: name.clone(),
                                delay_secs,
                            });
                        }
                    }
                }

                sync_state_with_auth_store(&name, &status, &event_tx, &auth_store).await;
            }
        }
    }

    stop_child(&mut child).await;
}

fn spawn_wa_rs_bot(instance_name: &str, command: Option<&str>) -> Result<Option<Child>, String> {
    let Some(command) = command else {
        return Ok(None);
    };

    let mut child = Command::new("sh");
    child
        .arg("-lc")
        .arg(command)
        .env("CHATWARP_INSTANCE_NAME", instance_name)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let spawned = child.spawn().map_err(|error| error.to_string())?;
    Ok(Some(spawned))
}

async fn stop_child(child: &mut Option<Child>) {
    let Some(mut active_child) = child.take() else {
        return;
    };

    let _ = active_child.kill().await;
    let _ = active_child.wait().await;
}

async fn sync_state_with_auth_store(
    name: &str,
    status: &Arc<RwLock<InstanceStatus>>,
    event_tx: &broadcast::Sender<Event>,
    auth_store: &Arc<dyn AuthStore>,
) {
    match auth_store.load(name).await {
        Ok(Some(auth)) if auth.metadata.me.is_some() => {
            if !is_connected(status).await {
                mark_connected(name, status, event_tx).await;
            }
        }
        Ok(_) => {
            let mut guard = status.write().await;
            if guard.state != ConnectionState::Connected {
                guard.state = ConnectionState::QrPending;
                guard.qrcode = QrCodeStatus::default();
                guard.last_error = None;
            }
        }
        Err(error) => {
            let reason = format!("wa_rs_auth_load_failed: {error}");
            set_last_error(status, reason).await;
        }
    }
}

async fn transition_to_connecting(status: &Arc<RwLock<InstanceStatus>>) {
    let mut guard = status.write().await;
    guard.state = ConnectionState::Connecting;
    guard.qrcode = QrCodeStatus::default();
    guard.last_error = None;
}

async fn transition_to_disconnected(
    name: &str,
    status: &Arc<RwLock<InstanceStatus>>,
    event_tx: &broadcast::Sender<Event>,
    reason: &str,
) {
    {
        let mut guard = status.write().await;
        guard.state = ConnectionState::Disconnected;
        guard.qrcode = QrCodeStatus::default();
        guard.last_error = Some(reason.to_owned());
    }
    let _ = event_tx.send(Event::Disconnected {
        instance_name: name.to_owned(),
        reason: reason.to_owned(),
    });
}

async fn mark_connected(
    name: &str,
    status: &Arc<RwLock<InstanceStatus>>,
    event_tx: &broadcast::Sender<Event>,
) {
    {
        let mut guard = status.write().await;
        guard.state = ConnectionState::Connected;
        guard.qrcode = QrCodeStatus::default();
        guard.last_error = None;
    }

    let _ = event_tx.send(Event::Connected {
        instance_name: name.to_owned(),
    });
}

async fn is_connected(status: &Arc<RwLock<InstanceStatus>>) -> bool {
    status.read().await.state == ConnectionState::Connected
}

async fn set_last_error(status: &Arc<RwLock<InstanceStatus>>, reason: String) {
    let mut guard = status.write().await;
    guard.last_error = Some(reason);
}

fn backoff_seconds(attempt: u32) -> u64 {
    match attempt {
        0 => 1,
        1 => 2,
        2 => 4,
        3 => 8,
        4 => 16,
        _ => 30,
    }
}
