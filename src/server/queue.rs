use crate::api_store::ApiBind;
use crate::server::AppState;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

/// Representa o estado genérico de um job em fila.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueStatus {
    Pending,
    Processing,
    Sent,
    Failed,
}

impl QueueStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            QueueStatus::Pending => "pending",
            QueueStatus::Processing => "processing",
            QueueStatus::Sent => "sent",
            QueueStatus::Failed => "failed",
        }
    }
}

/// Job genérico de fila.
pub trait QueueJob {
    type Id: Clone + Send + Sync + 'static;

    fn id(&self) -> Self::Id;
}

/// Interface genérica para filas baseadas em banco.
#[async_trait]
pub trait Queue<J: QueueJob + Send + Sync> {
    async fn enqueue(&self, job: J) -> anyhow::Result<()>;
    async fn claim_batch(&self, limit: i64) -> anyhow::Result<Vec<J>>;
}

/// Job específico da fila de webhooks (`webhook_outbox`).
#[derive(Debug, Clone)]
pub struct WebhookJob {
    pub id: Uuid,
    pub session: Option<String>,
    pub event: String,
    pub payload: Value,
    pub attempts: i32,
}

impl QueueJob for WebhookJob {
    type Id = Uuid;

    fn id(&self) -> Self::Id {
        self.id
    }
}

/// Implementação de fila de webhooks em cima do `AppState`.
#[derive(Clone)]
pub struct WebhookQueue {
    state: Arc<AppState>,
}

impl WebhookQueue {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    /// Marca um webhook como enviado com sucesso.
    pub async fn mark_sent(&self, id: Uuid) -> anyhow::Result<()> {
        self.state
            .api_store
            .execute(
                "UPDATE webhook_outbox SET status = 'sent', last_error = NULL WHERE id = $1",
                vec![ApiBind::Uuid(id)],
            )
            .await?;
        Ok(())
    }

    /// Marca um webhook para nova tentativa, aplicando backoff incremental.
    pub async fn mark_retry(&self, id: Uuid, attempts: i32, error: String) -> anyhow::Result<()> {
        let (status, delay_seconds) = if attempts >= 5 {
            ("failed", 600)
        } else {
            ("pending", backoff_seconds(attempts))
        };

        self.state
            .api_store
            .execute(
                "UPDATE webhook_outbox \
                 SET status = $2, attempts = $3, last_error = $4, \
                     next_attempt_at = now() + ($5 || ' seconds')::interval \
                 WHERE id = $1",
                vec![
                    ApiBind::Uuid(id),
                    ApiBind::Text(status.to_string()),
                    ApiBind::Int(attempts),
                    ApiBind::Text(error),
                    ApiBind::Int(delay_seconds),
                ],
            )
            .await?;
        Ok(())
    }
}

#[async_trait]
impl Queue<WebhookJob> for WebhookQueue {
    /// Insere um novo registro em `webhook_outbox`.
    async fn enqueue(&self, job: WebhookJob) -> anyhow::Result<()> {
        self.state
            .api_store
            .execute(
                "INSERT INTO webhook_outbox (id, session, event, payload, status, attempts, next_attempt_at) \
                 VALUES ($1, $2, $3, $4, 'pending', $5, now())",
                vec![
                    ApiBind::Uuid(job.id),
                    ApiBind::NullableText(job.session),
                    ApiBind::Text(job.event),
                    ApiBind::Json(job.payload),
                    ApiBind::Int(job.attempts),
                ],
            )
            .await?;
        Ok(())
    }

    /// Seleciona e marca um lote de webhooks como `processing` usando `FOR UPDATE SKIP LOCKED`.
    async fn claim_batch(&self, limit: i64) -> anyhow::Result<Vec<WebhookJob>> {
        let rows = self
            .state
            .api_store
            .query_json(
                "WITH claimed AS ( \
                    SELECT id \
                    FROM webhook_outbox \
                    WHERE status = 'pending' AND next_attempt_at <= now() \
                    ORDER BY created_at \
                    LIMIT $1 \
                    FOR UPDATE SKIP LOCKED \
                ), updated AS ( \
                    UPDATE webhook_outbox w \
                    SET status = 'processing' \
                    FROM claimed \
                    WHERE w.id = claimed.id \
                    RETURNING w.id, w.session, w.event, w.payload, w.attempts \
                ) \
                SELECT row_to_json(updated)::jsonb as value FROM updated",
                vec![ApiBind::Int(limit as i32)],
            )
            .await?;

        let mut jobs = Vec::with_capacity(rows.len());
        for row in rows {
            let value = row.get("value").cloned().unwrap_or_else(|| row.clone());

            let id = value
                .get("id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok());
            let event = value
                .get("event")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let session = value
                .get("session")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let payload = value.get("payload").cloned().unwrap_or(Value::Null);
            let attempts = value.get("attempts").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

            let Some(id) = id else { continue };

            jobs.push(WebhookJob {
                id,
                session,
                event,
                payload,
                attempts,
            });
        }

        Ok(jobs)
    }
}

/// Job específico da fila de mensagens (`api_messages`).
#[derive(Debug, Clone)]
pub struct MessageJob {
    pub id: Uuid,
    pub session: String,
    pub chat_id: String,
    pub message_type: String,
    pub payload: Value,
    pub created_at: Option<DateTime<Utc>>,
    pub attempts: i32,
}

impl QueueJob for MessageJob {
    type Id = Uuid;

