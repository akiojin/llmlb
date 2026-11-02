//! 組織・単一所属の永続化。

use crate::common::error::{CommonError, LbError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

/// システムのAdmin/Viewerとは独立した、組織内の閲覧ロール。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrganizationRole {
    /// 同じ組織のアカウント・使用量を閲覧できる。
    Admin,
    /// 現在の組織における自分の情報だけを閲覧できる。
    Member,
}

impl OrganizationRole {
    /// 永続化に使う固定値。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Member => "member",
        }
    }

    fn from_database(value: &str) -> Result<Self, LbError> {
        match value {
            "admin" => Ok(Self::Admin),
            "member" => Ok(Self::Member),
            _ => Err(LbError::Database("Invalid organization role".into())),
        }
    }
}

/// UUIDは改名・アーカイブ後も変わらない。
#[derive(Debug, Clone, Serialize)]
pub struct Organization {
    /// 組織ID。
    pub id: Uuid,
    /// 表示名。
    pub name: String,
    /// 作成日時。
    pub created_at: DateTime<Utc>,
    /// 最終変更日時。
    pub updated_at: DateTime<Utc>,
    /// 空の組織をアーカイブした日時。
    pub archived_at: Option<DateTime<Utc>>,
}

/// 現在の所属。履歴の帰属snapshotとは別に扱う。
#[derive(Debug, Clone, Serialize)]
pub struct Membership {
    /// 利用アカウントID。
    pub account_id: Uuid,
    /// 未所属ならNone。
    pub organization_id: Option<Uuid>,
    /// 組織内の閲覧ロール。
    pub organization_role: OrganizationRole,
    /// 次の要求から認証を拒否する状態。
    pub disabled: bool,
}

#[derive(sqlx::FromRow)]
struct OrganizationRow {
    id: String,
    name: String,
    created_at: String,
    updated_at: String,
    archived_at: Option<String>,
}

impl OrganizationRow {
    fn into_organization(self) -> Result<Organization, LbError> {
        Ok(Organization {
            id: parse_uuid(&self.id)?,
            name: self.name,
            created_at: parse_time(&self.created_at)?,
            updated_at: parse_time(&self.updated_at)?,
            archived_at: self.archived_at.as_deref().map(parse_time).transpose()?,
        })
    }
}

#[derive(sqlx::FromRow)]
struct MembershipRow {
    id: String,
    organization_id: Option<String>,
    organization_role: String,
    disabled: bool,
}

impl MembershipRow {
    fn into_membership(self) -> Result<Membership, LbError> {
        Ok(Membership {
            account_id: parse_uuid(&self.id)?,
            organization_id: self
                .organization_id
                .as_deref()
                .map(parse_uuid)
                .transpose()?,
            organization_role: OrganizationRole::from_database(&self.organization_role)?,
            disabled: self.disabled,
        })
    }
}

fn parse_uuid(value: &str) -> Result<Uuid, LbError> {
    Uuid::parse_str(value)
        .map_err(|error| LbError::Database(format!("Invalid stored UUID: {error}")))
}

fn parse_time(value: &str) -> Result<DateTime<Utc>, LbError> {
    DateTime::parse_from_rfc3339(value)
        .map(|time| time.with_timezone(&Utc))
        .map_err(|error| LbError::Database(format!("Invalid stored timestamp: {error}")))
}

fn normalized_name(name: &str) -> Result<(String, String), LbError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CommonError::Validation("Organization name must not be empty".into()).into());
    }
    Ok((name.to_owned(), name.to_lowercase()))
}

fn map_write_error(error: sqlx::Error) -> LbError {
    if let Some(database) = error.as_database_error() {
        if database.is_unique_violation() {
            return LbError::Conflict("An active organization already has this name".into());
        }
        match database.message() {
            "organization_has_members" => {
                return LbError::Conflict(
                    "Remove all organization memberships before archiving".into(),
                );
            }
            "invalid_organization_membership" => {
                return CommonError::Validation("An active organization is required".into()).into();
            }
            "unassigned_organization_admin" => {
                return CommonError::Validation(
                    "An unassigned account cannot have the organization admin role".into(),
                )
                .into();
            }
            "last_administrator" => {
                return CommonError::Validation(
                    "Cannot remove, demote, or disable the last enabled administrator".into(),
                )
                .into();
            }
            _ => {}
        }
    }
    LbError::Database(format!("Organization write failed: {error}"))
}

