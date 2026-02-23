//! System API (self-update status / apply).

use crate::common::auth::{Claims, UserRole};
use crate::common::error::LbError;
use crate::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::Serialize;
use serde_json::json;

use super::error::AppError;

#[derive(Debug, Serialize)]
struct SystemInfoResponse {
    version: String,
    pid: u32,
    in_flight: usize,
    update: crate::update::UpdateState,
}

#[derive(Debug, Serialize)]
struct CheckUpdateResponse {
    update: crate::update::UpdateState,
}

#[derive(Debug, Serialize)]
struct ApplyUpdateResponse {
    queued: bool,
    mode: &'static str,
}

#[derive(Debug, Serialize)]
struct ForceApplyUpdateResponse {
    queued: bool,
    mode: &'static str,
    dropped_in_flight: usize,
}

/// GET /api/version
///
/// 認証不要。ビルド時のバージョン文字列を返す。
pub async fn get_version() -> Response {
    Json(json!({ "version": env!("CARGO_PKG_VERSION") })).into_response()
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

/// POST /api/system/update/check
///
/// Check for updates (GitHub API only, no download).
/// Rate-limited to once per 60 seconds.
///
/// Admin only (JWT middleware applied in create_app).
pub async fn check_update(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    if claims.role != UserRole::Admin {
        return AppError(LbError::Authorization("Admin access required".to_string()))
            .into_response();
    }

    // Rate limit: reject if checked within the last 60 seconds.
    if state.update_manager.is_manual_check_rate_limited() {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
                "error": {
                    "message": "Rate limited: please wait before checking again",
                    "type": "rate_limit",
                    "code": 429
                }
            })),
        )
            .into_response();
    }

    state.update_manager.record_manual_check();

    match state.update_manager.check_only(true).await {
        Ok(update) => {
            // If an update is available, start background download.
            if matches!(&update, crate::update::UpdateState::Available { .. }) {
                state.update_manager.download_background();
            }
            (StatusCode::OK, Json(CheckUpdateResponse { update })).into_response()
        }
        Err(err) => {
            state
                .update_manager
                .record_check_failure(err.to_string())
                .await;
            AppError(LbError::Http(err.to_string())).into_response()
        }
    }
}

/// POST /api/system/update/apply
///
/// Admin only (JWT middleware applied in create_app).
pub async fn apply_update(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    if claims.role != UserRole::Admin {
        return AppError(LbError::Authorization("Admin access required".to_string()))
            .into_response();
    }

    let queued = state.update_manager.request_apply_normal().await;
    (
        StatusCode::ACCEPTED,
        Json(ApplyUpdateResponse {
            queued,
            mode: "normal",
        }),
    )
        .into_response()
}

/// POST /api/system/update/apply/force
///
/// Admin only (JWT middleware applied in create_app).
pub async fn apply_force_update(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    if claims.role != UserRole::Admin {
        return AppError(LbError::Authorization("Admin access required".to_string()))
            .into_response();
    }

    match state.update_manager.request_apply_force().await {
        Ok(dropped_in_flight) => (
            StatusCode::ACCEPTED,
            Json(ForceApplyUpdateResponse {
                queued: false,
                mode: "force",
                dropped_in_flight,
            }),
        )
            .into_response(),
        Err(err) => AppError(LbError::Conflict(err.to_string())).into_response(),
    }
}
