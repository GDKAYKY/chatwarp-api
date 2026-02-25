use std::sync::Arc;

use tokio::sync::{RwLock, broadcast, mpsc};

use crate::{
    instance::handle::{ConnectionState, InstanceCommand},
    wa::events::Event,
};

/// Main task loop for a single instance.
pub async fn run(
    name: String,
    state: Arc<RwLock<ConnectionState>>,
    mut command_rx: mpsc::Receiver<InstanceCommand>,
    event_tx: broadcast::Sender<Event>,
) {
    while let Some(command) = command_rx.recv().await {
        match command {
            InstanceCommand::Connect => {
                connect_flow(&name, &state, &event_tx).await;
            }
            InstanceCommand::Disconnect => {
                let mut guard = state.write().await;
                *guard = ConnectionState::Disconnected;
                let _ = event_tx.send(Event::Disconnected {
                    instance_name: name.clone(),
                    reason: "manual_disconnect".to_owned(),
                });
            }
            InstanceCommand::MarkConnected => {
                let mut guard = state.write().await;
                *guard = ConnectionState::Connected;
                let _ = event_tx.send(Event::Connected {
                    instance_name: name.clone(),
                });
            }
            InstanceCommand::SendMessage(payload) => {
                let guard = state.read().await;
                if *guard == ConnectionState::Connected {
                    let _ = event_tx.send(Event::OutboundAck {
                        instance_name: name.clone(),
                        bytes: payload.len(),
                    });
                }
            }
            InstanceCommand::Shutdown => break,
        }
    }
}

async fn connect_flow(
    name: &str,
    state: &Arc<RwLock<ConnectionState>>,
    event_tx: &broadcast::Sender<Event>,
) {
    {
        let mut guard = state.write().await;
        *guard = ConnectionState::Connecting;
    }

    let _ = event_tx.send(Event::ReconnectScheduled {
        instance_name: name.to_owned(),
        delay_secs: backoff_seconds(0),
    });

    {
        let mut guard = state.write().await;
        *guard = ConnectionState::QrPending;
    }

    let _ = event_tx.send(Event::QrCode(format!("qr:{name}:synthetic")));
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
