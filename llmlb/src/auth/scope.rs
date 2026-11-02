//! Server-side authorization from current database state, independent of JWT role claims.

use crate::common::{
    auth::{Claims, UserRole},
    error::LbError,
};
use crate::db::organizations::{self, AccountMembership, OrganizationRole};
use sqlx::SqlitePool;
use uuid::Uuid;

/// Authorized visibility derived from current database membership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsageScope {
    /// System-wide visibility, including unattributed records.
    Global,
    /// Records received within this organization.
    Organization(Uuid),
    /// Own records received within the current organization.
    Account {
        /// Current organization identifier.
        organization_id: Uuid,
        /// Authenticated account identifier.
        account_id: Uuid,
    },
}

impl UsageScope {
    /// Return the wire label for this visibility scope.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Organization(_) => "organization",
            Self::Account { .. } => "account",
        }
    }
}

/// Authentication snapshot consumed by inference accounting. A key's issuer is never its usage account.
#[derive(Debug, Clone)]
pub struct PrincipalAttribution {
    /// Organization identifier; null denotes unassigned or legacy usage.
    pub organization_id: Option<Uuid>,
    /// Authenticated usage account, if attributable.
    pub account_id: Option<Uuid>,
    /// Authenticated key identifier, absent for JWT or debug keys.
    pub api_key_id: Option<Uuid>,
    /// Credential source used to establish this snapshot.
    pub auth_source: String,
    /// Whether the current authenticated principal may submit inference.
    pub inference_allowed: bool,
}

fn unauthorized() -> LbError {
    LbError::Authentication("Account is unavailable".into())
}

/// Load a live account and reject missing, inactive or revoked sessions.
pub async fn current_account(
    pool: &SqlitePool,
    claims: &Claims,
) -> Result<AccountMembership, LbError> {
    let id = Uuid::parse_str(&claims.sub).map_err(|_| unauthorized())?;
    let account = organizations::account(pool, id)
        .await?
        .ok_or_else(unauthorized)?;
    if !account.is_active || claims.password_changed_at < account.password_changed_at {
        return Err(unauthorized());
    }
    Ok(account)
}

/// Resolve visibility from a current account without trusting JWT role claims.
pub fn account_scope(account: &AccountMembership) -> Result<UsageScope, LbError> {
    if account.role == UserRole::Admin {
        return Ok(UsageScope::Global);
    }
    let organization_id = account
        .organization_id
        .filter(|_| account.organization_active)
        .ok_or_else(|| {
            LbError::Authorization("An active organization membership is required".into())
        })?;
    match account.organization_role {
        Some(OrganizationRole::Admin) => Ok(UsageScope::Organization(organization_id)),
        Some(OrganizationRole::Member) => Ok(UsageScope::Account {
            organization_id,
            account_id: account.id,
        }),
        None => Err(LbError::Authorization(
            "An active organization membership is required".into(),
        )),
    }
}

/// Resolve current visibility from a verified JWT subject and the database.
pub async fn resolve_scope(pool: &SqlitePool, claims: &Claims) -> Result<UsageScope, LbError> {
    account_scope(&current_account(pool, claims).await?)
}

/// Refresh JWT system role and capture the current principal from database state.
pub async fn authenticate_jwt(
    pool: &SqlitePool,
    mut claims: Claims,
) -> Result<(Claims, PrincipalAttribution), LbError> {
    let account = current_account(pool, &claims).await?;
    claims.role = account.role;
    claims.must_change_password = account.must_change_password;
    let principal = PrincipalAttribution {
        organization_id: account
            .organization_id
            .filter(|_| account.organization_active),
        account_id: Some(account.id),
        api_key_id: None,
        auth_source: "jwt".into(),
        inference_allowed: account_scope(&account).is_ok(),
    };
    Ok((claims, principal))
}

/// Resolve key ownership and current eligibility in one SQLite read snapshot.
pub async fn authenticate_key(
    pool: &SqlitePool,
    id: Uuid,
) -> Result<PrincipalAttribution, LbError> {
    #[cfg(debug_assertions)]
    if id.is_nil() {
        return Ok(PrincipalAttribution {
            organization_id: None,
            account_id: None,
            api_key_id: None,
            auth_source: "api_key".into(),
            inference_allowed: true,
        });
    }
    #[derive(sqlx::FromRow)]
    struct KeyRow {
        usage_account_id: Option<String>,
        organization_id: Option<String>,
        attribution_revoked_at: Option<String>,
        account_organization_id: Option<String>,
        account_active: Option<bool>,
        organization_active: bool,
    }
    let row: KeyRow = sqlx::query_as("SELECT k.usage_account_id,k.organization_id,k.attribution_revoked_at,u.organization_id AS account_organization_id,u.is_active AS account_active,(o.id IS NOT NULL AND o.archived_at IS NULL) AS organization_active FROM api_keys k LEFT JOIN users u ON u.id = k.usage_account_id LEFT JOIN organizations o ON o.id = k.organization_id WHERE k.id = ?")
        .bind(id.to_string()).fetch_optional(pool).await.map_err(|error| LbError::Database(format!("Key ownership query failed: {error}")))?
        .ok_or_else(unauthorized)?;
    let (account_id, organization_id) = match (row.usage_account_id, row.organization_id) {
        (None, None) => (None, None),
        (Some(account), Some(organization))
            if row.attribution_revoked_at.is_none()
                && row.account_active == Some(true)
                && row.organization_active
                && row.account_organization_id.as_deref() == Some(organization.as_str()) =>
        {
            (
                Some(Uuid::parse_str(&account).map_err(|_| unauthorized())?),
                Some(Uuid::parse_str(&organization).map_err(|_| unauthorized())?),
            )
        }
        _ => return Err(unauthorized()),
    };
    Ok(PrincipalAttribution {
        organization_id,
        account_id,
        api_key_id: Some(id),
        auth_source: "api_key".into(),
        inference_allowed: true,
    })
}
