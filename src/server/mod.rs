use crate::api_store::ApiStore;
use axum::{
    Router,
    extract::{Form, State},
    http::{StatusCode, header},
    middleware,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use image::Luma;
use qrcode::QrCode;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, sync::Arc};
use tokio::sync::{RwLock, mpsc};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;

pub mod handlers;
pub mod messages_worker;
pub mod routes;
pub mod webhooks;
pub mod queue;

pub struct AppState {
    pub instances: DashMap<String, InstanceState>,
    pub sessions_runtime: DashMap<String, SessionRuntime>,
    pub api_store: Arc<dyn ApiStore>,
    pub clients: DashMap<String, Arc<crate::client::Client>>,
    pub settings: Arc<RwLock<Settings>>,
    pub api_password_hash: Option<[u8; 32]>,
    pub session_ttl_seconds: u64,
    pub message_notify: mpsc::Sender<()>,
}

#[derive(Clone, Debug, Default)]
pub struct Settings {
    pub webhook_events: std::collections::HashMap<String, bool>,
    pub allowed_events: Option<HashSet<String>>,
}

impl Settings {
    pub fn new() -> Self {
        let mut webhook_events = std::collections::HashMap::new();
        let allowed_events = std::env::var("ALLOWED_EVENTS")
            .ok()
            .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
            .map(|items| items.into_iter().collect::<HashSet<_>>());
        // pre-load from env
        for (key, val) in std::env::vars() {
            if let Some(event) = key.strip_prefix("WEBHOOK_EVENTS_") {
                let enabled = val == "true" || val == "1";
                webhook_events.insert(event.to_string(), enabled);
            }
        }
        Self {
            webhook_events,
            allowed_events,
        }
    }

    pub fn is_event_enabled(&self, event: &str) -> bool {
        if let Some(allowed) = &self.allowed_events {
            if !allowed.contains(event) {
                return false;
            }
        }
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
    let router = Router::<Arc<AppState>>::new()
        .merge(routes::router())
        .route("/", get(root_handler))
        .route("/auth/login", get(login_page).post(login_handler))
        .route("/auth/logout", post(logout_handler))
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
        .with_state(state.clone());

    let router = if state.api_password_hash.is_some() {
        router.layer(middleware::from_fn_with_state(state, auth_middleware))
    } else {
        router
    };

    router.layer(
        TraceLayer::new_for_http()
            .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
            .on_response(DefaultOnResponse::new().level(Level::INFO)),
    )
}

async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    req: axum::http::Request<axum::body::Body>,
    next: middleware::Next,
) -> Response {
    let Some(expected_hash) = state.api_password_hash else {
        return next.run(req).await;
    };

    let path = req.uri().path();
    if path == "/auth/login"
        || path == "/auth/logout"
        || path == "/healthz"
        || path == "/readyz"
        || path == "/health"
        || path == "/ping"
        || path == "/metrics"
        || path == "/openapi.json"
        || path == "/docs/openapi.json"
        || path == "/swagger"
        || path == "/docs/swagger"
    {
        return next.run(req).await;
    }

    let headers = req.headers();
    if let Some(cookie) = get_cookie(headers, "chatwarp_auth") {
        if let Some(cookie_hash) = parse_hex_32(&cookie) {
            if constant_time_eq_bytes(&cookie_hash, &expected_hash) {
                return next.run(req).await;
            }
        }
    }
    let header_password = headers
        .get("x-chatwarp-password")
        .and_then(|v| v.to_str().ok());
    let bearer_password = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let provided = header_password.or(bearer_password);
    let authorized = provided
        .map(|p| hash_password(p))
        .map(|h| constant_time_eq_bytes(&h, &expected_hash))
        .unwrap_or(false);

    if authorized {
        next.run(req).await
    } else {
        (StatusCode::UNAUTHORIZED, Html(login_html())).into_response()
    }
}

fn constant_time_eq_bytes(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn hash_password(value: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let result = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result[..]);
    out
}

fn parse_hex_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        let idx = i * 2;
        let byte = u8::from_str_radix(&value[idx..idx + 2], 16).ok()?;
        out[i] = byte;
    }
    Some(out)
}

fn get_cookie(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in cookie.split(';') {
        let mut iter = part.trim().splitn(2, '=');
        let key = iter.next()?.trim();
        let value = iter.next()?.trim();
        if key == name {
            return Some(value.to_string());
        }
    }
    None
}

#[derive(serde::Deserialize)]
struct LoginForm {
    password: String,
}

async fn login_page() -> impl IntoResponse {
    Html(login_html())
}

