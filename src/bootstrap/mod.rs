use std::{net::SocketAddr, sync::Arc};

use tokio::{signal, time::Duration};
use tracing::{error, info, warn};
use tracing_subscriber::{EnvFilter, fmt};

use crate::{
    config::AppConfig,
    errors::AppError,
    events::EventManager,
    http,
    repo::PgRepository,
    sidecar::SidecarClients,
    state::{AppState, RuntimeInstance},
};

pub async fn run() -> Result<(), AppError> {
    dotenvy::dotenv().ok();
    init_tracing();
    install_unexpected_error_hooks();

    let config = Arc::new(AppConfig::from_env()?);
    let _sentry_guard = init_sentry(config.sentry.dsn.clone());

    if config.provider.enabled {
        info!("Provider:Files - ON");
    }

    let repo = Arc::new(connect_repo_with_retry(&config.database.connection_uri).await?);
    repo.verify_schema().await?;
    info!("Repository:PostgreSQL - ON");

    let sidecar = Arc::new(match SidecarClients::connect(&config.sidecar).await {
        Ok(client) => {
            if client.health().await {
                info!("WhatsApp sidecar - ON");
            } else {
                warn!("WhatsApp sidecar health check failed at startup");
            }
            client
        }
        Err(error) => {
            warn!("WhatsApp sidecar unavailable at startup ({error}). Running in degraded mode.");
            SidecarClients::connect_lazy(&config.sidecar)?
        }
    });

    let events = Arc::new(EventManager::new(config.clone()).await?);
    let state = AppState {
        config,
        repo,
        sidecar,
        events,
        wa_instances: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    };

    load_instances(&state).await?;

    let app = http::build_router(state.clone());
    serve(state, app).await
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .json()
        .init();
}

fn init_sentry(dsn: Option<String>) -> Option<sentry::ClientInitGuard> {
    dsn.map(|dsn| {
        info!("Sentry - ON");
        sentry::init((
            dsn,
            sentry::ClientOptions {
                release: sentry::release_name!(),
                traces_sample_rate: 1.0,
                ..Default::default()
            },
        ))
    })
}

fn install_unexpected_error_hooks() {
    std::panic::set_hook(Box::new(|panic_info| {
        error!("uncaught panic: {panic_info}");
    }));
}

async fn load_instances(state: &AppState) -> Result<(), AppError> {
    let instances = state.repo.list_instances().await?;
    let mut lock = state.wa_instances.write().await;
    for instance in instances {
        lock.insert(
            instance.name.clone(),
            RuntimeInstance {
                id: instance.name.clone(),
                integration: instance.integration.unwrap_or_else(|| "WHATSAPP-BAILEYS".to_string()),
                state: "close".to_string(),
                token: instance.token,
                number: None,
                owner_jid: None,
                profile_name: None,
                profile_pic_url: None,
            },
        );
    }
    info!("WA instances preloaded: {}", lock.len());
    Ok(())
}

async fn serve(state: AppState, app: axum::Router) -> Result<(), AppError> {
    let addr: SocketAddr = format!("0.0.0.0:{}", state.config.server.port)
        .parse()
        .map_err(|error: std::net::AddrParseError| AppError::Config(error.to_string()))?;

    if state.config.server.kind == "https" {
        let cert = state.config.ssl_conf.fullchain.clone();
        let key = state.config.ssl_conf.privkey.clone();

        match axum_server::tls_rustls::RustlsConfig::from_pem_file(cert, key).await {
            Ok(tls_config) => {
                info!("HTTPS - ON: {}", state.config.server.port);
                let server = axum_server::bind_rustls(addr, tls_config).serve(app.into_make_service());

                tokio::select! {
                    res = server => res.map_err(|error| AppError::internal(error.to_string()))?,
                    _ = shutdown_signal() => {},
                }

                return Ok(());
            }
            Err(error) => {
                warn!("SSL cert load failed - falling back to HTTP: {error}");
            }
        }
    }

    info!("HTTP - ON: {}", state.config.server.port);
    let server = axum_server::bind(addr).serve(app.into_make_service());

    tokio::select! {
        res = server => res.map_err(|error| AppError::internal(error.to_string()))?,
        _ = shutdown_signal() => {},
    }
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler");
        sigterm.recv().await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

async fn connect_repo_with_retry(uri: &str) -> Result<PgRepository, AppError> {
    let max_attempts = 30u32;
    let wait = Duration::from_secs(2);

    for attempt in 1..=max_attempts {
        match PgRepository::connect(uri).await {
            Ok(repo) => return Ok(repo),
            Err(error) => {
                if attempt == max_attempts {
                    return Err(error);
                }
                warn!("PostgreSQL not ready (attempt {attempt}/{max_attempts}): {error}");
                tokio::time::sleep(wait).await;
            }
        }
    }

    Err(AppError::Config("unreachable retry branch".to_string()))
}
