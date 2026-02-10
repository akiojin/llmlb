//! System API (self-update status / apply).

use crate::common::auth::{Claims, UserRole};
use crate::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Serialize)]
struct SystemInfoResponse {
    version: String,
    pid: u32,
    in_flight: usize,
    update: crate::update::UpdateState,
}

/// GET /api/system
pub async fn get_system(State(state): State<AppState>) -> Response {
    let update = state.update_manager.state().await;
    let in_flight = state.inference_gate.in_flight();
    Json(SystemInfoResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        pid: std::process::id(),
        in_flight,
        update,
    })
    .into_response()
}

/// POST /api/system/update/apply
///
/// Admin only when auth is enabled.
pub async fn apply_update(
    State(state): State<AppState>,
    claims: Option<Extension<Claims>>,
) -> Response {
    if !crate::config::is_auth_disabled() {
        let Some(Extension(claims)) = claims else {
            return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
        };
        if claims.role != UserRole::Admin {
            return (StatusCode::FORBIDDEN, "Admin access required").into_response();
        }
    }

    state.update_manager.request_apply();
    (
        StatusCode::ACCEPTED,
        Json(json!({
            "queued": true,
        })),
    )
        .into_response()
}
