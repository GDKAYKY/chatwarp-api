pub mod app;
pub mod db;
pub mod instance;
mod config;
mod error;
pub mod wa;

use app::{AppState, build_router};
use config::Config;

/// Starts the chatwarp-api runtime.
pub async fn run() -> anyhow::Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    let config = Config::from_env()?;
    let bind_addr = config.bind_addr;

    tracing::info!(%bind_addr, "starting chatwarp-api");

    let state = AppState::new();
    state.set_ready(true);

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    axum::serve(listener, build_router(state)).await?;

    Ok(())
}
