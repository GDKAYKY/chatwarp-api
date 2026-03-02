mod config;
mod error;
mod whatsapp;

use config::AppConfig;
use error::AppError;
use tracing::error;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    if let Err(error) = run().await {
        error!("{error}");
        eprintln!("fatal: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), AppError> {
    init_tracing();
    let config = AppConfig::from_env()?;
    whatsapp::run_client(&config).await
}

fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .compact()
        .try_init();
}
