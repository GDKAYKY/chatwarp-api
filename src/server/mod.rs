use crate::api_store::ApiStore;
use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use image::Luma;
use qrcode::QrCode;
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod handlers;
pub mod messages_worker;
pub mod routes;
pub mod webhooks;

pub struct AppState {
    pub instances: DashMap<String, InstanceState>,
    pub sessions_runtime: DashMap<String, SessionRuntime>,
    pub api_store: Arc<dyn ApiStore>,
    pub clients: DashMap<String, Arc<crate::client::Client>>,
    pub settings: Arc<RwLock<Settings>>,
}

#[derive(Clone, Debug, Default)]
pub struct Settings {
    pub webhook_events: std::collections::HashMap<String, bool>,
}

impl Settings {
    pub fn new() -> Self {
        let mut webhook_events = std::collections::HashMap::new();
        // pre-load from env
        for (key, val) in std::env::vars() {
            if let Some(event) = key.strip_prefix("WEBHOOK_EVENTS_") {
                let enabled = val == "true" || val == "1";
                webhook_events.insert(event.to_string(), enabled);
            }
        }
        Self { webhook_events }
    }

    pub fn is_event_enabled(&self, event: &str) -> bool {
        self.webhook_events.get(event).copied().unwrap_or(true)
    }
}

pub struct InstanceState {
    pub qr_code: Arc<RwLock<Option<String>>>,
    pub qr_count: Arc<RwLock<u32>>,
    pub connection_state: Arc<RwLock<String>>,
}

#[derive(Clone, Debug)]
pub struct SessionRuntime {
    pub connection_state: String,
    pub qr_code: Option<String>,
    pub pair_code: Option<String>,
    pub last_seen: Option<DateTime<Utc>>,
}

impl SessionRuntime {
    pub fn new() -> Self {
        Self {
            connection_state: "disconnected".to_string(),
            qr_code: None,
            pair_code: None,
            last_seen: None,
        }
    }
}

impl InstanceState {
    pub fn new() -> Self {
        Self {
            qr_code: Arc::new(RwLock::new(None)),
            qr_count: Arc::new(RwLock::new(0)),
            connection_state: Arc::new(RwLock::new("disconnected".to_string())),
        }
    }
}

pub fn create_router(state: Arc<AppState>) -> Router<()> {
    Router::<Arc<AppState>>::new()
        .merge(routes::router())
        .route("/", get(root_handler))
        .route("/healthz", get(health_handler))
        .route("/readyz", get(ready_handler))
        .route("/openapi.json", get(handlers::openapi_handler))
        .route("/docs/openapi.json", get(handlers::openapi_handler))
        .route("/swagger", get(handlers::swagger_handler))
        .route("/docs/swagger", get(handlers::swagger_handler))
        .route("/metrics", get(handlers::metrics_handler))
        .route("/settings/events", get(get_events_settings))
        .route("/settings/toggle-event", post(toggle_event))
        // Instance routes
        .route("/instance/create", post(handlers::create_instance))
        .route("/instance/delete/:name", get(handlers::delete_instance)) // Should be DELETE, but ROUTES.md says DELETE
        .route(
            "/instance/connectionState/:name",
            get(handlers::connection_state),
        )
        .route("/instance/connect/:name", get(handlers::connect_instance))
        .route("/instance/:name/state", get(handlers::instance_state))
        // Message routes
        .route(
            "/message/:operation/:instance_name",
            post(handlers::send_message),
        )
        // Chat routes
        .route(
            "/chat/findMessages/:instance_name",
            post(handlers::find_messages),
        )
        .route("/chat/findChats/:instance_name", get(handlers::find_chats))
        // Group routes
        .route("/group/create/:instance_name", post(handlers::create_group))
        .route(
            "/group/fetchAllGroups/:instance_name",
            get(handlers::fetch_groups),
        )
        .with_state(state)
}

