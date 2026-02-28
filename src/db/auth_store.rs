use std::{
    collections::HashMap,
    sync::Arc,
};

use futures::future::BoxFuture;
use thiserror::Error;
use tokio::sync::RwLock;

use crate::{
    db::auth_repo::{
        AuthRepo,
        AuthRepoError,
    },
    wa::auth::AuthState,
};

/// Generic persistence contract for instance auth state.
pub trait AuthStore: Send + Sync {
    /// Loads auth state for the target instance.
    fn load<'a>(
        &'a self,
        instance_name: &'a str,
    ) -> BoxFuture<'a, Result<Option<AuthState>, AuthStoreError>>;

    /// Saves auth state for the target instance.
    fn save<'a>(
        &'a self,
        instance_name: &'a str,
        state: &'a AuthState,
    ) -> BoxFuture<'a, Result<(), AuthStoreError>>;
}

/// PostgreSQL-backed auth store implementation.
#[derive(Clone)]
pub struct PgAuthStore {
    repo: AuthRepo,
}

impl PgAuthStore {
    /// Creates a new postgres auth store.
    pub fn new(repo: AuthRepo) -> Self {
        Self { repo }
    }
}

impl AuthStore for PgAuthStore {
    fn load<'a>(
        &'a self,
        instance_name: &'a str,
    ) -> BoxFuture<'a, Result<Option<AuthState>, AuthStoreError>> {
        Box::pin(async move {
            self.repo
                .load(instance_name)
                .await
                .map_err(AuthStoreError::Repository)
        })
    }

    fn save<'a>(
        &'a self,
        instance_name: &'a str,
        state: &'a AuthState,
    ) -> BoxFuture<'a, Result<(), AuthStoreError>> {
        Box::pin(async move {
            self.repo
                .save(instance_name, state)
                .await
                .map_err(AuthStoreError::Repository)
        })
    }
}

/// In-memory auth store used by tests and lightweight local runs.
#[derive(Clone, Default)]
pub struct InMemoryAuthStore {
    states: Arc<RwLock<HashMap<String, AuthState>>>,
}

impl InMemoryAuthStore {
    /// Creates an empty in-memory store.
    pub fn new() -> Self {
        Self::default()
    }
}

impl AuthStore for InMemoryAuthStore {
    fn load<'a>(
        &'a self,
        instance_name: &'a str,
    ) -> BoxFuture<'a, Result<Option<AuthState>, AuthStoreError>> {
        Box::pin(async move {
            let guard = self.states.read().await;
            Ok(guard.get(instance_name).cloned())
        })
    }

    fn save<'a>(
        &'a self,
        instance_name: &'a str,
        state: &'a AuthState,
    ) -> BoxFuture<'a, Result<(), AuthStoreError>> {
        Box::pin(async move {
            let mut guard = self.states.write().await;
            guard.insert(instance_name.to_owned(), state.clone());
            Ok(())
        })
    }
}

/// Errors exposed by generic auth store operations.
#[derive(Debug, Error)]
pub enum AuthStoreError {
    #[error(transparent)]
    Repository(#[from] AuthRepoError),
}
