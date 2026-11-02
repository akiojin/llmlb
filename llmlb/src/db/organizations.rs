//! Organization identities, current membership and immutable API-key ownership.

use crate::auth::scope::UsageScope;
use crate::common::{
    auth::UserRole,
    error::{CommonError, LbError},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use uuid::Uuid;

/// Fixed organization-local roles; these do not grant system administration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrganizationRole {
    /// Read the organization accounts and usage without system admin powers.
    Admin,
    /// Read only own accounts and usage within the organization.
    Member,
}

impl OrganizationRole {
    /// Return the stable database and wire spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Member => "member",
        }
    }
}

/// Organization metadata including an optional archive marker.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Organization {
    /// Immutable identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Creation time in UTC.
    pub created_at: String,
    /// Last modification time in UTC.
    pub updated_at: String,
    /// Archive time, or null for an active organization.
    pub archived_at: Option<String>,
}

/// Current account membership and authentication state.
#[derive(Debug, Clone, Serialize)]
pub struct AccountMembership {
    /// Immutable identifier.
    pub id: Uuid,
    /// Account login name.
    pub username: String,
    /// System role, independent of organization membership.
    pub role: UserRole,
    /// Optional notification address.
    pub email: Option<String>,
    /// Organization identifier; null denotes unassigned or legacy usage.
    pub organization_id: Option<Uuid>,
    /// Role inside the current organization.
    pub organization_role: Option<OrganizationRole>,
    /// Whether the account can authenticate.
    pub is_active: bool,
    #[serde(skip)]
    /// Whether the current organization exists and is not archived.
    pub organization_active: bool,
    #[serde(skip)]
    /// Current session revocation epoch in milliseconds.
    pub password_changed_at: i64,
    #[serde(skip)]
    /// Whether password change is required.
    pub must_change_password: bool,
}

#[derive(sqlx::FromRow)]
struct AccountRow {
    id: String,
    username: String,
    role: String,
    email: Option<String>,
    organization_id: Option<String>,
    organization_role: Option<String>,
    is_active: bool,
    organization_active: bool,
    password_changed_at: i64,
    must_change_password: bool,
}

const ACCOUNT_SELECT: &str = "SELECT u.id, u.username, u.role, u.email, u.organization_id, u.organization_role, u.is_active, u.password_changed_at, u.must_change_password, (o.id IS NOT NULL AND o.archived_at IS NULL) AS organization_active FROM users u LEFT JOIN organizations o ON o.id = u.organization_id";

fn database(error: sqlx::Error) -> LbError {
    LbError::Database(format!("Organization query failed: {error}"))
}
fn validation(message: &str) -> LbError {
    LbError::Common(CommonError::Validation(message.into()))
}
fn parse_uuid(raw: &str) -> Result<Uuid, LbError> {
    Uuid::parse_str(raw).map_err(|_| LbError::Database("Invalid membership identifier".into()))
}

impl AccountRow {
    fn into_membership(self) -> Result<AccountMembership, LbError> {
        let role = match self.role.as_str() {
            "admin" => UserRole::Admin,
            "viewer" => UserRole::Viewer,
            _ => return Err(LbError::Database("Invalid system role".into())),
        };
        let organization_role = match self.organization_role.as_deref() {
            Some("admin") => Some(OrganizationRole::Admin),
            Some("member") => Some(OrganizationRole::Member),
            None => None,
            _ => return Err(LbError::Database("Invalid organization role".into())),
        };
        Ok(AccountMembership {
            id: parse_uuid(&self.id)?,
            username: self.username,
            role,
            email: self.email,
            organization_id: self
                .organization_id
                .as_deref()
                .map(parse_uuid)
                .transpose()?,
            organization_role,
            is_active: self.is_active,
            organization_active: self.organization_active,
            password_changed_at: self.password_changed_at,
            must_change_password: self.must_change_password,
        })
    }
}

