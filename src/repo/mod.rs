use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use tracing::info;

use crate::errors::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceRecord {
    pub name: String,
    pub token: Option<String>,
    pub integration: Option<String>,
}

#[derive(Debug)]
pub struct PgRepository {
    pub pool: PgPool,
}

impl PgRepository {
    pub async fn connect(uri: &str) -> Result<Self, AppError> {
        let pool = PgPoolOptions::new().max_connections(10).connect(uri).await?;
        Ok(Self { pool })
    }

    pub fn connect_lazy(uri: &str) -> Result<Self, AppError> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect_lazy(uri)
            .map_err(|error| AppError::Config(error.to_string()))?;
        Ok(Self { pool })
    }

    pub async fn verify_schema(&self) -> Result<(), AppError> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public' AND table_name IN ('instance', 'instances')",
        )
        .fetch_one(&self.pool)
        .await?;

        if count.0 == 0 {
            return Err(AppError::Config(
                "No Evolution instance table found (expected instance or instances)".to_string(),
            ));
        }

        info!("schema check passed for Evolution instance table");
        Ok(())
    }

    pub async fn find_instance_by_name(&self, name: &str) -> Result<Option<InstanceRecord>, AppError> {
        self.fetch_instance_by("name", name).await
    }

    pub async fn find_instance_by_token(
        &self,
        token: &str,
    ) -> Result<Option<InstanceRecord>, AppError> {
        self.fetch_instance_by("token", token).await
    }

    pub async fn list_instances(&self) -> Result<Vec<InstanceRecord>, AppError> {
        let mut result = self
            .list_instances_from("instance")
            .await
            .unwrap_or_else(|_| Vec::new());
        if result.is_empty() {
            result = self
                .list_instances_from("instances")
                .await
                .unwrap_or_else(|_| Vec::new());
        }
        Ok(result)
    }

    pub async fn upsert_instance(
        &self,
        name: &str,
        token: Option<&str>,
        integration: Option<&str>,
    ) -> Result<(), AppError> {
        for table in ["instance", "instances"] {
            let query = format!(
                "INSERT INTO {table} (name, token, integration) VALUES ($1, $2, $3)
                 ON CONFLICT (name) DO UPDATE
                 SET token = EXCLUDED.token, integration = EXCLUDED.integration"
            );

            if sqlx::query(&query)
                .bind(name)
                .bind(token)
                .bind(integration)
                .execute(&self.pool)
                .await
                .is_ok()
            {
                return Ok(());
            }
        }

        Err(AppError::internal("failed to upsert instance in repository"))
    }

    pub async fn delete_instance(&self, name: &str) -> Result<(), AppError> {
        for table in ["instance", "instances"] {
            let query = format!("DELETE FROM {table} WHERE name = $1");
            if sqlx::query(&query).bind(name).execute(&self.pool).await.is_ok() {
                return Ok(());
            }
        }

        Err(AppError::internal("failed to delete instance from repository"))
    }

    async fn list_instances_from(&self, table: &str) -> Result<Vec<InstanceRecord>, AppError> {
        let query = format!("SELECT name, token, integration FROM {table}");
        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let name = row.try_get::<String, _>("name").ok()?;
                Some(InstanceRecord {
                    name,
                    token: row.try_get::<String, _>("token").ok(),
                    integration: row.try_get::<String, _>("integration").ok(),
                })
            })
            .collect())
    }

    async fn fetch_instance_by(
        &self,
        column: &str,
        value: &str,
    ) -> Result<Option<InstanceRecord>, AppError> {
        for table in ["instance", "instances"] {
            let query = format!(
                "SELECT name, token, integration FROM {table} WHERE {column} = $1 LIMIT 1"
            );
            match sqlx::query(&query).bind(value).fetch_optional(&self.pool).await {
                Ok(Some(row)) => {
                    let name = row.try_get::<String, _>("name").unwrap_or_default();
                    if name.is_empty() {
                        continue;
                    }
                    return Ok(Some(InstanceRecord {
                        name,
                        token: row.try_get::<String, _>("token").ok(),
                        integration: row.try_get::<String, _>("integration").ok(),
                    }));
                }
                Ok(None) => continue,
                Err(_) => continue,
            }
        }

        Ok(None)
    }
}