async fn login_handler(
    State(state): State<Arc<AppState>>,
    Form(payload): Form<LoginForm>,
) -> impl IntoResponse {
    let Some(expected_hash) = state.api_password_hash else {
        return (StatusCode::OK, "ok").into_response();
    };

    let provided_hash = hash_password(&payload.password);
    if constant_time_eq_bytes(&provided_hash, &expected_hash) {
        let token = hex_32(&expected_hash);
        let cookie = format!(
            "chatwarp_auth={}; Max-Age={}; HttpOnly; SameSite=Lax; Path=/",
            token, state.session_ttl_seconds
        );
        let mut response = Html(login_success_html()).into_response();
        response
            .headers_mut()
            .insert(header::SET_COOKIE, cookie.parse().unwrap());
        response
    } else {
        (StatusCode::UNAUTHORIZED, Html(login_html_with_error())).into_response()
    }
}

async fn logout_handler() -> impl IntoResponse {
    let cookie = "chatwarp_auth=; Max-Age=0; HttpOnly; SameSite=Lax; Path=/";
    let mut response = (StatusCode::OK, "ok").into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, cookie.parse().unwrap());
    response
}

fn hex_32(value: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for b in value {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn login_html() -> String {
    r#"
<!DOCTYPE html>
<html>
<head>
  <title>ChatWarp API Login</title>
  <style>
    body { font-family: sans-serif; background: #f0f2f5; display: flex; align-items: center; justify-content: center; min-height: 100vh; margin: 0; }
    .card { background: white; padding: 2rem; border-radius: 8px; box-shadow: 0 2px 6px rgba(0,0,0,0.15); width: 320px; }
    h1 { font-size: 1.2rem; margin: 0 0 1rem 0; color: #128c7e; }
    label { display: block; font-size: 0.9rem; margin-bottom: 0.5rem; color: #555; }
    input { width: 100%; padding: 0.6rem; border: 1px solid #ddd; border-radius: 6px; margin-bottom: 1rem; }
    button { width: 100%; padding: 0.6rem; border: none; border-radius: 6px; background: #128c7e; color: white; font-weight: 600; cursor: pointer; }
    small { display: block; margin-top: 0.75rem; color: #888; }
  </style>
</head>
<body>
  <form class="card" method="post" action="/auth/login">
    <h1>ChatWarp API</h1>
    <label for="password">Senha</label>
    <input id="password" name="password" type="password" placeholder="Digite a senha" required />
    <button type="submit">Entrar</button>
    <small>Sua sessao expira automaticamente.</small>
  </form>
</body>
</html>
"#
    .to_string()
}

fn login_html_with_error() -> String {
    r#"
<!DOCTYPE html>
<html>
<head>
  <title>ChatWarp API Login</title>
  <style>
    body { font-family: sans-serif; background: #f0f2f5; display: flex; align-items: center; justify-content: center; min-height: 100vh; margin: 0; }
    .card { background: white; padding: 2rem; border-radius: 8px; box-shadow: 0 2px 6px rgba(0,0,0,0.15); width: 320px; }
    h1 { font-size: 1.2rem; margin: 0 0 0.5rem 0; color: #128c7e; }
    p { color: #c0392b; margin: 0 0 1rem 0; font-size: 0.9rem; }
    label { display: block; font-size: 0.9rem; margin-bottom: 0.5rem; color: #555; }
    input { width: 100%; padding: 0.6rem; border: 1px solid #ddd; border-radius: 6px; margin-bottom: 1rem; }
    button { width: 100%; padding: 0.6rem; border: none; border-radius: 6px; background: #128c7e; color: white; font-weight: 600; cursor: pointer; }
  </style>
</head>
<body>
  <form class="card" method="post" action="/auth/login">
    <h1>ChatWarp API</h1>
    <p>Senha incorreta.</p>
    <label for="password">Senha</label>
    <input id="password" name="password" type="password" placeholder="Digite a senha" required />
    <button type="submit">Entrar</button>
  </form>
</body>
</html>
"#
    .to_string()
}

fn login_success_html() -> String {
    r#"
<!DOCTYPE html>
<html>
<head>
  <title>ChatWarp API</title>
  <meta http-equiv="refresh" content="1; url=/" />
  <style>
    body { font-family: sans-serif; background: #f0f2f5; display: flex; align-items: center; justify-content: center; min-height: 100vh; margin: 0; }
    .card { background: white; padding: 2rem; border-radius: 8px; box-shadow: 0 2px 6px rgba(0,0,0,0.15); width: 320px; text-align: center; }
    h1 { font-size: 1.1rem; margin: 0 0 0.5rem 0; color: #128c7e; }
  </style>
</head>
<body>
  <div class="card">
    <h1>Autenticado</h1>
    <p>Redirecionando...</p>
  </div>
</body>
</html>
"#
    .to_string()
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

                // Attempt to logout when the page is closed.
                window.addEventListener('pagehide', () => {{
                    try {{
                        const data = new Blob([], {{ type: 'application/x-www-form-urlencoded' }});
                        navigator.sendBeacon('/auth/logout', data);
                    }} catch (_) {{}}
                }});
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
