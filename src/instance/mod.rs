pub mod error;
pub mod handle;
pub mod runner;

use std::{
    collections::HashMap,
    sync::Arc,
};

use tokio::sync::{RwLock, broadcast, mpsc};

use crate::{
    db::auth_store::{
        AuthStore,
        InMemoryAuthStore,
    },
    wa::events::Event,
};

pub use error::InstanceError;
pub use handle::{ConnectionState, InstanceCommand, InstanceHandle};

/// Configuration used when creating a new instance task.
#[derive(Debug, Clone, Default)]
pub struct InstanceConfig {
    /// Whether to trigger initial connect command after creation.
    pub auto_connect: bool,
}

/// In-memory manager for multiple WA instances.
#[derive(Clone)]
pub struct InstanceManager {
    instances: Arc<RwLock<HashMap<String, InstanceHandle>>>,
    auth_store: Arc<dyn AuthStore>,
    wa_ws_url: String,
}

impl InstanceManager {
    const DEFAULT_WA_WS_URL: &'static str = "wss://web.whatsapp.com/ws/chat";

    /// Creates a new empty manager.
    pub fn new() -> Self {
        Self::new_with_runtime(
            Arc::new(InMemoryAuthStore::new()),
            Self::DEFAULT_WA_WS_URL.to_owned(),
        )
    }

    /// Creates a manager with explicit auth store and ws endpoint.
    pub fn new_with_runtime(auth_store: Arc<dyn AuthStore>, wa_ws_url: String) -> Self {
        Self {
            instances: Arc::new(RwLock::new(HashMap::new())),
            auth_store,
            wa_ws_url,
        }
    }

    /// Creates a new instance and starts its runner task.
    pub async fn create(&self, name: &str, config: InstanceConfig) -> Result<(), InstanceError> {
        let name = normalize_instance_name(name)?;
        let handle = {
            let mut instances = self.instances.write().await;
            if instances.contains_key(name) {
                return Err(InstanceError::AlreadyExists);
            }

            let (tx, rx) = mpsc::channel(64);
            let (event_tx, _) = broadcast::channel::<Event>(256);
            let state = Arc::new(RwLock::new(ConnectionState::Disconnected));
            let handle = InstanceHandle::new(tx, state.clone(), event_tx.clone());

            tokio::spawn(crate::instance::runner::run(
                name.to_owned(),
                state,
                rx,
                event_tx,
                self.auth_store.clone(),
                self.wa_ws_url.clone(),
            ));
            instances.insert(name.to_owned(), handle.clone());
            handle
        };

        if config.auto_connect {
            handle.connect().await?;
        }

        Ok(())
    }

    /// Returns an instance handle by name.
    pub async fn get(&self, name: &str) -> Option<InstanceHandle> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }

        self.instances.read().await.get(name).cloned()
    }

    /// Returns the current total number of tracked instances.
    pub async fn count(&self) -> usize {
        self.instances.read().await.len()
    }

    /// Deletes an instance and asks its runner to shutdown.
    pub async fn delete(&self, name: &str) -> Result<(), InstanceError> {
        let name = normalize_instance_name(name)?;
        let handle = {
            let mut instances = self.instances.write().await;
            instances.remove(name).ok_or(InstanceError::NotFound)?
        };

        handle
            .tx
            .send(InstanceCommand::Shutdown)
            .await
            .map_err(|_| InstanceError::CommandChannelClosed)?;

        Ok(())
    }
}

impl Default for InstanceManager {
    fn default() -> Self {
        Self::new()
    }
}

fn normalize_instance_name(name: &str) -> Result<&str, InstanceError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(InstanceError::InvalidName);
    }

    Ok(trimmed)
}
