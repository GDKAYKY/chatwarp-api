use std::{net::SocketAddr, str::FromStr};

use thiserror::Error;

/// Runtime configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    /// Socket address for binding the HTTP server.
    pub bind_addr: SocketAddr,
    /// Max wait for connect route to receive QR event.
    pub instance_connect_wait_ms: u64,
    /// Max HTTP request body size in KiB.
    pub server_body_limit_kb: usize,
    /// Database URL used by runtime auth persistence.
    pub database_url: String,
    /// Websocket endpoint used for WA transport.
    pub wa_ws_url: String,
}

impl Config {
    /// Loads runtime configuration using environment variables.
    pub fn from_env() -> Result<Self, ConfigError> {
        let _ = dotenvy::dotenv();

        let port = match std::env::var("SERVER_PORT") {
            Ok(raw) => u16::from_str(&raw).map_err(|_| ConfigError::InvalidPort(raw))?,
            Err(_) => 8080,
        };

        let instance_connect_wait_ms = match std::env::var("INSTANCE_CONNECT_WAIT_MS") {
            Ok(raw) => u64::from_str(&raw).map_err(|_| ConfigError::InvalidConnectWait(raw))?,
            Err(_) => 300,
        };

        let server_body_limit_kb = match std::env::var("SERVER_BODY_LIMIT_KB") {
            Ok(raw) => usize::from_str(&raw).map_err(|_| ConfigError::InvalidBodyLimit(raw))?,
            Err(_) => 256,
        };

        let database_url = std::env::var("DATABASE_URL").map_err(|_| ConfigError::MissingDatabaseUrl)?;
        let wa_ws_url = std::env::var("WA_WS_URL")
            .unwrap_or_else(|_| "wss://web.whatsapp.com/ws/chat".to_owned());

        Ok(Self {
            bind_addr: SocketAddr::from(([0, 0, 0, 0], port)),
            instance_connect_wait_ms,
            server_body_limit_kb,
            database_url,
            wa_ws_url,
        })
    }
}

/// Errors while loading runtime configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid SERVER_PORT value: {0}")]
    InvalidPort(String),
    #[error("invalid INSTANCE_CONNECT_WAIT_MS value: {0}")]
    InvalidConnectWait(String),
    #[error("invalid SERVER_BODY_LIMIT_KB value: {0}")]
    InvalidBodyLimit(String),
    #[error("missing DATABASE_URL environment variable")]
    MissingDatabaseUrl,
}
