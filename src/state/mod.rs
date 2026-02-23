use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;

use crate::{
    config::AppConfig,
    errors::AppError,
    events::EventManager,
    repo::PgRepository,
    sidecar::SidecarClients,
};

#[derive(Debug, Clone)]
pub struct RuntimeInstance {
    pub id: String,
    pub integration: String,
    pub state: String,
    pub token: Option<String>,
    pub number: Option<String>,
    pub owner_jid: Option<String>,
    pub profile_name: Option<String>,
    pub profile_pic_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub repo: Arc<PgRepository>,
    pub sidecar: Arc<SidecarClients>,
    pub events: Arc<EventManager>,
    pub wa_instances: Arc<RwLock<HashMap<String, RuntimeInstance>>>,
}

impl AppState {
    pub async fn new_for_tests(config: AppConfig) -> Result<Self, AppError> {
        let config = Arc::new(config);
        let repo = Arc::new(PgRepository::connect_lazy(&config.database.connection_uri)?);
        let sidecar = Arc::new(SidecarClients::connect_lazy(&config.sidecar)?);
        let events = Arc::new(EventManager::new(config.clone()).await?);

        Ok(Self {
            config,
            repo,
            sidecar,
            events,
            wa_instances: Arc::new(RwLock::new(HashMap::new())),
        })
    }
}
