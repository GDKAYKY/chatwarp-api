use std::{collections::HashSet, env, net::IpAddr};

use serde::{Deserialize, Serialize};

use crate::errors::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub cors: CorsConfig,
    pub ssl_conf: SslConfig,
    pub provider: ProviderConfig,
    pub database: DatabaseConfig,
    pub webhook: WebhookConfig,
    pub authentication: AuthenticationConfig,
    pub metrics: MetricsConfig,
    pub sentry: SentryConfig,
    pub websocket: WebsocketConfig,
    pub rabbitmq: RabbitmqConfig,
    pub sidecar: SidecarConfig,
    pub facebook: FacebookConfig,
    pub telemetry: TelemetryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub name: String,
    pub kind: String,
    pub port: u16,
    pub url: String,
    pub disable_docs: bool,
    pub disable_manager: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    pub origins: Vec<String>,
    pub methods: Vec<String>,
    pub credentials: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslConfig {
    pub privkey: String,
    pub fullchain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub enabled: bool,
    pub host: String,
    pub port: String,
    pub prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveDataConfig {
    pub instance: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub connection_uri: String,
    pub client_name: String,
    pub provider: String,
    pub save_data: SaveDataConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebhookEvents {
    pub errors: bool,
    pub errors_webhook: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    pub events: WebhookEvents,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationConfig {
    pub api_key: String,
    pub expose_in_fetch_instances: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub auth_required: bool,
    pub user: Option<String>,
    pub password: Option<String>,
    pub allowed_ips: Option<HashSet<IpAddr>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentryConfig {
    pub dsn: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebsocketConfig {
    pub enabled: bool,
    pub global_events: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RabbitmqConfig {
    pub enabled: bool,
    pub global_enabled: bool,
    pub uri: String,
    pub exchange_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarConfig {
    pub endpoint: String,
    pub connect_timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacebookConfig {
    pub app_id: Option<String>,
    pub config_id: Option<String>,
    pub user_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    pub enabled: bool,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, AppError> {
        let server_port = parse_u16("SERVER_PORT", 8080);
        let server_kind = var("SERVER_TYPE", "http");
        let server_kind = if server_kind.is_empty() {
            "http".to_string()
        } else {
            server_kind
        };

        let database_uri = var("DATABASE_CONNECTION_URI", "");
        if database_uri.is_empty() {
            return Err(AppError::Config(
                "DATABASE_CONNECTION_URI is required for Rust runtime".to_string(),
            ));
        }

        Ok(Self {
            server: ServerConfig {
                name: var("SERVER_NAME", "evolution"),
                kind: server_kind,
                port: server_port,
                url: var("SERVER_URL", ""),
                disable_docs: bool_var("SERVER_DISABLE_DOCS"),
                disable_manager: bool_var("SERVER_DISABLE_MANAGER"),
            },
            cors: CorsConfig {
                origins: split_csv("CORS_ORIGIN", vec!["*".to_string()]),
                methods: split_csv(
                    "CORS_METHODS",
                    vec![
                        "POST".to_string(),
                        "GET".to_string(),
                        "PUT".to_string(),
                        "DELETE".to_string(),
                    ],
                ),
                credentials: bool_var("CORS_CREDENTIALS"),
            },
            ssl_conf: SslConfig {
                privkey: var("SSL_CONF_PRIVKEY", ""),
                fullchain: var("SSL_CONF_FULLCHAIN", ""),
            },
            provider: ProviderConfig {
                enabled: bool_var("PROVIDER_ENABLED"),
                host: var("PROVIDER_HOST", ""),
                port: var("PROVIDER_PORT", "5656"),
                prefix: var("PROVIDER_PREFIX", "evolution"),
            },
            database: DatabaseConfig {
                connection_uri: database_uri,
                client_name: var("DATABASE_CONNECTION_CLIENT_NAME", "evolution"),
                provider: var("DATABASE_PROVIDER", "postgresql"),
                save_data: SaveDataConfig {
                    instance: bool_var("DATABASE_SAVE_DATA_INSTANCE"),
                },
            },
            webhook: WebhookConfig {
                events: WebhookEvents {
                    errors: bool_var("WEBHOOK_EVENTS_ERRORS"),
                    errors_webhook: var("WEBHOOK_EVENTS_ERRORS_WEBHOOK", ""),
                },
            },
            authentication: AuthenticationConfig {
                api_key: var("AUTHENTICATION_API_KEY", "BQYHJGJHJ"),
                expose_in_fetch_instances: bool_var("AUTHENTICATION_EXPOSE_IN_FETCH_INSTANCES"),
            },
            metrics: MetricsConfig {
                enabled: bool_var("PROMETHEUS_METRICS"),
                auth_required: bool_var("METRICS_AUTH_REQUIRED"),
                user: optional_var("METRICS_USER"),
                password: optional_var("METRICS_PASSWORD"),
                allowed_ips: parse_allowed_ips(optional_var("METRICS_ALLOWED_IPS")),
            },
            sentry: SentryConfig {
                dsn: optional_var("SENTRY_DSN"),
            },
            websocket: WebsocketConfig {
                enabled: bool_var("WEBSOCKET_ENABLED"),
                global_events: bool_var("WEBSOCKET_GLOBAL_EVENTS"),
            },
            rabbitmq: RabbitmqConfig {
                enabled: bool_var("RABBITMQ_ENABLED"),
                global_enabled: bool_var("RABBITMQ_GLOBAL_ENABLED"),
                uri: var("RABBITMQ_URI", ""),
                exchange_name: var("RABBITMQ_EXCHANGE_NAME", "evolution_exchange"),
            },
            sidecar: SidecarConfig {
                endpoint: var("SIDECAR_GRPC_ENDPOINT", "http://127.0.0.1:50051"),
                connect_timeout_ms: parse_u64("SIDECAR_CONNECT_TIMEOUT_MS", 3000),
            },
            facebook: FacebookConfig {
                app_id: optional_var("FACEBOOK_APP_ID"),
                config_id: optional_var("FACEBOOK_CONFIG_ID"),
                user_token: optional_var("FACEBOOK_USER_TOKEN"),
            },
            telemetry: TelemetryConfig {
                enabled: env::var("TELEMETRY_ENABLED")
                    .map(|v| v == "true")
                    .unwrap_or(true),
            },
        })
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                name: "evolution".to_string(),
                kind: "http".to_string(),
                port: 8080,
                url: "http://localhost:8080".to_string(),
                disable_docs: false,
                disable_manager: false,
            },
            cors: CorsConfig {
                origins: vec!["*".to_string()],
                methods: vec![
                    "POST".to_string(),
                    "GET".to_string(),
                    "PUT".to_string(),
                    "DELETE".to_string(),
                ],
                credentials: false,
            },
            ssl_conf: SslConfig {
                privkey: String::new(),
                fullchain: String::new(),
            },
            provider: ProviderConfig {
                enabled: false,
                host: String::new(),
                port: "5656".to_string(),
                prefix: "evolution".to_string(),
            },
            database: DatabaseConfig {
                connection_uri: "postgres://evolution:evolution@localhost/evolution".to_string(),
                client_name: "evolution".to_string(),
                provider: "postgresql".to_string(),
                save_data: SaveDataConfig { instance: true },
            },
            webhook: WebhookConfig {
                events: WebhookEvents::default(),
            },
            authentication: AuthenticationConfig {
                api_key: "BQYHJGJHJ".to_string(),
                expose_in_fetch_instances: false,
            },
            metrics: MetricsConfig {
                enabled: false,
                auth_required: false,
                user: None,
                password: None,
                allowed_ips: None,
            },
            sentry: SentryConfig { dsn: None },
            websocket: WebsocketConfig {
                enabled: false,
                global_events: false,
            },
            rabbitmq: RabbitmqConfig {
                enabled: false,
                global_enabled: false,
                uri: String::new(),
                exchange_name: "evolution_exchange".to_string(),
            },
            sidecar: SidecarConfig {
                endpoint: "http://127.0.0.1:50051".to_string(),
                connect_timeout_ms: 3000,
            },
            facebook: FacebookConfig {
                app_id: None,
                config_id: None,
                user_token: None,
            },
            telemetry: TelemetryConfig { enabled: false },
        }
    }
}

fn var(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn optional_var(key: &str) -> Option<String> {
    env::var(key).ok().filter(|value| !value.trim().is_empty())
}

fn bool_var(key: &str) -> bool {
    env::var(key).map(|value| value == "true").unwrap_or(false)
}

fn parse_u16(key: &str, default: u16) -> u16 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default)
}

fn parse_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn split_csv(key: &str, default: Vec<String>) -> Vec<String> {
    env::var(key)
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(|entry| entry.trim().to_string())
                .filter(|entry| !entry.is_empty())
                .collect::<Vec<_>>()
        })
        .filter(|entries| !entries.is_empty())
        .unwrap_or(default)
}

fn parse_allowed_ips(raw: Option<String>) -> Option<HashSet<IpAddr>> {
    raw.map(|list| {
        list.split(',')
            .filter_map(|entry| entry.trim().parse::<IpAddr>().ok())
            .collect::<HashSet<_>>()
    })
    .filter(|set| !set.is_empty())
}
