use base64::Engine as _;
use chatwarp_api::api_store::{ApiStore, NoopApiStore};
use chatwarp_api::bot::Bot;
use chatwarp_api::models::message_model::{IncomingMessageMetadata, MessageContext};
use chatwarp_api::pair_code::PairCodeOptions;
use chatwarp_api::upload::UploadResponse;
use chatwarp_api_tokio_transport::TokioWebSocketTransportFactory;
use chatwarp_api_ureq_http_client::UreqHttpClient;
use chrono::Utc;
use serde_json::json;
use std::io::Cursor;
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
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

fn init_tracing() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
        // .add_directive("ureq_proto::util=warn".parse().unwrap());

    let _ = tracing_subscriber::registry()
        .with(env_filter)
        .with(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_target(true)
                .with_thread_ids(false),
        )
        .try_init();
}

fn main() {
    init_tracing();

    // Parse CLI arguments for phone number and optional custom code
    let args: Vec<String> = std::env::args().collect();
    let phone_number = parse_arg(&args, "--phone", "-p");
    let custom_code = parse_arg(&args, "--code", "-c");

    if let Some(ref phone) = phone_number {
        info!(phone = %phone, "Phone number provided via CLI");
        if let Some(ref code) = custom_code {
            info!(pair_code = %code, "Custom pair code provided via CLI");
        }
        info!("Using pair code authentication concurrently with QR");
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to build tokio runtime");

    // Pre-load settings from env before spawning tokio threads
    let initial_settings = chatwarp_api::server::Settings::new();

    rt.block_on(async {
        let database_url = std::env::var("DATABASE_URL").ok();

        let (backend, api_store): (Arc<dyn chatwarp_api::store::Backend>, Arc<dyn ApiStore>) =
            if let Some(url) = database_url {
                if url.starts_with("postgres://") || url.starts_with("postgresql://") {
                    #[cfg(feature = "postgres-storage")]
                    {
                        match chatwarp_api::store::PostgresStore::new(&url).await {
                            Ok(store) => {
                                info!("PostgreSQL backend initialized");
                                let store = Arc::new(store);
                                (store.clone(), store as Arc<dyn ApiStore>)
                            }
                            Err(e) => {
                                error!(error = %e, "Failed to create PostgreSQL backend");
                                return;
                            }
                        }
                    }
                    #[cfg(not(feature = "postgres-storage"))]
                    {
                        error!("PostgreSQL support is not enabled in this build");
                        return;
                    }
                } else {
                    #[cfg(feature = "sqlite-storage")]
                    {
                        match chatwarp_api::store::SqliteStore::new(&url).await {
                            Ok(store) => {
                                info!(database_url = %url, "SQLite backend initialized with custom URL");
                                (Arc::new(store), Arc::new(NoopApiStore))
                            }
                            Err(e) => {
                                error!(database_url = %url, error = %e, "Failed to create SQLite backend");
                                return;
                            }
                        }
                    }
                    #[cfg(not(feature = "sqlite-storage"))]
                    {
                        error!("SQLite support is not enabled in this build");
                        return;
                    }
                }
            } else {
                #[cfg(feature = "sqlite-storage")]
                {
                    match chatwarp_api::store::SqliteStore::new("whatsapp.db").await {
                        Ok(store) => {
                            info!(database_url = "whatsapp.db", "SQLite backend initialized with default database");
                            (Arc::new(store), Arc::new(NoopApiStore))
                        }
                        Err(e) => {
                            error!(error = %e, "Failed to create SQLite backend");
                            return;
                        }
                    }
                }
                #[cfg(not(feature = "sqlite-storage"))]
                {
                    error!("No database URL provided and SQLite support is not enabled");
                    return;
                }
            };

        let api_password = std::env::var("CHATWARP_PASSWORD")
            .ok()
            .filter(|v| !v.is_empty());
        let api_password_hash = api_password.as_deref().map(|v| {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(v.as_bytes());
            let result = hasher.finalize();
            let mut out = [0u8; 32];
            out.copy_from_slice(&result[..]);
            out
        });
        if api_password_hash.is_some() {
            info!("HTTP API auth enabled via CHATWARP_PASSWORD");
        }

        let session_ttl_seconds = std::env::var("CHATWARP_SESSION_TTL_SECONDS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(1800);

        let (message_notify_tx, message_notify_rx) = tokio::sync::mpsc::channel(1024);

        // Initialize AppState
        let app_state = Arc::new(AppState {
            instances: DashMap::new(),
            sessions_runtime: DashMap::new(),
            api_store: api_store.clone(),
            clients: DashMap::new(),
            settings: Arc::new(tokio::sync::RwLock::new(initial_settings)),
            api_password_hash,
            session_ttl_seconds,
            message_notify: message_notify_tx,
            webhook_config_cache: DashMap::new(),
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
        let startup_enabled = app_state.settings.read().await.is_event_enabled("APPLICATION_STARTUP");
        if startup_enabled {
            chatwarp_api::server::webhooks::enqueue(&app_state, None, "APPLICATION_STARTUP", json!({})).await;
        }
        let messages_set_enabled = app_state.settings.read().await.is_event_enabled("MESSAGES_SET");
        if messages_set_enabled {
            chatwarp_api::server::webhooks::enqueue(&app_state, None, "MESSAGES_SET", json!({})).await;
        }

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
                            info!(timeout_secs = timeout.as_secs(), qr_code = %code, "Pairing QR code received");

                            if let Some(instance) = state.instances.get(&instance_name) {
                                *instance.qr_code.write().await = Some(code.clone());
                                *instance.connection_state.write().await = "qr_pending".to_string();
                                let mut count = instance.qr_count.write().await;
                                *count += 1;
                            }

                            chatwarp_api::server::webhooks::enqueue(
                                &state,
                                Some(&instance_name),
                                "QRCODE_UPDATED",
                                json!({ "qrcode": code, "timeout": timeout.as_secs() })
                            ).await;
                        }
                        Event::PairingCode { code, timeout } => {
                            info!(
                                timeout_secs = timeout.as_secs(),
                                pair_code = %code,
                                instructions = "WhatsApp > Linked Devices > Link a Device > Link with phone number instead",
                                "Pair code generated"
                            );
                        }

                        Event::Message(msg, info) => {
                            let ctx = MessageContext {
                                message: msg.clone(),
                                info: info.clone(),
                                client: client.clone(),
                            };

                            let metadata = IncomingMessageMetadata::from_message(&msg, &info);
                            let sender_jid = metadata.sender_jid.clone();
                            let remote_jid = metadata.remote_jid.clone();
                            let is_from_me = metadata.is_from_me;
                            let text_content = metadata.text_content.clone();

                            // Speculatively pre-warm the E2E session for this DM sender.
                            // Cost on hot path: one moka cache lookup (~ns). Cost on cold path:
                            // background prekey fetch that makes the *reply* instant.
                            if !is_from_me && !info.source.is_group {
                                let warm_client = client.clone();
                                let sender_jid_for_warm = info.source.sender.clone();
                                tokio::spawn(async move {
                                    let jid = sender_jid_for_warm.to_non_ad();
                                    if let Ok(devices) = warm_client
                                        .get_user_devices(std::slice::from_ref(&jid))
                                        .await
                                    {
                                        let _ = warm_client.ensure_e2e_sessions(devices).await;
                                    }
                                });
                            }

                            // Fire-and-forget: webhook processing runs in background so the
                            // message handler can respond to the user immediately.
                            {
                                let bg_state = Arc::clone(&state);
                                let bg_instance = Arc::new(instance_name.clone());
                                let bg_msg = Arc::new(msg.clone());
                                let bg_client = Arc::clone(&client);
                                let bg_text = Arc::new(text_content.clone());
                                let bg_sender = Arc::new(sender_jid.clone());
                                let bg_remote = Arc::new(remote_jid.clone());
                                let bg_info = Arc::new(info.clone());

                                tokio::spawn(async move {
                                    let base64_enabled = match chatwarp_api::server::webhooks::load_instance_webhook(
                                        &bg_state,
                                        bg_instance.as_str(),
                                    )
                                    .await
                                    {
                                        Ok(Some(cfg)) if cfg.enabled && cfg.base64 => true,
                                        _ => {
                                            let global_enabled = std::env::var("WEBHOOK_GLOBAL_ENABLED")
                                                .ok()
                                                .map(|v| v == "true" || v == "1")
                                                .unwrap_or(false);
                                            let global_base64 = std::env::var("WEBHOOK_GLOBAL_WEBHOOK_BASE64")
                                                .ok()
                                                .map(|v| v == "true" || v == "1")
                                                .unwrap_or(false);
                                            global_enabled && global_base64
                                        }
                                    };

                                    let message_payload = if let Some(image) = bg_msg.as_ref().image_message.as_deref() {
                                        let mut message = serde_json::Map::new();
                                        message.insert("messageType".to_string(), json!("image"));

                                        if let Some(url) = &image.url {
                                            message.insert("url".to_string(), json!(url));
                                        }
                                        if let Some(mimetype) = &image.mimetype {
                                            message.insert("mimetype".to_string(), json!(mimetype));
                                        }
                                        if let Some(caption) = &image.caption {
                                            message.insert("text".to_string(), json!(caption));
                                        }
                                        if let Some(file_length) = image.file_length {
                                            message.insert("fileLength".to_string(), json!(file_length));
                                        }

                                        if base64_enabled {
                                            match bg_client.download(image).await {
                                                Ok(bytes) => {
                                                    let mime = image
                                                        .mimetype
                                                        .as_deref()
                                                        .unwrap_or("application/octet-stream");
                                                    let encoded = base64::engine::general_purpose::STANDARD
                                                        .encode(bytes);
                                                    let data_url = format!("data:{};base64,{}", mime, encoded);
                                                    message.insert("base64".to_string(), json!(data_url));
                                                }
                                                Err(e) => {
                                                    error!(error = %e, "Failed to download image for webhook base64");
                                                }
                                            }
                                        }

                                        serde_json::Value::Object(message)
                                    } else if let Some(video) = bg_msg.as_ref().video_message.as_deref() {
                                        let mut message = serde_json::Map::new();
                                        message.insert("messageType".to_string(), json!("video"));

                                        if let Some(url) = &video.url {
                                            message.insert("url".to_string(), json!(url));
                                        }
                                        if let Some(mimetype) = &video.mimetype {
                                            message.insert("mimetype".to_string(), json!(mimetype));
                                        }
                                        if let Some(caption) = &video.caption {
                                            message.insert("text".to_string(), json!(caption));
                                        }
                                        if let Some(file_length) = video.file_length {
                                            message.insert("fileLength".to_string(), json!(file_length));
                                        }

                                        if base64_enabled {
                                            match bg_client.download(video).await {
                                                Ok(bytes) => {
                                                    let mime = video
                                                        .mimetype
                                                        .as_deref()
                                                        .unwrap_or("application/octet-stream");
                                                    let encoded = base64::engine::general_purpose::STANDARD
                                                        .encode(bytes);
                                                    let data_url = format!("data:{};base64,{}", mime, encoded);
                                                    message.insert("base64".to_string(), json!(data_url));
                                                }
                                                Err(e) => {
                                                    error!(error = %e, "Failed to download video for webhook base64");
                                                }
                                            }
                                        }

                                        serde_json::Value::Object(message)
                                    } else if let Some(audio) = bg_msg.as_ref().audio_message.as_deref() {
                                        let mut message = serde_json::Map::new();
                                        message.insert("messageType".to_string(), json!("voice"));

                                        if let Some(url) = &audio.url {
                                            message.insert("url".to_string(), json!(url));
                                        }
                                        if let Some(mimetype) = &audio.mimetype {
                                            message.insert("mimetype".to_string(), json!(mimetype));
                                        }
                                        if let Some(file_length) = audio.file_length {
                                            message.insert("fileLength".to_string(), json!(file_length));
                                        }
                                        if let Some(ptt) = audio.ptt {
                                            message.insert("ptt".to_string(), json!(ptt));
                                        }

                                        if base64_enabled {
                                            match bg_client.download(audio).await {
                                                Ok(bytes) => {
                                                    let mime = audio
                                                        .mimetype
                                                        .as_deref()
                                                        .unwrap_or("application/octet-stream");
                                                    let encoded = base64::engine::general_purpose::STANDARD
                                                        .encode(bytes);
                                                    let data_url = format!("data:{};base64,{}", mime, encoded);
                                                    message.insert("base64".to_string(), json!(data_url));
                                                }
                                                Err(e) => {
                                                    error!(error = %e, "Failed to download audio for webhook base64");
                                                }
                                            }
                                        }

                                        serde_json::Value::Object(message)
                                    } else if let Some(doc) = bg_msg.as_ref().document_message.as_deref() {
                                        let mut message = serde_json::Map::new();
                                        message.insert("messageType".to_string(), json!("file"));

                                        if let Some(url) = &doc.url {
                                            message.insert("url".to_string(), json!(url));
                                        }
                                        if let Some(mimetype) = &doc.mimetype {
                                            message.insert("mimetype".to_string(), json!(mimetype));
                                        }
                                        if let Some(caption) = &doc.caption {
                                            message.insert("text".to_string(), json!(caption));
                                        }
                                        if let Some(file_name) = &doc.file_name {
                                            message.insert("filename".to_string(), json!(file_name));
                                        }
                                        if let Some(file_length) = doc.file_length {
                                            message.insert("fileLength".to_string(), json!(file_length));
                                        }

                                        if base64_enabled {
                                            match bg_client.download(doc).await {
                                                Ok(bytes) => {
                                                    let mime = doc
                                                        .mimetype
                                                        .as_deref()
                                                        .unwrap_or("application/octet-stream");
                                                    let encoded = base64::engine::general_purpose::STANDARD
                                                        .encode(bytes);
                                                    let data_url = format!("data:{};base64,{}", mime, encoded);
                                                    message.insert("base64".to_string(), json!(data_url));
                                                }
                                                Err(e) => {
                                                    error!(error = %e, "Failed to download document for webhook base64");
                                                }
                                            }
                                        }

                                        serde_json::Value::Object(message)
                                    } else if let Some(sticker) = bg_msg.as_ref().sticker_message.as_deref() {
                                        let mut message = serde_json::Map::new();
                                        message.insert("messageType".to_string(), json!("sticker"));

                                        if let Some(url) = &sticker.url {
                                            message.insert("url".to_string(), json!(url));
                                        }
                                        if let Some(mimetype) = &sticker.mimetype {
                                            message.insert("mimetype".to_string(), json!(mimetype));
                                        }
                                        if let Some(file_length) = sticker.file_length {
                                            message.insert("fileLength".to_string(), json!(file_length));
                                        }
                                        if let Some(is_animated) = sticker.is_animated {
                                            message.insert("isAnimated".to_string(), json!(is_animated));
                                        }

                                        if base64_enabled {
                                            match bg_client.download(sticker).await {
                                                Ok(bytes) => {
                                                    let mime = sticker
                                                        .mimetype
                                                        .as_deref()
                                                        .unwrap_or("application/octet-stream");
                                                    let encoded = base64::engine::general_purpose::STANDARD
                                                        .encode(bytes);
                                                    let data_url = format!("data:{};base64,{}", mime, encoded);
                                                    message.insert("base64".to_string(), json!(data_url));
                                                }
                                                Err(e) => {
                                                    error!(error = %e, "Failed to download sticker for webhook base64");
                                                }
                                            }
                                        }

                                        serde_json::Value::Object(message)
                                    } else {
                                        json!({
                                            "messageType": "conversation",
                                            "text": bg_text.as_str()
                                        })
                                    };

                                    let mut message_item = serde_json::Map::new();
                                    let mut key_item = serde_json::Map::new();
                                    key_item.insert("remoteJid".to_string(), json!(bg_remote.as_str()));
                                    key_item.insert("fromMe".to_string(), json!(is_from_me));
                                    key_item.insert("MessageId".to_string(), json!(bg_info.id));
                                    key_item.insert(
                                        "participant".to_string(),
                                        if is_from_me {
                                            serde_json::Value::Null
                                        } else {
                                            json!(bg_sender.as_str())
                                        },
                                    );
                                    if !bg_info.push_name.is_empty() {
                                        key_item.insert(
                                            "senderName".to_string(),
                                            json!(bg_info.push_name.as_str()),
                                        );
                                    }
                                    message_item.insert(
                                        "key".to_string(),
                                        serde_json::Value::Object(key_item),
                                    );
                                    message_item.insert("message".to_string(), message_payload);

                                    chatwarp_api::server::webhooks::enqueue(
                                        &bg_state,
                                        Some(bg_instance.as_str()),
                                        "MESSAGES_UPSERT",
                                        json!({
                                            "messages": [serde_json::Value::Object(message_item)],
                                            "type": "notify"
                                        })
                                    ).await;
                                });
                            }

                            if let Some(media_ping_request) = get_pingable_media(&ctx.message) {
                                handle_media_ping(&ctx, media_ping_request).await;
                            }

                            if let Some(text) = ctx.message.text_content()
                                && text == "ping"
                            {
                                let start = std::time::Instant::now();
                                info!(chat = %ctx.info.source.chat, sender = %ctx.info.source.sender, "Received text ping, sending pong");

                                // Fire-and-forget: send reaction in background
                                {
                                    let reaction_ctx_client = ctx.client.clone();
                                    let chat_jid = ctx.info.source.chat.clone();
                                    let msg_id = ctx.info.id.clone();
                                    let from_me = ctx.info.source.is_from_me;
                                    let is_group = ctx.info.source.is_group;
                                    let sender = ctx.info.source.sender.clone();

                                    tokio::spawn(async move {
                                        let message_key = wa::MessageKey {
                                            remote_jid: Some(chat_jid.to_string()),
                                            id: Some(msg_id),
                                            from_me: Some(from_me),
                                            participant: if is_group {
                                                Some(sender.to_string())
                                            } else {
                                                None
                                            },
                                        };

                                        let reaction_message = wa::message::ReactionMessage {
                                            key: Some(message_key),
                                            text: Some("🏓".to_string()),
                                            sender_timestamp_ms: Some(Utc::now().timestamp_millis()),
                                            ..Default::default()
                                        };

                                        let final_message_to_send = wa::Message {
                                            reaction_message: Some(reaction_message),
                                            ..Default::default()
                                        };

                                        if let Err(e) = reaction_ctx_client.send_message(chat_jid, final_message_to_send).await {
                                            error!(error = %e, "Failed to send reaction");
                                        }
                                    });
                                }

                                // Determine participant JID for quoting
                                let participant_jid = if ctx.info.source.is_from_me {
                                    ctx.client.get_pn().await.unwrap_or_default().to_string()
                                } else {
                                    ctx.info.source.sender.to_string()
                                };

                                let context_info = wa::ContextInfo {
                                    stanza_id: Some(ctx.info.id.clone()),
                                    participant: Some(participant_jid),
                                    quoted_message: Some(ctx.message.clone()),
                                    ..Default::default()
                                };

                                let duration = start.elapsed();
                                let duration_str = format!("{:.2?}", duration);

                                // Single message with timing included — no edit needed
                                let reply_message = wa::Message {
                                    extended_text_message: Some(Box::new(
                                        wa::message::ExtendedTextMessage {
                                            text: Some(format!("🏓 Pong!\n`{}`", duration_str)),
                                            context_info: Some(Box::new(context_info)),
                                            ..Default::default()
                                        },
                                    )),
                                    ..Default::default()
                                };

                                match ctx.send_message(reply_message).await {
                                    Ok(id) => {
                                        let total = start.elapsed();
                                        info!(elapsed = ?total, message_id = %id, "Pong sent (single message, no edit)");
                                    }
                                    Err(e) => {
                                        error!(error = %e, "Failed to send pong message");
                                    }
                                }
                            }
                        }
                        Event::Connected(_) => {
                            info!("Bot connected successfully");
                            if let Some(instance) = state.instances.get(&instance_name) {
                                *instance.qr_code.write().await = None;
                                *instance.connection_state.write().await = "connected".to_string();
                            }
                            chatwarp_api::server::webhooks::enqueue(
                                &state,
                                Some(&instance_name),
                                "CONNECTION_UPDATE",
                                json!({ "action": "update", "state": "open" })
                            ).await;
                            // Pre-warm E2E sessions for recent DM chats in the background.
                            // This eliminates the ~20-30s first-message latency for known contacts.
                            tokio::spawn(chatwarp_api::server::messages_worker::warm_sessions(
                                state.clone(),
                                instance_name.clone(),
                            ));
                        }
                        Event::Receipt(receipt) => {
                            info!(message_ids = ?receipt.message_ids, receipt_type = ?receipt.r#type, "Received receipt");
                        }
                        Event::ChatPresence(presence) => {
                            let chat_id = presence.source.chat.to_string();
                            let sender = presence.source.sender.to_string();
                            let presence_state = match presence.state {
                                warp_core::types::presence::ChatPresence::Composing => "composing",
                                warp_core::types::presence::ChatPresence::Paused => "paused",
                            };
                            let media = match presence.media {
                                warp_core::types::presence::ChatPresenceMedia::Audio => "audio",
                                warp_core::types::presence::ChatPresenceMedia::Text => "",
                            };

                            let payload = json!({
                                "chatId": chat_id,
                                "sender": sender,
                                "state": presence_state,
                                "media": media,
                                "isGroup": presence.source.is_group,
                                "timestamp": chrono::Utc::now().timestamp_millis(),
                            });

                            chatwarp_api::server::webhooks::enqueue(
                                &state,
                                Some(&instance_name),
                                "CHAT_PRESENCE",
                                payload.clone(),
                            )
                            .await;

                            state
                                .api_store
                                .execute(
                                    "INSERT INTO api_events (session, event, payload, created_at) \
                                     VALUES ($1, $2, $3, now())",
                                    vec![
                                        chatwarp_api::api_store::ApiBind::Text(
                                            instance_name.clone(),
                                        ),
                                        chatwarp_api::api_store::ApiBind::Text(
                                            "CHAT_PRESENCE".to_string(),
                                        ),
                                        chatwarp_api::api_store::ApiBind::Json(payload),
                                    ],
                                )
                                .await
                                .ok();
                        }
                        Event::LoggedOut(_) => {
                            error!("Bot was logged out");
                            if let Some(instance) = state.instances.get(&instance_name) {
                                *instance.connection_state.write().await =
                                    "disconnected".to_string();
                            }
                            chatwarp_api::server::webhooks::enqueue(
                                &state,
                                Some(&instance_name),
                                "CONNECTION_UPDATE",
                                json!({ "action": "update", "state": "close", "reason": "loggedOut" })
                            ).await;
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

        app_state
            .clients
            .insert(default_instance_name.clone(), bot.client());
        tokio::spawn(chatwarp_api::server::messages_worker::spawn_messages_worker(
            app_state.clone(),
            message_notify_rx,
        ));

        let bot_handle = match bot.run().await {
            Ok(handle) => handle,
            Err(e) => {
                error!(error = %e, "Bot failed to start");
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

        info!(address = %addr, "HTTP server listening");
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
    info!(media_type = ?media.media_type(), sender = %ctx.info.source.sender, "Received media ping");

    let mut data_buffer = Cursor::new(Vec::new());
    if let Err(e) = ctx.client.download_to_file(media, &mut data_buffer).await {
        error!(error = %e, "Failed to download media");
        let _ = ctx
            .send_message(wa::Message {
                conversation: Some("Failed to download your media.".to_string()),
                ..Default::default()
            })
            .await;
        return;
    }

    info!(
        bytes = data_buffer.get_ref().len(),
        "Media downloaded successfully, uploading"
    );
    let plaintext_data = data_buffer.into_inner();
    let upload_response = match ctx.client.upload(plaintext_data, media.media_type()).await {
        Ok(resp) => resp,
        Err(e) => {
            error!(error = %e, "Failed to upload media");
            let _ = ctx
                .send_message(wa::Message {
                    conversation: Some("Failed to re-upload the media.".to_string()),
                    ..Default::default()
                })
                .await;
            return;
        }
    };

    info!("Media uploaded successfully, constructing reply message");
    let reply_msg = media.build_pong_reply(upload_response);

    if let Err(e) = ctx.send_message(reply_msg).await {
        error!(error = %e, "Failed to send media pong reply");
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
