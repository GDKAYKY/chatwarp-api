use serde_json::Value;

pub fn session_from_body(body: &Value) -> String {
    body.get("session")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("default")
        .to_string()
}

pub fn chat_id_from_body(body: &Value) -> Option<String> {
    body.get("chatId")
        .or_else(|| body.get("chat_id"))
        .or_else(|| body.get("to"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}
