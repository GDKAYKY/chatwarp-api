use chatwarp_api::api_store::{ApiStore, NoopApiStore};
use chatwarp_api::bot::{Bot, MessageContext};
use chatwarp_api::pair_code::PairCodeOptions;
use chatwarp_api::upload::UploadResponse;
use chatwarp_api_tokio_transport::TokioWebSocketTransportFactory;
use chatwarp_api_ureq_http_client::UreqHttpClient;
use chrono::{Local, Utc};
use log::{error, info};
use serde_json::json;
use std::io::Cursor;
use std::sync::Arc;
use waproto::whatsapp as wa;
use warp_core::download::{Downloadable, MediaType};
use warp_core::proto_helpers::MessageExt;
use warp_core::types::events::Event;

// This is a demo of a simple ping-pong bot with every type of media.
//
// Usage:
//   cargo run                                      # QR code pairing only
//   cargo run -- --phone 15551234567               # Pair code + QR code (concurrent)
//   cargo run -- -p 15551234567                    # Short form
//   cargo run -- -p 15551234567 --code MYCODE12    # Custom 8-char pair code
//   cargo run -- -p 15551234567 -c MYCODE12        # Short form

use chatwarp_api::server::{AppState, InstanceState, SessionRuntime, create_router};
use dashmap::DashMap;

