//! 組織管理は global JWT のみ。閲覧は DB の現在の組織・本人 scope に従う。

use crate::api::error::{AppError, HandlerError};
use crate::auth::scope::{resolve_scope, UsageScope};
use crate::common::{auth::Claims, error::LbError};
use crate::db::organizations::{self, AccountMembership, Organization, OrganizationRole};
use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Validated organization name input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrganizationName {
    /// Display name.
    pub name: String,
}

/// Scoped organization list.
#[derive(Serialize)]
pub struct OrganizationsResponse {
    /// Organizations visible in the authorized scope.
    pub organizations: Vec<Organization>,
}

/// Scoped organization account list.
#[derive(Serialize)]
pub struct MembersResponse {
    /// Accounts visible in the authorized scope.
    pub users: Vec<AccountMembership>,
}

/// The wrapper makes an omitted organization_id an error; explicit null clears membership.
#[derive(Deserialize)]
#[serde(transparent)]
pub struct NullableOrganizationId(
    /// Explicit null clears membership; an omitted field is rejected.
    pub Option<Uuid>,
);

/// Explicit membership replacement and optional account activation change.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MembershipRequest {
    /// Organization identifier; null denotes unassigned or legacy usage.
    pub organization_id: NullableOrganizationId,
    /// Role inside the current organization.
    pub organization_role: OrganizationRole,
    /// Whether the account can authenticate.
    pub is_active: Option<bool>,
}

async fn require_global(state: &AppState, claims: &Claims) -> Result<(), HandlerError> {
    if resolve_scope(&state.db_pool, claims)
        .await
        .map_err(AppError)?
        != UsageScope::Global
    {
        return Err(AppError(LbError::Authorization(
            "System administrator access required".into(),
        ))
        .into());
    }
    Ok(())
}

/// List organizations in the current authenticated scope.
pub async fn list(
    Extension(scope): Extension<UsageScope>,
    State(state): State<AppState>,
) -> Result<Json<OrganizationsResponse>, HandlerError> {
    let organizations = organizations::list(&state.db_pool, &scope)
        .await
        .map_err(AppError)?;
    Ok(Json(OrganizationsResponse { organizations }))
}

/// Create an organization as a system administrator.
pub async fn create(
    Extension(claims): Extension<Claims>,
    State(state): State<AppState>,
    Json(request): Json<OrganizationName>,
) -> Result<(StatusCode, Json<Organization>), HandlerError> {
    require_global(&state, &claims).await?;
    let organization = organizations::create(&state.db_pool, &request.name)
        .await
        .map_err(AppError)?;
    Ok((StatusCode::CREATED, Json(organization)))
}

/// Rename an active organization as a system administrator.
pub async fn rename(
    Extension(claims): Extension<Claims>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<OrganizationName>,
) -> Result<Json<Organization>, HandlerError> {
    require_global(&state, &claims).await?;
    Ok(Json(
        organizations::rename(&state.db_pool, id, &request.name)
            .await
            .map_err(AppError)?,
    ))
}

/// Archive an empty organization as a system administrator.
pub async fn archive(
    Extension(claims): Extension<Claims>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, HandlerError> {
    require_global(&state, &claims).await?;
    organizations::archive(&state.db_pool, id)
        .await
        .map_err(AppError)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Read the permitted accounts in an organization.
pub async fn members(
    Extension(scope): Extension<UsageScope>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MembersResponse>, HandlerError> {
    let users = organizations::members(&state.db_pool, id, &scope)
        .await
        .map_err(AppError)?;
    Ok(Json(MembersResponse { users }))
}

/// Change organization membership as a system administrator.
pub async fn set_membership(
    Extension(claims): Extension<Claims>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<MembershipRequest>,
) -> Result<Json<AccountMembership>, HandlerError> {
    require_global(&state, &claims).await?;
    Ok(Json(
        organizations::set_membership(
            &state.db_pool,
            id,
            request.organization_id.0,
            request.organization_role,
            request.is_active,
        )
        .await
        .map_err(AppError)?,
    ))
}
