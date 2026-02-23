use std::time::Duration;

use tokio::{sync::Mutex, time::timeout};
use tonic::transport::{Channel, Endpoint};
use tracing::warn;

use crate::{
    config::SidecarConfig,
    errors::AppError,
    proto::whatsapp_v2::{
        GenericResponse, HealthRequest, InstanceRequest, SendMessageRequest,
        message_service_client::MessageServiceClient, session_service_client::SessionServiceClient,
    },
};

#[derive(Debug)]
pub struct SidecarClients {
    session: Mutex<SessionServiceClient<Channel>>,
    message: Mutex<MessageServiceClient<Channel>>,
    timeout: Duration,
}

impl SidecarClients {
    pub async fn connect(config: &SidecarConfig) -> Result<Self, AppError> {
        let endpoint = Endpoint::from_shared(config.endpoint.clone())
            .map_err(|error| AppError::Config(error.to_string()))?
            .connect_timeout(Duration::from_millis(config.connect_timeout_ms));

        let channel = endpoint.connect().await?;
        Ok(Self::from_channel(
            channel,
            Duration::from_millis(config.connect_timeout_ms),
        ))
    }

    pub fn connect_lazy(config: &SidecarConfig) -> Result<Self, AppError> {
        let endpoint = Endpoint::from_shared(config.endpoint.clone())
            .map_err(|error| AppError::Config(error.to_string()))?
            .connect_timeout(Duration::from_millis(config.connect_timeout_ms));

        let channel = endpoint.connect_lazy();
        Ok(Self::from_channel(
            channel,
            Duration::from_millis(config.connect_timeout_ms),
        ))
    }

    pub async fn health(&self) -> bool {
        let mut session = self.session.lock().await;
        let call = session.health(HealthRequest {});
        match timeout(self.timeout, call).await {
            Ok(Ok(response)) => response.into_inner().ok,
            Ok(Err(error)) => {
                warn!("sidecar health error: {error}");
                false
            }
            Err(_) => false,
        }
    }

    pub async fn connect_instance(&self, instance_name: &str) -> Result<GenericResponse, AppError> {
        let mut session = self.session.lock().await;
        let response = timeout(
            self.timeout,
            session.connect_instance(InstanceRequest {
                instance_name: instance_name.to_string(),
            }),
        )
        .await
        .map_err(|_| AppError::service_unavailable("sidecar connect timed out"))??;

        Ok(response.into_inner())
    }

    pub async fn restart_instance(&self, instance_name: &str) -> Result<GenericResponse, AppError> {
        let mut session = self.session.lock().await;
        let response = timeout(
            self.timeout,
            session.restart_instance(InstanceRequest {
                instance_name: instance_name.to_string(),
            }),
        )
        .await
        .map_err(|_| AppError::service_unavailable("sidecar restart timed out"))??;

        Ok(response.into_inner())
    }

    pub async fn logout_instance(&self, instance_name: &str) -> Result<GenericResponse, AppError> {
        let mut session = self.session.lock().await;
        let response = timeout(
            self.timeout,
            session.logout_instance(InstanceRequest {
                instance_name: instance_name.to_string(),
            }),
        )
        .await
        .map_err(|_| AppError::service_unavailable("sidecar logout timed out"))??;

        Ok(response.into_inner())
    }

    pub async fn connection_state(&self, instance_name: &str) -> Result<GenericResponse, AppError> {
        let mut session = self.session.lock().await;
        let response = timeout(
            self.timeout,
            session.connection_state(InstanceRequest {
                instance_name: instance_name.to_string(),
            }),
        )
        .await
        .map_err(|_| AppError::service_unavailable("sidecar state timed out"))??;

        Ok(response.into_inner())
    }

    pub async fn send_message(
        &self,
        instance_name: &str,
        operation: &str,
        payload_json: String,
    ) -> Result<GenericResponse, AppError> {
        let mut message = self.message.lock().await;
        let response = timeout(
            self.timeout,
            message.execute(SendMessageRequest {
                instance_name: instance_name.to_string(),
                operation: operation.to_string(),
                payload_json,
            }),
        )
        .await
        .map_err(|_| AppError::service_unavailable("sidecar send timed out"))??;

        Ok(response.into_inner())
    }

    fn from_channel(channel: Channel, timeout: Duration) -> Self {
        Self {
            session: Mutex::new(SessionServiceClient::new(channel.clone())),
            message: Mutex::new(MessageServiceClient::new(channel)),
            timeout,
        }
    }
}
