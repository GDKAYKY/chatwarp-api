use std::env;

use crate::error::AppError;
use log::error;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub auth_storage_path: String,
    pub recipient_jid: String,
    pub message_text: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, AppError> {
        let auth_storage_path =
            env::var("WA_AUTH_STORAGE_PATH").unwrap_or_else(|_| "session.db".into());

        let recipient_jid = env::var("WA_RECIPIENT_JID").map_err(|_| {
            let cwd = env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "unknown".to_string());
            error!("Failed to find WA_RECIPIENT_JID. Current directory: {}", cwd);
            AppError::MissingEnv("WA_RECIPIENT_JID")
        })?;

        if recipient_jid.trim().is_empty() || !recipient_jid.contains('@') {
            return Err(AppError::InvalidEnv {
                name: "WA_RECIPIENT_JID",
                reason: "expected jid format, e.g. 5511999999999@c.us".to_owned(),
            });
        }

        let message_text =
            env::var("WA_MESSAGE_TEXT").map_err(|_| AppError::MissingEnv("WA_MESSAGE_TEXT"))?;

        if message_text.trim().is_empty() {
            return Err(AppError::InvalidEnv {
                name: "WA_MESSAGE_TEXT",
                reason: "cannot be empty".to_owned(),
            });
        }

        Ok(Self {
            auth_storage_path,
            recipient_jid,
            message_text,
        })
    }
}
