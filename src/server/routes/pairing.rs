use crate::api_store::ApiBind;
use crate::server::webhooks;
use crate::server::AppState;
use axum::{Json, extract::{Path, State}, http::StatusCode, response::IntoResponse};
use serde_json::json;
use std::sync::Arc;
use warp_core::pair_code::PairCodeUtils;

pub async fn get_qr(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
) -> impl IntoResponse {
    let qr = state
        .sessions_runtime
        .get(&session)
        .and_then(|entry| entry.qr_code.clone());

    if let Some(qr_code) = qr {
        let _ = state
            .api_store
            .execute(
                "UPDATE api_sessions SET qr_code = $2, updated_at = now() WHERE session = $1",
                vec![ApiBind::Text(session.clone()), ApiBind::Text(qr_code.clone())],
            )
            .await;

        webhooks::enqueue(
            &state,
            Some(&session),
            "QRCODE_UPDATED",
            json!({"qr": qr_code}),
        )
        .await;

        return (StatusCode::OK, Json(json!({"session": session, "qr": qr_code})));
    }

    (
        StatusCode::NOT_FOUND,
        Json(json!({"error": "qr_not_available"})),
    )
}

pub async fn request_code(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
) -> impl IntoResponse {
    let code = PairCodeUtils::generate_code();

    let _ = state
        .api_store
        .execute(
            "UPDATE api_sessions SET pair_code = $2, updated_at = now() WHERE session = $1",
            vec![ApiBind::Text(session.clone()), ApiBind::Text(code.clone())],
        )
        .await;

    state
        .sessions_runtime
        .entry(session.clone())
        .and_modify(|entry| entry.pair_code = Some(code.clone()))
        .or_insert_with(|| {
            let mut runtime = crate::server::SessionRuntime::new();
            runtime.pair_code = Some(code.clone());
            runtime
        });

    (StatusCode::OK, Json(json!({"session": session, "code": code})))
}
