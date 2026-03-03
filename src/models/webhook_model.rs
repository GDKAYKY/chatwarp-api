use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct WebhookConfig {
    pub enabled: bool,
    pub url: String,
    pub by_events: bool,
    pub base64: bool,
    pub headers: HashMap<String, String>,
    pub events: Option<Vec<String>>,
}