/// 新しい組織を作成する。空白を除いた名前を大小文字を区別せず一意にする。
pub async fn create(pool: &SqlitePool, name: &str) -> Result<Organization, LbError> {
    let (name, normalized) = normalized_name(name)?;
    let now = Utc::now().to_rfc3339();
    sqlx::query_as::<_, OrganizationRow>(
        "INSERT INTO organizations (id, name, normalized_name, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?)
         RETURNING id, name, created_at, updated_at, archived_at",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(name)
    .bind(normalized)
    .bind(&now)
    .bind(&now)
    .fetch_one(pool)
    .await
    .map_err(map_write_error)?
    .into_organization()
}

/// アーカイブ済みを含めて組織を取得する。公開API側で認可scopeを適用する。
pub async fn list(pool: &SqlitePool) -> Result<Vec<Organization>, LbError> {
    sqlx::query_as::<_, OrganizationRow>(
        "SELECT id, name, created_at, updated_at, archived_at
         FROM organizations ORDER BY normalized_name, id",
    )
    .fetch_all(pool)
    .await
    .map_err(|error| LbError::Database(format!("Failed to list organizations: {error}")))?
    .into_iter()
    .map(OrganizationRow::into_organization)
    .collect()
}

/// immutableな組織IDで検索する。
pub async fn find(pool: &SqlitePool, id: Uuid) -> Result<Option<Organization>, LbError> {
    sqlx::query_as::<_, OrganizationRow>(
        "SELECT id, name, created_at, updated_at, archived_at FROM organizations WHERE id = ?",
    )
    .bind(id.to_string())
    .fetch_optional(pool)
    .await
    .map_err(|error| LbError::Database(format!("Failed to find organization: {error}")))?
    .map(OrganizationRow::into_organization)
    .transpose()
}

/// 有効な組織を改名する。過去の帰属IDは変更しない。
pub async fn rename(pool: &SqlitePool, id: Uuid, name: &str) -> Result<Organization, LbError> {
    let (name, normalized) = normalized_name(name)?;
    sqlx::query_as::<_, OrganizationRow>(
        "UPDATE organizations SET name = ?, normalized_name = ?, updated_at = ?
         WHERE id = ? AND archived_at IS NULL
         RETURNING id, name, created_at, updated_at, archived_at",
    )
    .bind(name)
    .bind(normalized)
    .bind(Utc::now().to_rfc3339())
    .bind(id.to_string())
    .fetch_optional(pool)
    .await
    .map_err(map_write_error)?
    .ok_or_else(|| LbError::NotFound("Active organization not found".into()))?
    .into_organization()
}

/// 所属アカウントが一人もいない組織だけをアーカイブする。
pub async fn archive(pool: &SqlitePool, id: Uuid) -> Result<Organization, LbError> {
    let now = Utc::now().to_rfc3339();
    sqlx::query_as::<_, OrganizationRow>(
        "UPDATE organizations SET archived_at = COALESCE(archived_at, ?), updated_at = ?
         WHERE id = ? RETURNING id, name, created_at, updated_at, archived_at",
    )
    .bind(&now)
    .bind(&now)
    .bind(id.to_string())
    .fetch_optional(pool)
    .await
    .map_err(map_write_error)?
    .ok_or_else(|| LbError::NotFound("Organization not found".into()))?
    .into_organization()
}

/// 現在の所属と無効化状態をDBから取得する。
pub async fn membership(
    pool: &SqlitePool,
    account_id: Uuid,
) -> Result<Option<Membership>, LbError> {
    sqlx::query_as::<_, MembershipRow>(
        "SELECT id, organization_id, organization_role, disabled FROM users WHERE id = ?",
    )
    .bind(account_id.to_string())
    .fetch_optional(pool)
    .await
    .map_err(|error| LbError::Database(format!("Failed to read membership: {error}")))?
    .map(MembershipRow::into_membership)
    .transpose()
}

/// 単一所属を変更する。キーや過去要求の帰属は変更しない。
pub async fn set_membership(
    pool: &SqlitePool,
    account_id: Uuid,
    organization_id: Option<Uuid>,
    role: OrganizationRole,
) -> Result<Membership, LbError> {
    if organization_id.is_none() && role != OrganizationRole::Member {
        return Err(CommonError::Validation(
            "An unassigned account cannot have the organization admin role".into(),
        )
        .into());
    }
    sqlx::query_as::<_, MembershipRow>(
        "UPDATE users SET organization_id = ?, organization_role = ? WHERE id = ?
         RETURNING id, organization_id, organization_role, disabled",
    )
    .bind(organization_id.map(|id| id.to_string()))
    .bind(role.as_str())
    .bind(account_id.to_string())
    .fetch_optional(pool)
    .await
    .map_err(map_write_error)?
    .ok_or_else(|| LbError::NotFound("Account not found".into()))?
    .into_membership()
}

/// 有効/無効を変更し、旧JWTが再有効化後に復活しないよう失効時刻を進める。
pub async fn set_disabled(
    pool: &SqlitePool,
    account_id: Uuid,
    disabled: bool,
) -> Result<Membership, LbError> {
    sqlx::query_as::<_, MembershipRow>(
        "UPDATE users SET
           password_changed_at = CASE WHEN disabled != ?
             THEN MAX(password_changed_at + 1, ?) ELSE password_changed_at END,
           disabled = ?
         WHERE id = ? RETURNING id, organization_id, organization_role, disabled",
    )
    .bind(disabled)
    .bind(Utc::now().timestamp_millis())
    .bind(disabled)
    .bind(account_id.to_string())
    .fetch_optional(pool)
    .await
    .map_err(map_write_error)?
    .ok_or_else(|| LbError::NotFound("Account not found".into()))?
    .into_membership()
}

#[cfg(test)]
mod tests;
