use crate::api_store::ApiBind;
use crate::server::AppState;
use axum::{Json, extract::{Path, State}, http::StatusCode, response::IntoResponse};
use serde_json::{Value, json};
use std::sync::Arc;

pub async fn get_profile(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
) -> impl IntoResponse {
    let rows = state
        .api_store
        .query_json(
            "SELECT row_to_json(api_profiles)::jsonb as value FROM api_profiles WHERE session = $1",
            vec![ApiBind::Text(session)],
        )
        .await;

    match rows {
        Ok(mut rows) => (
            StatusCode::OK,
            Json(rows.pop().unwrap_or_else(|| json!({}))),
        ),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        ),
    }
}

pub async fn update_name(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    update_profile_field(state, session, "name", body).await
}

pub async fn update_status(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    update_profile_field(state, session, "status", body).await
}

pub async fn update_picture(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    update_profile_field(state, session, "picture_url", body).await
}

async fn update_profile_field(
    state: Arc<AppState>,
    session: String,
    field: &str,
    body: Value,
) -> impl IntoResponse {
    let value = body
        .get("value")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let sql = format!(
        "INSERT INTO api_profiles (session, {field}, updated_at) \
         VALUES ($1, $2, now()) \
         ON CONFLICT (session) DO UPDATE SET {field} = EXCLUDED.{field}, updated_at = now()",
    );

    let result = state
        .api_store
        .execute(&sql, vec![ApiBind::Text(session.clone()), ApiBind::Text(value.clone())])
        .await;

    if let Err(err) = result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        );
    }

    (StatusCode::OK, Json(json!({"session": session, field: value})))
}
