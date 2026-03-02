use std::sync::Arc;
use tokio::sync::oneshot;
use tracing::{error, info};
use wa_rs::{
    bot::Bot,
    store::SqliteStore,
    transport::{TokioWebSocketTransportFactory, UreqHttpClient},
    types::events::Event,
    wa_rs_proto::whatsapp as wa,
};

use crate::{config::AppConfig, error::AppError};
use qrcode::QrCode;
use qrcode::render::unicode;

pub async fn run_client(config: &AppConfig) -> Result<(), AppError> {
    let (tx, rx) = oneshot::channel::<()>();
    let tx = Arc::new(tokio::sync::Mutex::new(Some(tx)));

    let recipient_jid = config
        .recipient_jid
        .parse::<wa_rs::Jid>()
        .map_err(|e| AppError::wa(format!("Invalid JID format: {}", e)))?;

    let message_text = config.message_text.clone();
    let auth_path = config.auth_storage_path.clone();

    info!(storage = %auth_path, "initializing_whatsapp_client");

    let backend = Arc::new(SqliteStore::new(&auth_path).await.map_err(|e| {
        error!(error = %e, path = %auth_path, "failed_to_initialize_sqlite_store");
        AppError::wa(e)
    })?);
    info!(path = %auth_path, "sqlite_store_initialized");

    let mut bot = Bot::builder()
        .with_backend(backend)
        .with_transport_factory(TokioWebSocketTransportFactory::new())
        .with_http_client(UreqHttpClient::new())
        .on_event(move |event, client| {
            let tx = tx.clone();
            let recipient_jid = recipient_jid.clone();
            let message_text = message_text.clone();

            async move {
                match event {
                    Event::PairingQrCode { code, .. } => {
                        match QrCode::new(code.as_bytes()) {
                            Ok(qr) => {
                                let image = qr.render::<unicode::Dense1x2>().build();
                                println!("\n=== QR CODE (Scan me!) ===\n");
                                println!("{}", image);
                                println!("\n==========================\n");
                            }
                            Err(_) => {
                                println!("\n=== QR CODE (Raw) ===");
                                println!("{}", code);
                                println!("================\n");
                            }
                        }
                        info!(qr_code = %code, "qr_code_printed");
                    }
                    Event::Connected(_) => {
                        info!("connection_state_update: connected");

                        let mut msg = wa::Message::default();
                        msg.conversation = Some(message_text);

                        info!(recipient = %recipient_jid, "sending_message");
                        match client.send_message(recipient_jid, msg).await {
                            Ok(id) => {
                                info!(message_id = %id, "message_sent_successfully");
                            }
                            Err(e) => {
                                error!(error = %e, "message_send_failed");
                            }
                        }

                        // Signal completion
                        let mut guard = tx.lock().await;
                        if let Some(sender) = guard.take() {
                            let _ = sender.send(());
                        }
                    }
                    Event::Disconnected(_) => {
                        info!("connection_state_update: disconnected");
                    }
                    _ => {}
                }
            }
        })
        .build()
        .await
        .map_err(|e| AppError::wa(e))?;

    // bot.run() spawns background loops for connection and sync
    bot.run().await.map_err(|e| AppError::wa(e))?;

    info!("waiting_for_message_delivery");

    // Wait for the message to be sent or some timeout
    let _ = rx.await;

    info!("client_task_finished");
    Ok(())
}
