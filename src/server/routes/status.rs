use crate::api_store::ApiBind;
use crate::server::webhooks;
use crate::server::AppState;
use axum::{Json, extract::{Path, State}, http::StatusCode, response::IntoResponse};
use serde_json::{Value, json};
use std::sync::Arc;

async fn create_status(
    state: Arc<AppState>,
    session: String,
    status_type: &str,
    payload: Value,
) -> impl IntoResponse {
    let result = state
        .api_store
        .execute(
            "INSERT INTO api_status_updates (session, status_type, payload, created_at) \
             VALUES ($1, $2, $3, now())",
            vec![
                ApiBind::Text(session.clone()),
                ApiBind::Text(status_type.to_string()),
                ApiBind::Json(payload.clone()),
            ],
        )
        .await;

    if let Err(err) = result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        );
    }

    webhooks::enqueue(
        &state,
        Some(&session),
        "MESSAGES_UPSERT",
        json!({"status_type": status_type, "payload": payload}),
    )
    .await;

    (StatusCode::OK, Json(json!({"status": "ok"})))
}

pub async fn status_text(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    create_status(state, session, "text", body).await
}

pub async fn status_image(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    create_status(state, session, "image", body).await
}

pub async fn status_video(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    create_status(state, session, "video", body).await
}

pub async fn status_delete(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let id = body.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());

    let result = if let Some(id) = id {
        state
            .api_store
            .execute(
                "DELETE FROM api_status_updates WHERE session = $1 AND id = $2",
                vec![ApiBind::Text(session.clone()), ApiBind::Text(id)],
            )
            .await
    } else {
        state
            .api_store
            .execute(
                "DELETE FROM api_status_updates WHERE session = $1",
                vec![ApiBind::Text(session.clone())],
            )
            .await
    };

    if let Err(err) = result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        );
    }

    webhooks::enqueue(
        &state,
        Some(&session),
        "MESSAGES_DELETE",
        json!({"session": session}),
    )
    .await;

    (StatusCode::OK, Json(json!({"status": "deleted"})))
}
