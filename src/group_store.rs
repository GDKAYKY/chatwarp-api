use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

/// Synthetic group metadata kept per instance for HTTP route tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupInfo {
    /// Stable synthetic id for this group.
    pub id: String,
    /// Group subject/title.
    pub subject: String,
    /// Participant jids.
    pub participants: Vec<String>,
    /// Unix timestamp in seconds.
    pub created_at: u64,
}

/// In-memory store for groups created through M9 routes.
#[derive(Clone, Default)]
pub struct GroupStore {
    by_instance: Arc<RwLock<HashMap<String, Vec<GroupInfo>>>>,
    sequence: Arc<AtomicU64>,
}

impl GroupStore {
    /// Creates a new empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Persists and returns a new synthetic group record.
    pub async fn create(
        &self,
        instance_name: &str,
        subject: String,
        participants: Vec<String>,
    ) -> GroupInfo {
        let next = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let created_at = unix_timestamp_secs();
        let id = format!("{next}-{instance_name}@g.us");

        let record = GroupInfo {
            id,
            subject,
            participants,
            created_at,
        };

        let mut guard = self.by_instance.write().await;
        guard
            .entry(instance_name.to_owned())
            .or_default()
            .push(record.clone());

        record
    }

    /// Lists stored groups for an instance.
    pub async fn list(&self, instance_name: &str) -> Vec<GroupInfo> {
        let guard = self.by_instance.read().await;
        guard.get(instance_name).cloned().unwrap_or_default()
    }
}

fn unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |value| value.as_secs())
}