fn main() {
    // Parse CLI arguments for phone number and optional custom code
    let args: Vec<String> = std::env::args().collect();
    let phone_number = parse_arg(&args, "--phone", "-p");
    let custom_code = parse_arg(&args, "--code", "-c");

    if let Some(ref phone) = phone_number {
        eprintln!("Phone number provided: {}", phone);
        if let Some(ref code) = custom_code {
            eprintln!("Custom pair code: {}", code);
        }
        eprintln!("Will use pair code authentication (concurrent with QR)");
    }
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            use std::io::Write;
            writeln!(
                buf,
                "{} [{:<5}] [{}] - {}",
                Local::now().format("%H:%M:%S"),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .init();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to build tokio runtime");

    rt.block_on(async {
        let database_url = std::env::var("DATABASE_URL").ok();

        let (backend, api_store): (Arc<dyn chatwarp_api::store::Backend>, Arc<dyn ApiStore>) =
            if let Some(url) = database_url {
                if url.starts_with("postgres://") || url.starts_with("postgresql://") {
                    #[cfg(feature = "postgres-storage")]
                    {
                        match chatwarp_api::store::PostgresStore::new(&url).await {
                            Ok(store) => {
                                info!("PostgreSQL backend initialized successfully.");
                                let store = Arc::new(store);
                                (store.clone(), store as Arc<dyn ApiStore>)
                            }
                            Err(e) => {
                                error!("Failed to create PostgreSQL backend: {}", e);
                                return;
                            }
                        }
                    }
                    #[cfg(not(feature = "postgres-storage"))]
                    {
                        error!("PostgreSQL support not enabled in this build.");
                        return;
                    }
                } else {
                    #[cfg(feature = "sqlite-storage")]
                    {
                        match chatwarp_api::store::SqliteStore::new(&url).await {
                            Ok(store) => {
                                info!("SQLite backend initialized with custom URL: {}", url);
                                (Arc::new(store), Arc::new(NoopApiStore))
                            }
                            Err(e) => {
                                error!("Failed to create SQLite backend with url {}: {}", url, e);
                                return;
                            }
                        }
                    }
                    #[cfg(not(feature = "sqlite-storage"))]
                    {
                        error!("SQLite support not enabled in this build.");
                        return;
                    }
                }
            } else {
                #[cfg(feature = "sqlite-storage")]
                {
                    match chatwarp_api::store::SqliteStore::new("whatsapp.db").await {
                        Ok(store) => {
                            info!("SQLite backend initialized with default whatsapp.db");
                            (Arc::new(store), Arc::new(NoopApiStore))
                        }
                        Err(e) => {
                            error!("Failed to create SQLite backend: {}", e);
                            return;
                        }
                    }
                }
                #[cfg(not(feature = "sqlite-storage"))]
                {
                    error!("No database URL provided and SQLite support not enabled.");
                    return;
                }
            };

        // Initialize AppState
        let app_state = Arc::new(AppState {
            instances: DashMap::new(),
            sessions_runtime: DashMap::new(),
            api_store: api_store.clone(),
        });

        // Initialize default instance
        let default_instance_name = "default".to_string();
        app_state
            .instances
            .insert(default_instance_name.clone(), InstanceState::new());
        app_state
            .sessions_runtime
            .insert(default_instance_name.clone(), SessionRuntime::new());

        chatwarp_api::server::webhooks::spawn_worker(app_state.clone());
        chatwarp_api::server::webhooks::enqueue(&app_state, None, "APPLICATION_STARTUP", json!({}))
            .await;
        chatwarp_api::server::webhooks::enqueue(&app_state, None, "MESSAGES_SET", json!({})).await;

        let transport_factory = TokioWebSocketTransportFactory::new();
        let http_client = UreqHttpClient::new();

        let mut builder = Bot::builder()
            .with_backend(backend)
            .with_transport_factory(transport_factory)
            .with_http_client(http_client);

        // Add pair code authentication if phone number provided
        if let Some(phone) = phone_number {
            builder = builder.with_pair_code(PairCodeOptions {
                phone_number: phone,
                custom_code,
                ..Default::default()
            });
        }

        let state_for_bot = app_state.clone();
        let name_for_bot = default_instance_name.clone();

        let mut bot = builder
            .on_event(move |event, client| {
                let state = state_for_bot.clone();
                let instance_name = name_for_bot.clone();
                async move {
                    match event {
                        Event::PairingQrCode { code, timeout } => {
                            info!("----------------------------------------");
                            info!(
                                "QR code received (valid for {} seconds):",
                                timeout.as_secs()
                            );
                            info!("\n{}\n", code);
                            info!("----------------------------------------");

                            if let Some(instance) = state.instances.get(&instance_name) {
                                *instance.qr_code.write().await = Some(code);
                                *instance.connection_state.write().await = "qr_pending".to_string();
                                let mut count = instance.qr_count.write().await;
                                *count += 1;
                            }
                        }
                        Event::PairingCode { code, timeout } => {
                            info!("========================================");
                            info!("PAIR CODE (valid for {} seconds):", timeout.as_secs());
                            info!("Enter this code on your phone:");
                            info!("WhatsApp > Linked Devices > Link a Device");
                            info!("> Link with phone number instead");
                            info!("");
                            info!("    >>> {} <<<", code);
                            info!("");
                            info!("========================================");
                        }

                        Event::Message(msg, info) => {
                            let ctx = MessageContext {
                                message: msg,
                                info,
                                client,
                            };

                            if let Some(media_ping_request) = get_pingable_media(&ctx.message) {
                                handle_media_ping(&ctx, media_ping_request).await;
                            }

                            if let Some(text) = ctx.message.text_content()
                                && text == "ping"
                            {
                                info!("Received text ping, sending pong...");

                                // Send reaction to the ping message
                                let message_key = wa::MessageKey {
                                    remote_jid: Some(ctx.info.source.chat.to_string()),
                                    id: Some(ctx.info.id.clone()),
                                    from_me: Some(ctx.info.source.is_from_me),
                                    participant: if ctx.info.source.is_group {
                                        Some(ctx.info.source.sender.to_string())
                                    } else {
                                        None
                                    },
                                };

                                let reaction_emoji = "🏓".to_string();

                                let reaction_message = wa::message::ReactionMessage {
                                    key: Some(message_key),
                                    text: Some(reaction_emoji),
                                    sender_timestamp_ms: Some(Utc::now().timestamp_millis()),
                                    ..Default::default()
                                };

                                let final_message_to_send = wa::Message {
                                    reaction_message: Some(reaction_message),
                                    ..Default::default()
                                };

                                if let Err(e) = ctx.send_message(final_message_to_send).await {
                                    error!("Failed to send reaction: {}", e);
                                }

                                let start = std::time::Instant::now();

                                // Determine participant JID
                                let participant_jid = if ctx.info.source.is_from_me {
                                    ctx.client.get_pn().await.unwrap_or_default().to_string()
                                } else {
                                    ctx.info.source.sender.to_string()
                                };

                                // Construct ContextInfo for quoting
                                let context_info = wa::ContextInfo {
                                    stanza_id: Some(ctx.info.id.clone()),
                                    participant: Some(participant_jid),
                                    quoted_message: Some(ctx.message.clone()),
                                    ..Default::default()
                                };

                                // Create the initial quoted reply message
                                let reply_message = wa::Message {
                                    extended_text_message: Some(Box::new(
                                        wa::message::ExtendedTextMessage {
                                            text: Some("🏓 Pong!".to_string()),
                                            context_info: Some(Box::new(context_info.clone())),
                                            ..Default::default()
                                        },
                                    )),
                                    ..Default::default()
                                };

                                // 1. Send the initial message and get its ID
                                let sent_msg_id = match ctx.send_message(reply_message).await {
                                    Ok(id) => id,
                                    Err(e) => {
                                        error!("Failed to send initial pong message: {}", e);
                                        return;
                                    }
                                };

                                // 2. Calculate the duration
                                let duration = start.elapsed();
                                let duration_str = format!("{:.2?}", duration);

                                info!(
                                    "Send took {}. Editing message {}...",
                                    duration_str, &sent_msg_id
                                );

                                // 3. Create the new content for the message
                                let updated_content = wa::Message {
                                    extended_text_message: Some(Box::new(
                                        wa::message::ExtendedTextMessage {
                                            text: Some(format!("🏓 Pong!\n`{}`", duration_str)),
                                            context_info: Some(Box::new(context_info)),
                                            ..Default::default()
                                        },
                                    )),
                                    ..Default::default()
                                };

                                // 4. Edit the original message with the new content
                                if let Err(e) =
                                    ctx.edit_message(sent_msg_id.clone(), updated_content).await
                                {
                                    error!("Failed to edit message {}: {}", sent_msg_id, e);
                                } else {
                                    info!("Successfully sent edit for message {}.", sent_msg_id);
                                }
                            }
                        }
                        Event::Connected(_) => {
                            info!("✅ Bot connected successfully!");
                            if let Some(instance) = state.instances.get(&instance_name) {
                                *instance.qr_code.write().await = None;
                                *instance.connection_state.write().await = "connected".to_string();
                            }
                        }
                        Event::Receipt(receipt) => {
                            info!(
                                "Got receipt for message(s) {:?}, type: {:?}",
                                receipt.message_ids, receipt.r#type
                            );
                        }
                        Event::LoggedOut(_) => {
                            error!("❌ Bot was logged out!");
                            if let Some(instance) = state.instances.get(&instance_name) {
                                *instance.connection_state.write().await =
                                    "disconnected".to_string();
                            }
                        }
                        _ => {
                            // debug!("Received unhandled event: {:?}", event);
                        }
                    }
                }
            })
            .build()
            .await
            .expect("Failed to build bot");

        let bot_handle = match bot.run().await {
            Ok(handle) => handle,
            Err(e) => {
                error!("Bot failed to start: {}", e);
                return;
            }
        };

        // Start Axum Server
        let app = create_router(app_state);
        let port = std::env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080);
        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

        info!("HTTP server listening on {}", addr);
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

        let server_handle = tokio::spawn(async move {
            axum::serve(listener, app.into_make_service())
                .await
                .unwrap();
        });

        // Wait for both tasks
        tokio::select! {
            _ = bot_handle => info!("Bot stopped"),
            _ = server_handle => info!("Server stopped"),
        }
    });
}

