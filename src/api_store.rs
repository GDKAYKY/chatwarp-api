use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum ApiBind {
    Text(String),
    NullableText(Option<String>),
    Bool(bool),
    Int(i32),
    Json(Value),
    NullableJson(Option<Value>),
    Uuid(Uuid),
}

#[async_trait]
pub trait ApiStore: Send + Sync {
    async fn query_json(&self, sql: &str, binds: Vec<ApiBind>) -> Result<Vec<Value>>;
    async fn execute(&self, sql: &str, binds: Vec<ApiBind>) -> Result<usize>;
}

pub struct NoopApiStore;

#[async_trait]
impl ApiStore for NoopApiStore {
    async fn query_json(&self, _sql: &str, _binds: Vec<ApiBind>) -> Result<Vec<Value>> {
        Err(anyhow!("api store not available (postgres-storage feature disabled)"))
    }

    async fn execute(&self, _sql: &str, _binds: Vec<ApiBind>) -> Result<usize> {
        Err(anyhow!("api store not available (postgres-storage feature disabled)"))
    }
}

#[cfg(feature = "postgres-storage")]
mod postgres_impl {
    use super::{ApiBind, ApiStore};
    use anyhow::Result;
    use async_trait::async_trait;
    use chatwarp_api_postgres_storage::BindValue as PgBind;
    use chatwarp_api_postgres_storage::PostgresStore;
    use serde_json::Value;

    fn to_pg_bind(bind: ApiBind) -> PgBind {
        match bind {
            ApiBind::Text(v) => PgBind::Text(v),
            ApiBind::NullableText(v) => PgBind::NullableText(v),
            ApiBind::Bool(v) => PgBind::Bool(v),
            ApiBind::Int(v) => PgBind::Int(v),
            ApiBind::Json(v) => PgBind::Json(v),
            ApiBind::NullableJson(v) => PgBind::NullableJson(v),
            ApiBind::Uuid(v) => PgBind::Uuid(v),
        }
    }

    #[async_trait]
    impl ApiStore for PostgresStore {
        async fn query_json(&self, sql: &str, binds: Vec<ApiBind>) -> Result<Vec<Value>> {
            let pg_binds = binds.into_iter().map(to_pg_bind).collect();
            Ok(self.api_query_json(sql, pg_binds).await?)
        }

        async fn execute(&self, sql: &str, binds: Vec<ApiBind>) -> Result<usize> {
            let pg_binds = binds.into_iter().map(to_pg_bind).collect();
            Ok(self.api_execute(sql, pg_binds).await?)
        }
    }
}
