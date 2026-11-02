//! 組織の管理。閲覧scopeと変更権限はJWTの現在DB主体から決める。

use super::error::{AppError, HandlerError};
use crate::auth::scope::Scope;
use crate::db::organizations::{self, Organization};
use crate::AppState;
use axum::{
    extract::{Path, Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
    Extension, Json,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

/// 作成・改名時の組織名。
#[derive(Deserialize)]
pub struct NameRequest {
    /// 前後の空白を除いて保存する。
    pub name: String,
}

/// JSON入力の検証より先に、変更routeのglobal権限を強制する。
pub async fn require_global_scope(request: Request, next: Next) -> Result<Response, HandlerError> {
    let scope = request.extensions().get::<Scope>().ok_or_else(|| {
        AppError(crate::common::error::LbError::Authentication(
            "Authenticated scope is required".into(),
        ))
    })?;
    scope.require_global()?;
    Ok(next.run(request).await)
}

/// scope付き応答をブラウザや中間cacheへ保存させない。
pub(crate) fn scoped_headers(scope: &Scope) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    if let Ok(identity) = scope.cache_key().parse() {
        headers.insert("x-llmlb-scope", identity);
    }
    headers
}

/// GET /api/organizations。非globalは現在の所属だけをSQL取得する。
pub async fn list_organizations(
    Extension(scope): Extension<Scope>,
    State(state): State<AppState>,
) -> Result<(HeaderMap, Json<serde_json::Value>), HandlerError> {
    scope.require_assigned()?;
    let organizations = match &scope {
        Scope::Global => organizations::list(&state.db_pool).await,
        Scope::Organization(id)
        | Scope::SelfAccount {
            organization_id: id,
            ..
        } => organizations::find(&state.db_pool, *id)
            .await
            .map(|organization| organization.into_iter().collect()),
        Scope::Unassigned(_) => unreachable!("require_assigned rejected unassigned scope"),
    }
    .map_err(AppError)?;
    Ok((
        scoped_headers(&scope),
        Json(json!({"organizations": organizations, "scope": scope.cache_key()})),
    ))
}

/// POST /api/organizations。global JWTのみに許可する。
pub async fn create_organization(
    Extension(scope): Extension<Scope>,
    State(state): State<AppState>,
    Json(request): Json<NameRequest>,
) -> Result<(StatusCode, Json<Organization>), HandlerError> {
    scope.require_global()?;
    let organization = organizations::create(&state.db_pool, &request.name)
        .await
        .map_err(AppError)?;
    Ok((StatusCode::CREATED, Json(organization)))
}

/// PUT /api/organizations/{id}。IDは改名で変えない。
pub async fn rename_organization(
    Extension(scope): Extension<Scope>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<NameRequest>,
) -> Result<Json<Organization>, HandlerError> {
    scope.require_global()?;
    organizations::rename(&state.db_pool, id, &request.name)
        .await
        .map(Json)
        .map_err(|error| AppError(error).into())
}

/// DELETE /api/organizations/{id}。空の組織を保持したままarchiveする。
pub async fn archive_organization(
    Extension(scope): Extension<Scope>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Organization>, HandlerError> {
    scope.require_global()?;
    organizations::archive(&state.db_pool, id)
        .await
        .map(Json)
        .map_err(|error| AppError(error).into())
}
