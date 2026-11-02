//! Current database authority for every JWT-authenticated request.

use crate::api::error::HandlerError;
use crate::common::auth::{Claims, UserRole};
use crate::db::organizations::OrganizationRole;
use axum::{http::StatusCode, response::IntoResponse};
use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use uuid::Uuid;

/// A fixed server-selected read boundary. Client filters can only narrow it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// Existing system administrator.
    Global,
    /// Current organization administrator.
    Organization(Uuid),
    /// Current member, in their current organization only.
    SelfAccount {
        /// Current organization.
        organization_id: Uuid,
        /// Authenticated account.
        account_id: Uuid,
    },
    /// A valid login which still needs an organization assignment.
    Unassigned(Uuid),
}

impl Scope {
    /// Opaque cache boundary returned alongside authenticated responses.
    pub fn cache_key(&self) -> String {
        match self {
            Self::Global => "global".into(),
            Self::Organization(id) => format!("organization:{id}"),
            Self::SelfAccount {
                organization_id,
                account_id,
            } => format!("self:{organization_id}:{account_id}"),
            Self::Unassigned(id) => format!("unassigned:{id}"),
        }
    }

    /// Require the existing system administrator, independently of org role.
    pub fn require_global(&self) -> Result<(), HandlerError> {
        if matches!(self, Self::Global) {
            Ok(())
        } else {
            Err((StatusCode::FORBIDDEN, "System administrator required")
                .into_response()
                .into())
        }
    }

    /// Allow authenticated scoped reads only after membership has been set.
    pub fn require_assigned(&self) -> Result<(), HandlerError> {
        if matches!(self, Self::Unassigned(_)) {
            Err((StatusCode::FORBIDDEN, axum::Json(serde_json::json!({"error":"organization_membership_required","message":"Ask a system administrator to assign an active organization"}))).into_response().into())
        } else {
            Ok(())
        }
    }

    /// Add a bound predicate before fetching rows, counts or totals.
    /// Column names must be trusted source constants, never request input.
    pub fn push_sql(
        &self,
        query: &mut QueryBuilder<'_, Sqlite>,
        organization_column: &'static str,
        account_column: &'static str,
    ) {
        match self {
            Self::Global => {
                query.push("1 = 1");
            }
            Self::Organization(id) => {
                query
                    .push(organization_column)
                    .push(" = ")
                    .push_bind(id.to_string());
            }
            Self::SelfAccount {
                organization_id,
                account_id,
            } => {
                query
                    .push(organization_column)
                    .push(" = ")
                    .push_bind(organization_id.to_string())
                    .push(" AND ")
                    .push(account_column)
                    .push(" = ")
                    .push_bind(account_id.to_string());
            }
            Self::Unassigned(_) => {
                query.push("0 = 1");
            }
        }
    }
}

/// Current authority; JWT role and client organization claims are not authority.
pub struct CurrentIdentity {
    /// Current system role and session flags applied to the verified token.
    pub claims: Claims,
    /// Current database read boundary.
    pub scope: Scope,
    /// Current account membership.
    pub organization_id: Option<Uuid>,
    /// Organization role, independent of the system role.
    pub organization_role: OrganizationRole,
}

#[derive(sqlx::FromRow)]
struct IdentityRow {
    id: String,
    role: String,
    must_change_password: bool,
    password_changed_at: i64,
    disabled: bool,
    organization_id: Option<String>,
    organization_role: String,
    active_organization: Option<String>,
}

fn unauthorized() -> HandlerError {
    (
        StatusCode::UNAUTHORIZED,
        "Session invalid or revoked: please sign in again",
    )
        .into_response()
        .into()
}

/// Resolve identity and revocation in one database snapshot; every error closes access.
pub async fn resolve(
    pool: &SqlitePool,
    verified: &Claims,
) -> Result<CurrentIdentity, HandlerError> {
    let id = Uuid::parse_str(&verified.sub).map_err(|_| unauthorized())?;
    // Legacy development bootstrap IDs can be nil. They still require a real,
    // active DB account and every current role/session check below.
    let row = sqlx::query_as::<_, IdentityRow>(
        "SELECT u.id,u.role,u.must_change_password,u.password_changed_at,u.disabled,
                u.organization_id,u.organization_role,o.id AS active_organization
         FROM users u LEFT JOIN organizations o ON o.id=u.organization_id AND o.archived_at IS NULL
         WHERE u.id=?",
    )
    .bind(id.to_string())
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        tracing::warn!("Current identity could not be verified: {error}");
        unauthorized()
    })?
    .ok_or_else(unauthorized)?;
    if row.disabled || verified.password_changed_at < row.password_changed_at {
        return Err(unauthorized());
    }
    let role = match row.role.as_str() {
        "admin" => UserRole::Admin,
        "viewer" => UserRole::Viewer,
        _ => return Err(unauthorized()),
    };
    let organization_id = row
        .organization_id
        .as_deref()
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|_| unauthorized())?;
    if organization_id.is_some() && row.active_organization.is_none() {
        return Err(unauthorized());
    }
    let organization_role = match row.organization_role.as_str() {
        "admin" => OrganizationRole::Admin,
        "member" => OrganizationRole::Member,
        _ => return Err(unauthorized()),
    };
    let scope = match (role, organization_id, organization_role) {
        (UserRole::Admin, _, _) => Scope::Global,
        (_, Some(org), OrganizationRole::Admin) => Scope::Organization(org),
        (_, Some(org), OrganizationRole::Member) => Scope::SelfAccount {
            organization_id: org,
            account_id: id,
        },
        _ => Scope::Unassigned(id),
    };
    let mut claims = verified.clone();
    claims.sub = row.id;
    claims.role = role;
    claims.must_change_password = row.must_change_password;
    Ok(CurrentIdentity {
        claims,
        scope,
        organization_id,
        organization_role,
    })
}
