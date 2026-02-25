pub mod app;
pub mod db;
pub mod events;
pub mod handlers;
pub mod instance;
mod group_store;
mod config;
mod error;
mod observability;
pub mod wa;

use app::{AppState, build_router};
use config::Config;
use tokio::time::Duration;

/// Starts the chatwarp-api runtime.
pub async fn run() -> anyhow::Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    let config = Config::from_env()?;
    let bind_addr = config.bind_addr;
    let connect_wait_ms = config.instance_connect_wait_ms;
    let max_body_bytes = config.server_body_limit_kb.saturating_mul(1024);

    tracing::info!(
        %bind_addr,
        connect_wait_ms,
        max_body_bytes,
        "starting chatwarp-api"
    );

    let state = AppState::with_runtime_tuning(
        Duration::from_millis(connect_wait_ms),
        max_body_bytes,
    );
    state.set_ready(true);

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    axum::serve(listener, build_router(state)).await?;

    Ok(())
}
