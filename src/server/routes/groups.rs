use crate::api_store::ApiBind;
use crate::server::webhooks;
use crate::server::AppState;
use axum::{Json, extract::{Path, State}, http::StatusCode, response::IntoResponse};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

pub async fn create_group(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let id = body
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let subject = body.get("subject").and_then(|v| v.as_str()).map(|s| s.to_string());
    let participants = body.get("participants").cloned();

    let result = state
        .api_store
        .execute(
            "INSERT INTO api_groups (session, id, subject, participants, created_at) \
             VALUES ($1, $2, $3, $4, now()) \
             ON CONFLICT (session, id) DO UPDATE SET subject = EXCLUDED.subject, participants = EXCLUDED.participants",
            vec![
                ApiBind::Text(session.clone()),
                ApiBind::Text(id.clone()),
                ApiBind::NullableText(subject),
                ApiBind::NullableJson(participants),
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
        "GROUPS_UPSERT",
        json!({"id": id}),
    )
    .await;

    (StatusCode::OK, Json(json!({"id": id})))
}

pub async fn list_groups(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
) -> impl IntoResponse {
    let Some(client_ref) = state.clients.get(&session) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "session_not_found", "session": session})),
        );
    };

    let client = client_ref.value().clone();
    drop(client_ref);

    match client.groups().get_participating().await {
        Ok(groups_map) => {
            let list: Vec<Value> = groups_map
                .values()
                .map(|g| {
                    json!({
                        "jid": g.id.to_string(),
                        "groupName": g.subject,
                    })
                })
                .collect();

            (StatusCode::OK, Json(json!(list)))
        }
        Err(err) => {
            log::error!("Failed to fetch groups for session {}: {}", session, err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "fetch_failed", "details": err.to_string()})),
            )
        }
    }
}

pub async fn get_group(
    State(state): State<Arc<AppState>>,
    Path((session, id)): Path<(String, String)>,
) -> impl IntoResponse {
    let rows = state
        .api_store
        .query_json(
            "SELECT row_to_json(api_groups)::jsonb as value FROM api_groups WHERE session = $1 AND id = $2",
            vec![ApiBind::Text(session), ApiBind::Text(id)],
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

pub async fn leave_group(
    State(state): State<Arc<AppState>>,
    Path((session, id)): Path<(String, String)>,
) -> impl IntoResponse {
    let result = state
        .api_store
        .execute(
            "UPDATE api_groups SET participants = '[]'::jsonb WHERE session = $1 AND id = $2",
            vec![ApiBind::Text(session.clone()), ApiBind::Text(id.clone())],
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
        "GROUP_PARTICIPANTS_UPDATE",
        json!({"id": id}),
    )
    .await;

    (StatusCode::OK, Json(json!({"status": "left"})))
}

pub async fn participants(
    State(state): State<Arc<AppState>>,
    Path((session, id)): Path<(String, String)>,
) -> impl IntoResponse {
    let rows = state
        .api_store
        .query_json(
            "SELECT jsonb_build_object('participants', participants) as value \
             FROM api_groups WHERE session = $1 AND id = $2",
            vec![ApiBind::Text(session), ApiBind::Text(id)],
        )
        .await;

    match rows {
        Ok(mut rows) => (
            StatusCode::OK,
            Json(rows.pop().unwrap_or_else(|| json!({"participants": []}))),
        ),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        ),
    }
}

pub async fn add_participants(
    State(state): State<Arc<AppState>>,
    Path((session, id)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    update_participants(state, session, id, body).await
}

pub async fn remove_participants(
    State(state): State<Arc<AppState>>,
    Path((session, id)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    update_participants(state, session, id, body).await
}

async fn update_participants(
    state: Arc<AppState>,
    session: String,
    id: String,
    body: Value,
) -> impl IntoResponse {
    let participants = body.get("participants").cloned().unwrap_or(json!([]));

    let result = state
        .api_store
        .execute(
            "UPDATE api_groups SET participants = $3 WHERE session = $1 AND id = $2",
            vec![
                ApiBind::Text(session.clone()),
                ApiBind::Text(id.clone()),
                ApiBind::Json(participants),
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
        "GROUP_PARTICIPANTS_UPDATE",
        json!({"id": id}),
    )
    .await;

    (StatusCode::OK, Json(json!({"status": "updated"})))
}

pub async fn join_group(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    create_group(State(state), Path(session), Json(body)).await
}

pub async fn invite_code(
    Path((_session, _id)): Path<(String, String)>,
) -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"invite_code": Uuid::new_v4().to_string()})))
}
