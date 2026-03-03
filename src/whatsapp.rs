use std::sync::Arc;

use log::{error, info};
use qrcode::QrCode;
use qrcode::render::unicode;
use tokio::sync::oneshot;
use warp_core::types::events::Event;
use warp_core_binary::jid::Jid;

use crate::bot::Bot;
use crate::config::AppConfig;
use crate::error::AppError;
#[cfg(feature = "sqlite-storage")]
use crate::store::SqliteStore;
#[cfg(feature = "tokio-transport")]
use chatwarp_api_tokio_transport::TokioWebSocketTransportFactory;
#[cfg(feature = "ureq-client")]
use chatwarp_api_ureq_http_client::UreqHttpClient;
use waproto::whatsapp as wa;

#[cfg(all(
    feature = "sqlite-storage",
    feature = "tokio-transport",
    feature = "ureq-client"
))]
pub async fn run_client(config: &AppConfig) -> Result<(), AppError> {
    let (tx, rx) = oneshot::channel::<()>();
    let tx = Arc::new(tokio::sync::Mutex::new(Some(tx)));

    let recipient_jid = config
        .recipient_jid
        .parse::<Jid>()
        .map_err(|e| AppError::wa(format!("Invalid JID format: {}", e)))?;

    let message_text = config.message_text.clone();
    let auth_path = config.auth_storage_path.clone();

    info!("initializing_whatsapp_client storage={}", auth_path);

    let backend = Arc::new(SqliteStore::new(&auth_path).await.map_err(|e| {
        error!(
            "failed_to_initialize_sqlite_store error={} path={}",
            e, auth_path
        );
        AppError::wa(e)
    })?);
    info!("sqlite_store_initialized path={}", auth_path);

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
                        info!("qr_code_printed qr_code={}", code);
                    }
                    Event::Connected(_) => {
                        info!("connection_state_update: connected");

                        let mut msg = wa::Message::default();
                        msg.conversation = Some(message_text);

                        info!("sending_message recipient={}", recipient_jid);
                        match client.send_message(recipient_jid, msg).await {
                            Ok(id) => {
                                info!("message_sent_successfully message_id={}", id);
                            }
                            Err(e) => {
                                error!("message_send_failed error={}", e);
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

#[cfg(not(all(
    feature = "sqlite-storage",
    feature = "tokio-transport",
    feature = "ureq-client"
)))]
pub async fn run_client(_config: &AppConfig) -> Result<(), AppError> {
    Err(AppError::wa(
        "run_client requires features: `sqlite-storage`, `tokio-transport`, and `ureq-client`",
    ))
}
