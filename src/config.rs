use std::{net::SocketAddr, str::FromStr};

use thiserror::Error;

/// Runtime configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    /// Socket address for binding the HTTP server.
    pub bind_addr: SocketAddr,
}

impl Config {
    /// Loads runtime configuration using environment variables.
    pub fn from_env() -> Result<Self, ConfigError> {
        let _ = dotenvy::dotenv();

        let port = match std::env::var("SERVER_PORT") {
            Ok(raw) => u16::from_str(&raw).map_err(|_| ConfigError::InvalidPort(raw))?,
            Err(_) => 8080,
        };

        Ok(Self {
            bind_addr: SocketAddr::from(([0, 0, 0, 0], port)),
        })
    }
}

/// Errors while loading runtime configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid SERVER_PORT value: {0}")]
    InvalidPort(String),
}
