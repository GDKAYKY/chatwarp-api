pub mod baileys;
pub mod bootstrap;
pub mod config;
pub mod domain;
pub mod errors;
pub mod events;
pub mod http;
pub mod manager_ui;
pub mod metrics;
pub mod proto;
pub mod repo;
pub mod sidecar;
pub mod state;

pub async fn run() -> Result<(), errors::AppError> {
    bootstrap::run().await
}
