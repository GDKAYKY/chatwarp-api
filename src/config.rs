use std::{net::SocketAddr, str::FromStr};

use thiserror::Error;
use url::Url;

/// Runtime protocol mode used by the WA transport stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaProtocolMode {
    /// Select mode automatically based on websocket host.
    Auto,
    /// Force real WhatsApp MD protocol.
    RealMd,
    /// Force local synthetic protocol used by tests/mocks.
    Synthetic,
}

impl WaProtocolMode {
    /// Resolves automatic mode using the websocket URL host.
    pub fn resolve_for_url(self, wa_ws_url: &str) -> WaProtocolMode {
        match self {
            WaProtocolMode::Auto => {
                let host = Url::parse(wa_ws_url)
                    .ok()
                    .and_then(|parsed| parsed.host_str().map(str::to_owned))
                    .unwrap_or_default();

                if host.eq_ignore_ascii_case("web.whatsapp.com") {
                    WaProtocolMode::RealMd
                } else {
                    WaProtocolMode::Synthetic
                }
            }
            explicit => explicit,
        }
    }
}

/// Runtime configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    /// Socket address for binding the HTTP server.
    pub bind_addr: SocketAddr,
    /// Max HTTP request body size in KiB.
    pub server_body_limit_kb: usize,
    /// Database URL used by runtime auth persistence.
    pub database_url: String,
    /// Websocket endpoint used for WA transport.
    pub wa_ws_url: String,
    /// WA protocol mode selection policy.
    pub wa_protocol_mode: WaProtocolMode,
}

impl Config {
    /// Loads runtime configuration using environment variables.
    pub fn from_env() -> Result<Self, ConfigError> {
        let _ = dotenvy::dotenv();

        let port = match std::env::var("SERVER_PORT") {
            Ok(raw) => u16::from_str(&raw).map_err(|_| ConfigError::InvalidPort(raw))?,
            Err(_) => 8080,
        };

        let server_body_limit_kb = match std::env::var("SERVER_BODY_LIMIT_KB") {
            Ok(raw) => usize::from_str(&raw).map_err(|_| ConfigError::InvalidBodyLimit(raw))?,
            Err(_) => 256,
        };

        let database_url = std::env::var("DATABASE_URL").map_err(|_| ConfigError::MissingDatabaseUrl)?;
        let wa_ws_url = std::env::var("WA_WS_URL")
            .unwrap_or_else(|_| "wss://web.whatsapp.com/ws/chat".to_owned());
        let wa_protocol_mode = match std::env::var("WA_PROTOCOL_MODE") {
            Ok(raw) => parse_protocol_mode(&raw)?,
            Err(_) => WaProtocolMode::Auto,
        };

        Ok(Self {
            bind_addr: SocketAddr::from(([0, 0, 0, 0], port)),
            server_body_limit_kb,
            database_url,
            wa_ws_url,
            wa_protocol_mode,
        })
    }
}

fn parse_protocol_mode(raw: &str) -> Result<WaProtocolMode, ConfigError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "auto" => Ok(WaProtocolMode::Auto),
        "realmd" | "real_md" | "real" => Ok(WaProtocolMode::RealMd),
        "synthetic" | "mock" => Ok(WaProtocolMode::Synthetic),
        _ => Err(ConfigError::InvalidProtocolMode(raw.to_owned())),
    }
}

/// Errors while loading runtime configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid SERVER_PORT value: {0}")]
    InvalidPort(String),
    #[error("invalid SERVER_BODY_LIMIT_KB value: {0}")]
    InvalidBodyLimit(String),
    #[error("invalid WA_PROTOCOL_MODE value: {0}")]
    InvalidProtocolMode(String),
    #[error("missing DATABASE_URL environment variable")]
    MissingDatabaseUrl,
}
