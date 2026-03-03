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
            <title>ChatWarp QR</title>
            <style>
                body {{ font-family: sans-serif; display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100vh; margin: 0; background: #f0f2f5; }}
                .container {{ background: white; padding: 2rem; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); text-align: center; }}
                h1 {{ color: #128c7e; }}
            </style>
        </head>
        <body>
            <div class="container">
                <h1>ChatWarp API</h1>
                {}
            </div>
            <script>
                // Auto refresh every 10 seconds to check for new QR
                setTimeout(() => location.reload(), 10000);
            </script>
        </body>
        </html>
        "#,
        qr_html
    ))
}

async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "{\"ok\": true}")
}

async fn ready_handler() -> impl IntoResponse {
    (StatusCode::OK, "{\"ok\": true}")
}
