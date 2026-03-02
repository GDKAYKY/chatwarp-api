use serde::{Deserialize, Serialize};

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