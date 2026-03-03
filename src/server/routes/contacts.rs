use crate::api_store::ApiBind;
use crate::server::AppState;
use crate::server::webhooks;
use axum::{Json, extract::{Query, State}, http::StatusCode, response::IntoResponse};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

pub async fn list_contacts(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let session = params.get("session").cloned();
    let sql = if session.is_some() {
        "SELECT row_to_json(api_contacts)::jsonb as value FROM api_contacts WHERE session = $1"
    } else {
        "SELECT row_to_json(api_contacts)::jsonb as value FROM api_contacts"
    };
    let binds = if let Some(session) = session {
        vec![ApiBind::Text(session)]
    } else {
        vec![]
    };

    match state.api_store.query_json(sql, binds).await {
        Ok(rows) => (StatusCode::OK, Json(json!(rows))),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        ),
    }
}

pub async fn list_contacts_all(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state
        .api_store
        .query_json("SELECT row_to_json(api_contacts)::jsonb as value FROM api_contacts", vec![])
        .await
    {
        Ok(rows) => {
            webhooks::enqueue(&state, None, "CONTACTS_SET", json!({"count": rows.len()})).await;
            (StatusCode::OK, Json(json!(rows)))
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        ),
    }
}

pub async fn check_exists(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let session = params
        .get("session")
        .cloned()
        .unwrap_or_else(|| "default".to_string());
    let id = params
        .get("id")
        .or_else(|| params.get("phone"))
        .cloned();

    let Some(id) = id else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "id_required"})),
        );
    };

    let rows = state
        .api_store
        .query_json(
            "SELECT row_to_json(api_contacts)::jsonb as value FROM api_contacts WHERE session = $1 AND id = $2",
            vec![ApiBind::Text(session.clone()), ApiBind::Text(id.clone())],
        )
        .await;

    match rows {
        Ok(mut rows) => {
            if let Some(row) = rows.pop() {
                (StatusCode::OK, Json(row))
            } else {
                (
                    StatusCode::OK,
                    Json(json!({"session": session, "id": id, "exists": false})),
                )
            }
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "details": err.to_string()})),
        ),
    }
}

pub async fn profile_picture(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let session = params
        .get("session")
        .cloned()
        .unwrap_or_else(|| "default".to_string());
    let id = params
        .get("id")
        .cloned()
        .unwrap_or_else(|| "".to_string());

    if id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "id_required"})),
        );
    }

    let rows = state
        .api_store
        .query_json(
            "SELECT row_to_json(api_contacts)::jsonb as value FROM api_contacts WHERE session = $1 AND id = $2",
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
