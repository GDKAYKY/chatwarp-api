use std::{collections::HashMap, sync::Arc};

use lapin::{
    BasicProperties, Channel, Connection, ConnectionProperties, ExchangeKind,
    options::{BasicPublishOptions, ExchangeDeclareOptions},
    types::FieldTable,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, broadcast};
use tracing::{error, warn};

use crate::{config::AppConfig, errors::AppError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventData {
    pub instance_name: String,
    pub origin: String,
    pub event: String,
    pub data: serde_json::Value,
    pub server_url: String,
    pub date_time: String,
    pub sender: String,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventSinkConfig {
    pub enabled: bool,
    pub url: Option<String>,
    pub events: Vec<String>,
}

#[derive(Debug)]
pub struct EventManager {
    config: Arc<AppConfig>,
    tx: broadcast::Sender<String>,
    webhook_by_instance: RwLock<HashMap<String, EventSinkConfig>>,
    websocket_by_instance: RwLock<HashMap<String, EventSinkConfig>>,
    rabbitmq_by_instance: RwLock<HashMap<String, EventSinkConfig>>,
    rabbitmq_channel: Option<Channel>,
}

impl EventManager {
    pub async fn new(config: Arc<AppConfig>) -> Result<Self, AppError> {
        let (tx, _) = broadcast::channel(1024);

        let rabbitmq_channel = if config.rabbitmq.enabled && !config.rabbitmq.uri.is_empty() {
            match Connection::connect(&config.rabbitmq.uri, ConnectionProperties::default()).await {
                Ok(connection) => match connection.create_channel().await {
                    Ok(channel) => {
                        if let Err(error) = channel
                            .exchange_declare(
                                &config.rabbitmq.exchange_name,
                                ExchangeKind::Topic,
                                ExchangeDeclareOptions::default(),
                                FieldTable::default(),
                            )
                            .await
                        {
                            warn!("failed to declare rabbitmq exchange: {error}");
                            None
                        } else {
                            Some(channel)
                        }
                    }
                    Err(error) => {
                        warn!("failed to create rabbitmq channel: {error}");
                        None
                    }
                },
                Err(error) => {
                    warn!("failed to connect rabbitmq: {error}");
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            config,
            tx,
            webhook_by_instance: RwLock::new(HashMap::new()),
            websocket_by_instance: RwLock::new(HashMap::new()),
            rabbitmq_by_instance: RwLock::new(HashMap::new()),
            rabbitmq_channel,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    pub async fn emit(&self, event: EventData) -> Result<(), AppError> {
        let payload = serde_json::to_string(&event)?;

        if self.config.websocket.enabled {
            let _ = self.tx.send(payload.clone());
        }

        self.emit_webhook(payload.clone()).await;
        self.emit_rabbitmq(payload, &event.event).await;

        Ok(())
    }

    async fn emit_webhook(&self, payload: String) {
        if !self.config.webhook.events.errors {
            return;
        }

        if self.config.webhook.events.errors_webhook.is_empty() {
            return;
        }

        let client = reqwest::Client::new();
        if let Err(error) = client
            .post(&self.config.webhook.events.errors_webhook)
            .header("content-type", "application/json")
            .body(payload)
            .send()
            .await
        {
            error!("failed to emit webhook event: {error}");
        }
    }

    async fn emit_rabbitmq(&self, payload: String, routing_key: &str) {
        let Some(channel) = &self.rabbitmq_channel else {
            return;
        };

        if let Err(error) = channel
            .basic_publish(
                &self.config.rabbitmq.exchange_name,
                routing_key,
                BasicPublishOptions::default(),
                payload.as_bytes(),
                BasicProperties::default(),
            )
            .await
        {
            error!("failed to publish rabbitmq event: {error}");
        }
    }

    pub async fn set_webhook(&self, instance: String, config: EventSinkConfig) {
        self.webhook_by_instance.write().await.insert(instance, config);
    }

    pub async fn set_websocket(&self, instance: String, config: EventSinkConfig) {
        self.websocket_by_instance.write().await.insert(instance, config);
    }

    pub async fn set_rabbitmq(&self, instance: String, config: EventSinkConfig) {
        self.rabbitmq_by_instance.write().await.insert(instance, config);
    }

    pub async fn webhook_config(&self, instance: &str) -> Option<EventSinkConfig> {
        self.webhook_by_instance.read().await.get(instance).cloned()
    }

    pub async fn websocket_config(&self, instance: &str) -> Option<EventSinkConfig> {
        self.websocket_by_instance
            .read()
            .await
            .get(instance)
            .cloned()
    }

    pub async fn rabbitmq_config(&self, instance: &str) -> Option<EventSinkConfig> {
        self.rabbitmq_by_instance
            .read()
            .await
            .get(instance)
            .cloned()
    }
}
