use serde_json::Error as SerdeError;
use sqlx::PgPool;
use thiserror::Error;

use crate::wa::auth::AuthState;

/// Repository for persisting auth state per instance in PostgreSQL.
#[derive(Clone)]
pub struct AuthRepo {
    pool: PgPool,
}

impl AuthRepo {
    /// Creates a new repository using a shared connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Saves auth state for an instance using upsert semantics.
    pub async fn save(&self, instance_name: &str, state: &AuthState) -> Result<(), AuthRepoError> {
        let serialized = serde_json::to_string(state)?;

        sqlx::query(
            r#"
            INSERT INTO auth_states (instance_name, state_json, updated_at)
            VALUES ($1, $2, NOW())
            ON CONFLICT (instance_name)
            DO UPDATE SET state_json = EXCLUDED.state_json, updated_at = NOW()
            "#,
        )
        .bind(instance_name)
        .bind(serialized)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Loads auth state for an instance.
    pub async fn load(&self, instance_name: &str) -> Result<Option<AuthState>, AuthRepoError> {
        let serialized = sqlx::query_scalar::<_, String>(
            "SELECT state_json FROM auth_states WHERE instance_name = $1",
        )
        .bind(instance_name)
        .fetch_optional(&self.pool)
        .await?;

        match serialized {
            Some(raw) => Ok(Some(serde_json::from_str(&raw)?)),
            None => Ok(None),
        }
    }

    /// Enqueues an outbound payload for external runner delivery.
    pub async fn enqueue_outbound(
        &self,
        instance_name: &str,
        message_id: &str,
        payload: &[u8],
    ) -> Result<(), AuthRepoError> {
        sqlx::query(
            r#"
            INSERT INTO wa_runner_outbox (instance_name, message_id, payload, created_at)
            VALUES ($1, $2, $3, NOW())
            "#,
        )
        .bind(instance_name)
        .bind(message_id)
        .bind(payload)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

/// Errors for auth repository operations.
#[derive(Debug, Error)]
pub enum AuthRepoError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] SerdeError),
}