trait MediaPing: Downloadable {
    fn media_type(&self) -> MediaType;

    fn build_pong_reply(&self, upload: UploadResponse) -> wa::Message;
}

impl MediaPing for wa::message::ImageMessage {
    fn media_type(&self) -> MediaType {
        MediaType::Image
    }

    fn build_pong_reply(&self, upload: UploadResponse) -> wa::Message {
        wa::Message {
            image_message: Some(Box::new(wa::message::ImageMessage {
                mimetype: self.mimetype.clone(),
                caption: Some("pong".to_string()),
                url: Some(upload.url),
                direct_path: Some(upload.direct_path),
                media_key: Some(upload.media_key),
                file_enc_sha256: Some(upload.file_enc_sha256),
                file_sha256: Some(upload.file_sha256),
                file_length: Some(upload.file_length),
                ..Default::default()
            })),
            ..Default::default()
        }
    }
}

impl MediaPing for wa::message::VideoMessage {
    fn media_type(&self) -> MediaType {
        MediaType::Video
    }

    fn build_pong_reply(&self, upload: UploadResponse) -> wa::Message {
        wa::Message {
            video_message: Some(Box::new(wa::message::VideoMessage {
                mimetype: self.mimetype.clone(),
                caption: Some("pong".to_string()),
                url: Some(upload.url),
                direct_path: Some(upload.direct_path),
                media_key: Some(upload.media_key),
                file_enc_sha256: Some(upload.file_enc_sha256),
                file_sha256: Some(upload.file_sha256),
                file_length: Some(upload.file_length),
                gif_playback: self.gif_playback,
                height: self.height,
                width: self.width,
                seconds: self.seconds,
                gif_attribution: self.gif_attribution,
                ..Default::default()
            })),
            ..Default::default()
        }
    }
}