/// Find the current account and organization state by immutable account identifier.
pub async fn account(pool: &SqlitePool, id: Uuid) -> Result<Option<AccountMembership>, LbError> {
    sqlx::query_as::<_, AccountRow>(&format!("{ACCOUNT_SELECT} WHERE u.id = ?"))
        .bind(id.to_string())
        .fetch_optional(pool)
        .await
        .map_err(database)?
        .map(AccountRow::into_membership)
        .transpose()
}

/// List only organizations inside the authorized scope.
pub async fn list(pool: &SqlitePool, scope: &UsageScope) -> Result<Vec<Organization>, LbError> {
    let mut query = QueryBuilder::<Sqlite>::new(
        "SELECT id, name, created_at, updated_at, archived_at FROM organizations",
    );
    match scope {
        UsageScope::Global => {}
        UsageScope::Organization(id)
        | UsageScope::Account {
            organization_id: id,
            ..
        } => {
            query
                .push(" WHERE archived_at IS NULL AND id = ")
                .push_bind(id.to_string());
        }
    }
    query.push(" ORDER BY created_at DESC, id");
    query
        .build_query_as()
        .fetch_all(pool)
        .await
        .map_err(database)
}

fn normalized_name(name: &str) -> Result<(&str, String), LbError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 128 || name.chars().any(char::is_control) {
        return Err(validation(
            "Organization name must contain 1 to 128 printable characters",
        ));
    }
    Ok((name, name.to_lowercase()))
}

fn write_error(error: sqlx::Error) -> LbError {
    if error
        .as_database_error()
        .is_some_and(|e| e.is_unique_violation())
    {
        LbError::Conflict("Organization name already exists".into())
    } else {
        super::registration_requests::map_write_error(error)
    }
}

/// Create an organization with a unique normalized active name.
pub async fn create(pool: &SqlitePool, name: &str) -> Result<Organization, LbError> {
    let (name, key) = normalized_name(name)?;
    let now = Utc::now().to_rfc3339();
    let organization = Organization {
        id: Uuid::new_v4().to_string(),
        name: name.into(),
        created_at: now.clone(),
        updated_at: now,
        archived_at: None,
    };
    sqlx::query(
        "INSERT INTO organizations (id,name,name_key,created_at,updated_at) VALUES (?,?,?,?,?)",
    )
    .bind(&organization.id)
    .bind(&organization.name)
    .bind(key)
    .bind(&organization.created_at)
    .bind(&organization.updated_at)
    .execute(pool)
    .await
    .map_err(write_error)?;
    Ok(organization)
}

/// Rename an active organization without changing its identifier.
pub async fn rename(pool: &SqlitePool, id: Uuid, name: &str) -> Result<Organization, LbError> {
    let (name, key) = normalized_name(name)?;
    sqlx::query_as::<_, Organization>("UPDATE organizations SET name = ?, name_key = ?, updated_at = ? WHERE id = ? AND archived_at IS NULL RETURNING id,name,created_at,updated_at,archived_at")
        .bind(name).bind(key).bind(Utc::now().to_rfc3339()).bind(id.to_string())
        .fetch_optional(pool).await.map_err(write_error)?
        .ok_or_else(|| LbError::NotFound("Organization not found".into()))
}

/// Archive an empty organization while holding the SQLite write transaction.
pub async fn archive(pool: &SqlitePool, id: Uuid) -> Result<(), LbError> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.map_err(database)?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM organizations WHERE id = ? AND archived_at IS NULL)",
    )
    .bind(id.to_string())
    .fetch_one(&mut *transaction)
    .await
    .map_err(database)?;
    if !exists {
        return Err(LbError::NotFound("Organization not found".into()));
    }
    let occupied: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE organization_id = ?)")
            .bind(id.to_string())
            .fetch_one(&mut *transaction)
            .await
            .map_err(database)?;
    if occupied {
        return Err(LbError::Conflict("Organization still has accounts".into()));
    }
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE organizations SET archived_at = ?, updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&now)
        .bind(id.to_string())
        .execute(&mut *transaction)
        .await
        .map_err(database)?;
    transaction.commit().await.map_err(database)
}