    fn id(&self) -> Self::Id {
        self.id
    }
}

/// Fila de mensagens baseada em `api_messages`.
#[derive(Clone)]
pub struct MessageQueue {
    state: Arc<AppState>,
}

impl MessageQueue {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    /// Marca a mensagem com um novo status simples (sem retry/backoff ainda).
    pub async fn mark_status(&self, id: Uuid, status: &str) -> anyhow::Result<()> {
        self.state
            .api_store
            .execute(
                "UPDATE api_messages SET status = $1 WHERE id = $2",
                vec![ApiBind::Text(status.to_string()), ApiBind::Uuid(id)],
            )
            .await
            .map(|_| ())
            .map_err(Into::into)
    }

    /// No futuro esta função poderá ser estendida para considerar `next_attempt_at`.
    pub async fn claim_for_sessions(
        &self,
        sessions: Vec<String>,
        limit: i64,
    ) -> anyhow::Result<Vec<MessageJob>> {
        if sessions.is_empty() {
            return Ok(Vec::new());
        }

        let mut sql = String::from(
            "WITH claimed AS ( \
                SELECT id \
                FROM api_messages \
                WHERE status = 'queued' AND session IN (",
        );
        for (idx, _) in sessions.iter().enumerate() {
            if idx > 0 {
                sql.push_str(", ");
            }
            sql.push_str(&format!("${}", idx + 1));
        }
        sql.push_str(
            ") \
                ORDER BY created_at \
                LIMIT ",
        );
        sql.push_str(&limit.to_string());
        sql.push_str(
            " \
                FOR UPDATE SKIP LOCKED \
            ), updated AS ( \
                UPDATE api_messages \
                SET status = 'processing' \
                FROM claimed \
                WHERE api_messages.id = claimed.id \
                RETURNING api_messages.id, api_messages.session, api_messages.chat_id, \
                         api_messages.message_type, api_messages.payload, api_messages.created_at, \
                         COALESCE(api_messages.attempts, 0) AS attempts \
            ) \
            SELECT row_to_json(updated)::jsonb as value FROM updated",
        );

        let binds = sessions.into_iter().map(ApiBind::Text).collect();
        let rows = self.state.api_store.query_json(&sql, binds).await?;

        let mut jobs = Vec::with_capacity(rows.len());
        for row in rows {
            let value = row.get("value").cloned().unwrap_or_else(|| row.clone());

            let id = value
                .get("id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok());
            let session = value
                .get("session")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let chat_id = value
                .get("chat_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let message_type = value
                .get("message_type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let payload = value.get("payload").cloned().unwrap_or(Value::Null);
            let created_at = value
                .get("created_at")
                .and_then(|v| v.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc));
            let attempts = value.get("attempts").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

            let Some(id) = id else { continue };

            jobs.push(MessageJob {
                id,
                session,
                chat_id,
                message_type,
                payload,
                created_at,
                attempts,
            });
        }

        Ok(jobs)
    }
}

#[async_trait]
impl Queue<MessageJob> for MessageQueue {
    async fn enqueue(&self, _job: MessageJob) -> anyhow::Result<()> {
        // O fluxo atual de criação de mensagens já insere em `api_messages`.
        // Mantemos esta função apenas para cumprir a interface genérica sem duplicar lógica.
        anyhow::bail!("MessageQueue::enqueue não é usado no fluxo atual")
    }

    async fn claim_batch(&self, _limit: i64) -> anyhow::Result<Vec<MessageJob>> {
        anyhow::bail!(
            "Use MessageQueue::claim_for_sessions para selecionar mensagens por sessão ativa"
        )
    }
}

fn backoff_seconds(attempts: i32) -> i32 {
    match attempts {
        1 => 5,
        2 => 30,
        3 => 120,
        _ => 600,
    }
}
