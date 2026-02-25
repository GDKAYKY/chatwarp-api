use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use rand_core::{OsRng, RngCore};

use crate::wa::error::SignalError;

/// Bundle exchanged with a remote peer for synthetic session bootstrap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionBundle {
    /// Remote identity key bytes.
    pub peer_identity_key: [u8; 32],
    /// Shared seed used to derive the initial session key.
    pub shared_secret: [u8; 32],
    /// Identifier for remote one-time pre-key.
    pub pre_key_id: u32,
    /// Identifier for remote signed pre-key.
    pub signed_pre_key_id: u32,
}

/// Mutable per-peer synthetic Signal session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalSession {
    /// Root key for message key derivation.
    pub root_key: [u8; 32],
    /// Outbound message counter.
    pub send_counter: u32,
    /// Inbound message counter.
    pub recv_counter: u32,
}

/// Access to local identity key material.
pub trait IdentityKeyStore {
    /// Returns local identity key bytes.
    fn local_identity_key(&self) -> Result<[u8; 32], SignalError>;
}

/// Access to pre-key material.
pub trait PreKeyStore {
    /// Loads a one-time pre-key by id.
    fn load_pre_key(&self, key_id: u32) -> Result<Option<[u8; 32]>, SignalError>;
    /// Stores a one-time pre-key by id.
    fn store_pre_key(&self, key_id: u32, key: [u8; 32]) -> Result<(), SignalError>;
}

/// Access to signed pre-key material.
pub trait SignedPreKeyStore {
    /// Loads a signed pre-key by id.
    fn load_signed_pre_key(&self, key_id: u32) -> Result<Option<[u8; 32]>, SignalError>;
    /// Stores a signed pre-key by id.
    fn store_signed_pre_key(&self, key_id: u32, key: [u8; 32]) -> Result<(), SignalError>;
}

/// Access to remote session state.
pub trait SessionStore {
    /// Loads a peer session.
    fn load_session(&self, jid: &str) -> Result<Option<SignalSession>, SignalError>;
    /// Persists a peer session.
    fn store_session(&self, jid: &str, session: SignalSession) -> Result<(), SignalError>;
}

/// Composed store capability for synthetic Signal flows.
pub trait SignalStore: IdentityKeyStore + PreKeyStore + SignedPreKeyStore + SessionStore {}

impl<T> SignalStore for T where T: IdentityKeyStore + PreKeyStore + SignedPreKeyStore + SessionStore {}

/// In-memory store implementation for synthetic Signal flows.
#[derive(Clone)]
pub struct InMemorySignalStore {
    identity_key: [u8; 32],
    pre_keys: Arc<RwLock<HashMap<u32, [u8; 32]>>>,
    signed_pre_keys: Arc<RwLock<HashMap<u32, [u8; 32]>>>,
    sessions: Arc<RwLock<HashMap<String, SignalSession>>>,
}

impl InMemorySignalStore {
    /// Creates a new store with random identity key.
    pub fn new() -> Self {
        let mut identity_key = [0_u8; 32];
        OsRng.fill_bytes(&mut identity_key);

        Self {
            identity_key,
            pre_keys: Arc::new(RwLock::new(HashMap::new())),
            signed_pre_keys: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Creates a store from deterministic identity key.
    pub fn from_identity_key(identity_key: [u8; 32]) -> Self {
        Self {
            identity_key,
            pre_keys: Arc::new(RwLock::new(HashMap::new())),
            signed_pre_keys: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemorySignalStore {
    fn default() -> Self {
        Self::new()
    }
}

impl IdentityKeyStore for InMemorySignalStore {
    fn local_identity_key(&self) -> Result<[u8; 32], SignalError> {
        Ok(self.identity_key)
    }
}

impl PreKeyStore for InMemorySignalStore {
    fn load_pre_key(&self, key_id: u32) -> Result<Option<[u8; 32]>, SignalError> {
        let pre_keys = self
            .pre_keys
            .read()
            .map_err(|_| SignalError::StorePoisoned("pre_keys"))?;
        Ok(pre_keys.get(&key_id).copied())
    }

    fn store_pre_key(&self, key_id: u32, key: [u8; 32]) -> Result<(), SignalError> {
        let mut pre_keys = self
            .pre_keys
            .write()
            .map_err(|_| SignalError::StorePoisoned("pre_keys"))?;
        pre_keys.insert(key_id, key);
        Ok(())
    }
}

impl SignedPreKeyStore for InMemorySignalStore {
    fn load_signed_pre_key(&self, key_id: u32) -> Result<Option<[u8; 32]>, SignalError> {
        let signed_pre_keys = self
            .signed_pre_keys
            .read()
            .map_err(|_| SignalError::StorePoisoned("signed_pre_keys"))?;
        Ok(signed_pre_keys.get(&key_id).copied())
    }

    fn store_signed_pre_key(&self, key_id: u32, key: [u8; 32]) -> Result<(), SignalError> {
        let mut signed_pre_keys = self
            .signed_pre_keys
            .write()
            .map_err(|_| SignalError::StorePoisoned("signed_pre_keys"))?;
        signed_pre_keys.insert(key_id, key);
        Ok(())
    }
}

impl SessionStore for InMemorySignalStore {
    fn load_session(&self, jid: &str) -> Result<Option<SignalSession>, SignalError> {
        let sessions = self
            .sessions
            .read()
            .map_err(|_| SignalError::StorePoisoned("sessions"))?;
        Ok(sessions.get(jid).cloned())
    }

    fn store_session(&self, jid: &str, session: SignalSession) -> Result<(), SignalError> {
        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| SignalError::StorePoisoned("sessions"))?;
        sessions.insert(jid.to_owned(), session);
        Ok(())
    }
}