fn get_pingable_media<'a>(message: &'a wa::Message) -> Option<&'a (dyn MediaPing + 'a)> {
    let base_message = message.get_base_message();

    if let Some(msg) = &base_message.image_message
        && msg.caption.as_deref() == Some("ping")
    {
        return Some(&**msg);
    }
    if let Some(msg) = &base_message.video_message
        && msg.caption.as_deref() == Some("ping")
    {
        return Some(&**msg);
    }

    None
}

async fn handle_media_ping(ctx: &MessageContext, media: &(dyn MediaPing + '_)) {
    info!(
        "Received {:?} ping from {}",
        media.media_type(),
        ctx.info.source.sender
    );

    let mut data_buffer = Cursor::new(Vec::new());
    if let Err(e) = ctx.client.download_to_file(media, &mut data_buffer).await {
        error!("Failed to download media: {}", e);
        let _ = ctx
            .send_message(wa::Message {
                conversation: Some("Failed to download your media.".to_string()),
                ..Default::default()
            })
            .await;
        return;
    }

    info!(
        "Successfully downloaded media. Size: {} bytes. Now uploading...",
        data_buffer.get_ref().len()
    );
    let plaintext_data = data_buffer.into_inner();
    let upload_response = match ctx.client.upload(plaintext_data, media.media_type()).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to upload media: {}", e);
            let _ = ctx
                .send_message(wa::Message {
                    conversation: Some("Failed to re-upload the media.".to_string()),
                    ..Default::default()
                })
                .await;
            return;
        }
    };

    info!("Successfully uploaded media. Constructing reply message...");
    let reply_msg = media.build_pong_reply(upload_response);

    if let Err(e) = ctx.send_message(reply_msg).await {
        error!("Failed to send media pong reply: {}", e);
    } else {
        info!("Media pong reply sent successfully.");
    }
}

/// Parse a CLI argument by its long and short flags.
/// Supports: --flag VALUE, -f VALUE, --flag=VALUE
fn parse_arg(args: &[String], long: &str, short: &str) -> Option<String> {
    let long_prefix = format!("{}=", long);
    let mut iter = args.iter().skip(1); // Skip program name
    while let Some(arg) = iter.next() {
        if arg == long || arg == short {
            return iter.next().cloned();
        }
        if let Some(value) = arg.strip_prefix(&long_prefix) {
            return Some(value.to_string());
        }
    }
    None
}