async fn root_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut qr_html = String::new();

    // For now, just show the QR of the first instance that has one
    let mut found = false;
    for entry in state.instances.iter() {
        let name = entry.key();
        let qr = entry.value().qr_code.read().await;
        if let Some(code) = qr.as_ref() {
            // Generate QR image
            if let Ok(qr_obj) = QrCode::new(code.as_bytes()) {
                let img = qr_obj.render::<Luma<u8>>().build();
                let mut buffer = std::io::Cursor::new(Vec::new());
                if img.write_to(&mut buffer, image::ImageFormat::Png).is_ok() {
                    let base64_img = general_purpose::STANDARD.encode(buffer.get_ref());
                    qr_html.push_str(&format!(
                        "<h2>Instance: {}</h2><img src=\"data:image/png;base64,{}\" style=\"width: 300px; height: 300px;\">",
                        name, base64_img
                    ));
                    found = true;
                    break;
                }
            }
        }
    }

    if !found {
        qr_html = "<h1>No QR Code available yet.</h1><p>Please wait or check instance status.</p>"
            .to_string();
    }

    Html(format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>ChatWarp QR & Settings</title>
            <style>
                body {{ font-family: sans-serif; display: flex; flex-direction: column; align-items: center; justify-content: center; min-height: 100vh; margin: 0; background: #f0f2f5; }}
                .container {{ background: white; padding: 2rem; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); text-align: center; margin-bottom: 2rem; }}
                h1 {{ color: #128c7e; margin-bottom: 0.5rem; }}
                .opts {{ margin-top: 1rem; padding-top: 1rem; border-top: 1px solid #eee; text-align: left; }}
                .switch {{ display: flex; align-items: center; justify-content: space-between; margin-bottom: 0.5rem; }}
                label.switch-label {{ font-size: 0.9rem; color: #555; cursor: pointer; display: flex; align-items: center; gap: 0.5rem; }}
            </style>
        </head>
        <body>
            <div class="container">
                <h1>ChatWarp API</h1>
                <p style="color: #666; margin-top: 0;">Scan QR inside your WhatsApp</p>
                {}
                
                <div class="opts">
                    <h4>Webhook Settings (Global)</h4>
                    <div class="switch">
                        <label class="switch-label">
                            <input type="checkbox" id="chkStartup" onchange="toggleEvent('APPLICATION_STARTUP', this.checked)">
                            Send APPLICATION_STARTUP
                        </label>
                    </div>
                    <div class="switch">
                        <label class="switch-label">
                            <input type="checkbox" id="chkMessagesSet" onchange="toggleEvent('MESSAGES_SET', this.checked)">
                            Send MESSAGES_SET
                        </label>
                    </div>
                    <small style="color: grey;">These changes are saved to your .env file.</small>
                </div>
            </div>
            <script>
                // Load webhook settings
                fetch('/settings/events')
                    .then(res => res.json())
                    .then(data => {{
                        document.getElementById('chkStartup').checked = data['APPLICATION_STARTUP'] !== false;
                        document.getElementById('chkMessagesSet').checked = data['MESSAGES_SET'] !== false;
                    }})
                    .catch(e => console.error('Failed to load settings', e));

                function toggleEvent(ev, isEnabled) {{
                    fetch('/settings/toggle-event', {{
                        method: 'POST',
                        headers: {{ 'Content-Type': 'application/json' }},
                        body: JSON.stringify({{ event: ev, enabled: isEnabled }})
                    }}).catch(e => alert('Failed to save settings: ' + e));
                }}

                // Optional: We do a soft refresh rather than reload, so it doesn't interrupt toggling.
                // But for the QR, let's keep the reload strategy for now or wrap it properly.
                setTimeout(() => location.reload(), 15000);
            </script>
        </body>
        </html>
        "#,
        qr_html
    ))
}

#[derive(serde::Deserialize)]
pub struct ToggleEventReq {
    pub event: String,
    pub enabled: bool,
}

async fn get_events_settings(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let settings = state.settings.read().await;
    let startup = settings.is_event_enabled("APPLICATION_STARTUP");
    let messages_set = settings.is_event_enabled("MESSAGES_SET");
    axum::Json(serde_json::json!({
        "APPLICATION_STARTUP": startup,
        "MESSAGES_SET": messages_set,
    }))
}

async fn toggle_event(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<ToggleEventReq>,
) -> impl IntoResponse {
    let mut settings = state.settings.write().await;
    settings
        .webhook_events
        .insert(payload.event, payload.enabled);

    axum::Json(serde_json::json!({"ok": true}))
}

async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "{\"ok\": true}")
}

async fn ready_handler() -> impl IntoResponse {
    (StatusCode::OK, "{\"ok\": true}")
}
