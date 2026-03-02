pub mod app;
pub mod db;
pub mod events;
pub mod handlers;
pub mod instance;
mod group_store;
mod config;
mod error;
mod openapi;
mod observability;
pub mod wa;

use app::{AppState, build_router};
use config::Config;
use db::{
    auth_repo::AuthRepo,
    auth_store::PgAuthStore,
};
use instance::InstanceManager;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

/// Starts the chatwarp-api runtime.
pub async fn run() -> anyhow::Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    let config = Config::from_env()?;
    let bind_addr = config.bind_addr;
    let max_body_bytes = config.server_body_limit_kb.saturating_mul(1024);
    let wa_ws_url = config.wa_ws_url;
    let wa_protocol_mode = config.wa_protocol_mode;
    let wa_runner_mode = config.wa_runner_mode;
    let wa_rs_bot_command = config.wa_rs_bot_command;
    let wa_rs_auth_poll_interval = std::time::Duration::from_secs(config.wa_rs_auth_poll_interval_secs);

    tracing::info!(
        %bind_addr,
        max_body_bytes,
        wa_runner_mode = ?wa_runner_mode,
        wa_protocol_mode = ?wa_protocol_mode,
        wa_rs_bot_command_configured = wa_rs_bot_command.is_some(),
        "starting chatwarp-api"
    );

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await?;
    let auth_store = Arc::new(PgAuthStore::new(AuthRepo::new(pool)));
    let instance_manager = InstanceManager::new_with_runtime_and_mode(
        auth_store,
        wa_ws_url,
        wa_protocol_mode,
        wa_runner_mode,
        wa_rs_bot_command,
        wa_rs_auth_poll_interval,
    );

    let state = AppState::with_instance_manager(max_body_bytes, instance_manager);
    state.set_ready(true);

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    axum::serve(listener, build_router(state)).await?;

    Ok(())
}
