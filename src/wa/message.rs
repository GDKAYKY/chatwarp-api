use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use bytes::Bytes;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};

use crate::wa::{
    auth::AuthState,
    binary_node::{BinaryNode, NodeContent},
    error::MessageError,
};

/// Supported operations for `/message/:operation/:instance_name`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageOperation {
    SendTemplate,
    SendText,
    SendMedia,
    SendPtv,
    SendWhatsAppAudio,
    SendStatus,
    SendSticker,
    SendLocation,
    SendContact,
    SendReaction,
    SendPoll,
    SendList,
    SendButtons,
}

impl MessageOperation {
    /// Parses an operation from route path segment.
    pub fn parse(raw: &str) -> Result<Self, MessageError> {
        match raw {
            "sendTemplate" => Ok(Self::SendTemplate),
            "sendText" => Ok(Self::SendText),
            "sendMedia" => Ok(Self::SendMedia),
            "sendPtv" => Ok(Self::SendPtv),
            "sendWhatsAppAudio" => Ok(Self::SendWhatsAppAudio),
            "sendStatus" => Ok(Self::SendStatus),
            "sendSticker" => Ok(Self::SendSticker),
            "sendLocation" => Ok(Self::SendLocation),
            "sendContact" => Ok(Self::SendContact),
            "sendReaction" => Ok(Self::SendReaction),
            "sendPoll" => Ok(Self::SendPoll),
            "sendList" => Ok(Self::SendList),
            "sendButtons" => Ok(Self::SendButtons),
            _ => Err(MessageError::InvalidOperation(raw.to_owned())),
        }
    }

    /// Returns canonical operation name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SendTemplate => "sendTemplate",
            Self::SendText => "sendText",
            Self::SendMedia => "sendMedia",
            Self::SendPtv => "sendPtv",
            Self::SendWhatsAppAudio => "sendWhatsAppAudio",
            Self::SendStatus => "sendStatus",
            Self::SendSticker => "sendSticker",
            Self::SendLocation => "sendLocation",
            Self::SendContact => "sendContact",
            Self::SendReaction => "sendReaction",
            Self::SendPoll => "sendPoll",
            Self::SendList => "sendList",
            Self::SendButtons => "sendButtons",
        }
    }
}

/// Outbound message received by the message API handler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutgoingMessage {
    /// Recipient JID.
    pub to: String,
    /// Typed content payload.
    pub content: MessageContent,
}

/// Supported outbound payload variants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MessageContent {
    Text { text: String },
    Image { data_base64: String, caption: Option<String> },
    Video { data_base64: String, caption: Option<String> },
    Audio { data_base64: String },
    Sticker { data_base64: String },
    Location { latitude: String, longitude: String, name: Option<String> },
    Contact { vcard: String },
    Reaction { message_id: String, emoji: String },
    Poll { question: String, options: Vec<String> },
    List { title: String, items: Vec<String> },
    Buttons { text: String, buttons: Vec<String> },
    Template { name: String, language: Option<String> },
    Status { text: String },
}

/// Validates that operation and payload type are compatible.
pub fn validate_operation(operation: MessageOperation, payload: &OutgoingMessage) -> Result<(), MessageError> {
    let content_ok = match operation {
        MessageOperation::SendText => matches!(&payload.content, MessageContent::Text { .. }),
        MessageOperation::SendMedia => {
            matches!(&payload.content, MessageContent::Image { .. } | MessageContent::Video { .. })
        }
        MessageOperation::SendPtv => matches!(&payload.content, MessageContent::Video { .. }),
        MessageOperation::SendWhatsAppAudio => matches!(&payload.content, MessageContent::Audio { .. }),
        MessageOperation::SendSticker => matches!(&payload.content, MessageContent::Sticker { .. }),
        MessageOperation::SendLocation => matches!(&payload.content, MessageContent::Location { .. }),
        MessageOperation::SendContact => matches!(&payload.content, MessageContent::Contact { .. }),
        MessageOperation::SendReaction => matches!(&payload.content, MessageContent::Reaction { .. }),
        MessageOperation::SendPoll => matches!(&payload.content, MessageContent::Poll { .. }),
        MessageOperation::SendList => matches!(&payload.content, MessageContent::List { .. }),
        MessageOperation::SendButtons => matches!(&payload.content, MessageContent::Buttons { .. }),
        MessageOperation::SendTemplate => matches!(&payload.content, MessageContent::Template { .. }),
        MessageOperation::SendStatus => matches!(&payload.content, MessageContent::Status { .. }),
    };

    if content_ok {
        Ok(())
    } else {
        Err(MessageError::InvalidContentForOperation {
            operation: operation.as_str().to_owned(),
        })
    }
}

/// Builds an outbound binary node payload for a message operation.
pub fn build_message_node(
    message_id: &str,
    operation: MessageOperation,
    message: &OutgoingMessage,
    auth: Option<&AuthState>,
) -> Result<BinaryNode, MessageError> {
    let mut attrs = HashMap::new();
    attrs.insert("to".to_owned(), message.to.clone());
    attrs.insert("op".to_owned(), operation.as_str().to_owned());
    attrs.insert("id".to_owned(), message_id.to_owned());

    if let Some(auth) = auth {
        attrs.insert(
            "registrationId".to_owned(),
            auth.identity.registration_id.to_string(),
        );
    }

    let content = serde_json::to_vec(&message.content)?;

    Ok(BinaryNode {
        tag: "message".to_owned(),
        attrs,
        content: NodeContent::Bytes(Bytes::from(content)),
    })
}

/// Generates a stable synthetic message id.
pub fn generate_message_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();

    let mut entropy = [0_u8; 4];
    OsRng.fill_bytes(&mut entropy);
    format!("msg-{millis:013}-{:08x}", u32::from_be_bytes(entropy))
}
