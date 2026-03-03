use axum::response::Html;
use serde_json::Value;

/// Returns the static OpenAPI 3.0 document for the current HTTP surface.
pub fn openapi_document() -> Value {
    let raw = include_str!("openapi.json");
    serde_json::from_str(raw).expect("openapi.json must be valid JSON")
}

/// Returns Swagger UI HTML page bound to `/openapi.json`.
pub fn swagger_ui() -> Html<&'static str> {
    Html(include_str!("swagger_ui.html"))
}
