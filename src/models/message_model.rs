use crate::client::Client;
use crate::types::message::MessageInfo;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use waproto::whatsapp as wa;
use warp_core::proto_helpers::MessageExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageModel {
    pub text: Option<String>,
    pub image: Option<MediaContent>,
    pub video: Option<MediaContent>,
    pub document: Option<MediaContent>,
    pub audio: Option<Vec<u8>>,
    pub buttons: Vec<Button>,
    pub template_buttons: Vec<TemplateButton>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MediaContent {
    Buffer(Vec<u8>),
    Url(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Button {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateButton {
    pub index: u32,
    pub quick_reply: Option<String>,
    pub url: Option<String>,
    pub call: Option<String>,
}

pub struct MessageContext {
    pub message: Box<wa::Message>,
    pub info: MessageInfo,
    pub client: Arc<Client>,
}

impl MessageContext {
    pub async fn send_message(&self, message: wa::Message) -> Result<String, anyhow::Error> {
        self.client
            .send_message(self.info.source.chat.clone(), message)
            .await
    }

    pub async fn edit_message(
        &self,
        original_message_id: String,
        new_message: wa::Message,
    ) -> Result<String, anyhow::Error> {
        self.client
            .edit_message(
                self.info.source.chat.clone(),
                original_message_id,
                new_message,
            )
            .await
    }
}

#[derive(Debug, Clone)]
pub struct IncomingMessageMetadata {
    pub sender_jid: String,
    pub remote_jid: String,
    pub is_from_me: bool,
    pub text_content: String,
}

impl IncomingMessageMetadata {
    pub fn from_message(message: &wa::Message, info: &MessageInfo) -> Self {
        Self {
            sender_jid: info.source.sender.to_string(),
            remote_jid: info.source.chat.to_string(),
            is_from_me: info.source.is_from_me,
            text_content: message.text_content().unwrap_or_default().to_string(),
        }
    }
}
