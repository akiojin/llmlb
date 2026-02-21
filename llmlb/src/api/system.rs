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
/// Force an update check now (ignores TTL cache).
///
/// Admin only when auth is enabled.
pub async fn check_update(
    State(state): State<AppState>,
    claims: Option<Extension<Claims>>,
) -> Response {
    if !crate::config::is_auth_disabled() {
        let Some(Extension(claims)) = claims else {
            return AppError(LbError::Authentication("Unauthorized".to_string())).into_response();
        };
        if claims.role != UserRole::Admin {
            return AppError(LbError::Authorization("Admin access required".to_string()))
                .into_response();
        }
    }

    match state.update_manager.check_now().await {
        Ok(update) => (StatusCode::OK, Json(CheckUpdateResponse { update })).into_response(),
        Err(err) => AppError(LbError::Http(err.to_string())).into_response(),
    }
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
            return AppError(LbError::Authentication("Unauthorized".to_string())).into_response();
        };
        if claims.role != UserRole::Admin {
            return AppError(LbError::Authorization("Admin access required".to_string()))
                .into_response();
        }
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
/// Admin only when auth is enabled.
pub async fn apply_force_update(
    State(state): State<AppState>,
    claims: Option<Extension<Claims>>,
) -> Response {
    if !crate::config::is_auth_disabled() {
        let Some(Extension(claims)) = claims else {
            return AppError(LbError::Authentication("Unauthorized".to_string())).into_response();
        };
        if claims.role != UserRole::Admin {
            return AppError(LbError::Authorization("Admin access required".to_string()))
                .into_response();
        }
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
