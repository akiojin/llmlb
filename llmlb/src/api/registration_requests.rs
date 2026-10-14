//! 管理者の登録申請一覧・承認・却下 API。

use super::{
    error::{AppError, HandlerError},
    users::UserResponse,
};
use crate::{common::auth::UserRole, db::registration_requests, AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

/// 管理者が指定する承認リクエスト。
#[derive(Debug, serde::Deserialize)]
pub struct ApproveRequest {
    /// 承認者が明示的に選ぶロール。
    pub role: UserRole,
}

/// GET /api/registration-requests - 未処理の申請一覧。
pub async fn list(
    State(state): State<AppState>,
) -> Result<Json<Vec<registration_requests::RegistrationRequest>>, HandlerError> {
    Ok(Json(
        registration_requests::list(&state.db_pool)
            .await
            .map_err(AppError)?,
    ))
}

/// POST /api/registration-requests/{id}/approve - ロールを指定して承認。
pub async fn approve(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<ApproveRequest>,
) -> Result<(StatusCode, Json<UserResponse>), HandlerError> {
    let user = registration_requests::approve(&state.db_pool, id, request.role)
        .await
        .map_err(AppError)?;
    Ok((StatusCode::CREATED, Json(UserResponse::from(user))))
}

/// DELETE /api/registration-requests/{id} - 未処理の申請を却下。
pub async fn reject(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, HandlerError> {
    registration_requests::reject(&state.db_pool, id)
        .await
        .map_err(AppError)?;
    Ok(StatusCode::NO_CONTENT)
}