/// Atomically validate and update one account membership and active state.
pub async fn set_membership(
    pool: &SqlitePool,
    id: Uuid,
    organization_id: Option<Uuid>,
    organization_role: OrganizationRole,
    is_active: Option<bool>,
) -> Result<AccountMembership, LbError> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.map_err(database)?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = ?)")
        .bind(id.to_string())
        .fetch_one(&mut *transaction)
        .await
        .map_err(database)?;
    if !exists {
        return Err(LbError::NotFound("Account not found".into()));
    }
    if let Some(organization_id) = organization_id {
        let active: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM organizations WHERE id = ? AND archived_at IS NULL)",
        )
        .bind(organization_id.to_string())
        .fetch_one(&mut *transaction)
        .await
        .map_err(database)?;
        if !active {
            return Err(validation("An active organization is required"));
        }
    }
    sqlx::query("UPDATE users SET organization_id = ?, organization_role = ?, is_active = COALESCE(?, is_active) WHERE id = ?")
        .bind(organization_id.map(|id| id.to_string())).bind(organization_id.map(|_| organization_role.as_str()))
        .bind(is_active).bind(id.to_string()).execute(&mut *transaction).await.map_err(write_error)?;
    let row = sqlx::query_as::<_, AccountRow>(&format!("{ACCOUNT_SELECT} WHERE u.id = ?"))
        .bind(id.to_string())
        .fetch_one(&mut *transaction)
        .await
        .map_err(database)?;
    transaction.commit().await.map_err(database)?;
    row.into_membership()
}

/// Read authorized organization members with scope applied before fetching rows.
pub async fn members(
    pool: &SqlitePool,
    organization_id: Uuid,
    scope: &UsageScope,
) -> Result<Vec<AccountMembership>, LbError> {
    let account_id = match scope {
        UsageScope::Global => None,
        UsageScope::Organization(id) if *id == organization_id => None,
        UsageScope::Account {
            organization_id: id,
            account_id,
        } if *id == organization_id => Some(*account_id),
        _ => return Err(LbError::NotFound("Organization not found".into())),
    };
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM organizations WHERE id = ?)")
            .bind(organization_id.to_string())
            .fetch_one(pool)
            .await
            .map_err(database)?;
    if !exists {
        return Err(LbError::NotFound("Organization not found".into()));
    }
    let mut query = QueryBuilder::<Sqlite>::new(ACCOUNT_SELECT);
    query
        .push(" WHERE u.organization_id = ")
        .push_bind(organization_id.to_string());
    if let Some(account_id) = account_id {
        query.push(" AND u.id = ").push_bind(account_id.to_string());
    }
    query.push(" ORDER BY u.username, u.id");
    query
        .build_query_as::<AccountRow>()
        .fetch_all(pool)
        .await
        .map_err(database)?
        .into_iter()
        .map(AccountRow::into_membership)
        .collect()
}

/// Immutable key usage ownership with its permanent revocation marker.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct KeyAttribution {
    /// Usage account, distinct from the key issuer.
    pub usage_account_id: Option<String>,
    /// Organization identifier; null denotes unassigned or legacy usage.
    pub organization_id: Option<String>,
    /// Permanent ownership revocation time after a transfer or deactivation.
    pub attribution_revoked_at: Option<String>,
}

/// Read a stored key attribution without inferring it from its issuer.
pub async fn key_attribution(pool: &SqlitePool, id: Uuid) -> Result<KeyAttribution, LbError> {
    sqlx::query_as(
        "SELECT usage_account_id,organization_id,attribution_revoked_at FROM api_keys WHERE id = ?",
    )
    .bind(id.to_string())
    .fetch_optional(pool)
    .await
    .map_err(database)?
    .ok_or_else(|| LbError::NotFound("API key not found".into()))
}

#[cfg(test)]
mod tests;
