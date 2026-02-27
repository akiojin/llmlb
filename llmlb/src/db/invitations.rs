// 招待コードCRUD操作

use crate::common::error::{CommonError, LbError};
use chrono::{DateTime, Duration, Utc};
use rand::RngExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use uuid::Uuid;

/// 招待コードのステータス
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvitationStatus {
    /// 有効
    Active,
    /// 使用済み
    Used,
    /// 無効化済み
    Revoked,
}

impl std::fmt::Display for InvitationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InvitationStatus::Active => write!(f, "active"),
            InvitationStatus::Used => write!(f, "used"),
            InvitationStatus::Revoked => write!(f, "revoked"),
        }
    }
}

impl std::str::FromStr for InvitationStatus {
    type Err = LbError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(InvitationStatus::Active),
            "used" => Ok(InvitationStatus::Used),
            "revoked" => Ok(InvitationStatus::Revoked),
            _ => Err(CommonError::Validation(format!("Invalid invitation status: {}", s)).into()),
        }
    }
}

/// 招待コード（DBから取得）
#[derive(Debug, Clone, Serialize)]
pub struct InvitationCode {
    /// 招待コードID
    pub id: Uuid,
    /// SHA-256ハッシュ
    pub code_hash: String,
    /// 発行者のユーザーID
    pub created_by: Uuid,
    /// 作成日時
    pub created_at: DateTime<Utc>,
    /// 有効期限
    pub expires_at: DateTime<Utc>,
    /// ステータス
    pub status: InvitationStatus,
    /// 使用者のユーザーID
    pub used_by: Option<Uuid>,
    /// 使用日時
    pub used_at: Option<DateTime<Utc>>,
}

/// 招待コード（平文コード含む、作成時のみ）
#[derive(Debug, Clone, Serialize)]
pub struct InvitationCodeWithPlaintext {
    /// 招待コードID
    pub id: Uuid,
    /// 平文の招待コード（発行時のみ表示）
    pub code: String,
    /// 作成日時
    pub created_at: DateTime<Utc>,
    /// 有効期限
    pub expires_at: DateTime<Utc>,
}

/// 招待コードを生成
///
/// # Arguments
/// * `pool` - データベース接続プール
/// * `created_by` - 発行したユーザーID
/// * `expires_in_hours` - 有効期限（時間）、デフォルト72時間
///
/// # Returns
/// * `Ok(InvitationCodeWithPlaintext)` - 生成された招待コード（平文コード含む）
/// * `Err(LbError)` - 生成失敗
pub async fn create(
    pool: &SqlitePool,
    created_by: Uuid,
    expires_in_hours: Option<i64>,
) -> Result<InvitationCodeWithPlaintext, LbError> {
    let id = Uuid::new_v4();
    let code = generate_invitation_code();
    let code_hash = hash_with_sha256(&code);
    let created_at = Utc::now();
    let expires_at = created_at + Duration::hours(expires_in_hours.unwrap_or(72));

    sqlx::query(
        "INSERT INTO invitation_codes (id, code_hash, created_by, created_at, expires_at, status)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(id.to_string())
    .bind(&code_hash)
    .bind(created_by.to_string())
    .bind(created_at.to_rfc3339())
    .bind(expires_at.to_rfc3339())
    .bind("active")
    .execute(pool)
    .await
    .map_err(|e| LbError::Database(format!("Failed to create invitation code: {}", e)))?;

    Ok(InvitationCodeWithPlaintext {
        id,
        code,
        created_at,
        expires_at,
    })
}

/// ハッシュ値で招待コードを検索
///
/// # Arguments
/// * `pool` - データベース接続プール
/// * `code_hash` - SHA-256ハッシュ
///
/// # Returns
/// * `Ok(Some(InvitationCode))` - 招待コードが見つかった
/// * `Ok(None)` - 招待コードが見つからなかった
/// * `Err(LbError)` - 検索失敗
pub async fn find_by_hash(
    pool: &SqlitePool,
    code_hash: &str,
) -> Result<Option<InvitationCode>, LbError> {
    let row = sqlx::query_as::<_, InvitationCodeRow>(
        "SELECT id, code_hash, created_by, created_at, expires_at, status, used_by, used_at
         FROM invitation_codes WHERE code_hash = ?",
    )
    .bind(code_hash)
    .fetch_optional(pool)
    .await
    .map_err(|e| LbError::Database(format!("Failed to find invitation code: {}", e)))?;

    row.map(|r| r.try_into_invitation_code()).transpose()
}

/// 平文コードから招待コードを検索
///
/// # Arguments
/// * `pool` - データベース接続プール
/// * `plaintext_code` - 平文の招待コード
///
/// # Returns
/// * `Ok(Some(InvitationCode))` - 招待コードが見つかった
/// * `Ok(None)` - 招待コードが見つからなかった
/// * `Err(LbError)` - 検索失敗
pub async fn find_by_code(
    pool: &SqlitePool,
    plaintext_code: &str,
) -> Result<Option<InvitationCode>, LbError> {
    let code_hash = hash_with_sha256(plaintext_code);
    find_by_hash(pool, &code_hash).await
}

/// すべての招待コードを取得
///
/// # Arguments
/// * `pool` - データベース接続プール
///
/// # Returns
/// * `Ok(Vec<InvitationCode>)` - 招待コード一覧
/// * `Err(LbError)` - 取得失敗
pub async fn list(pool: &SqlitePool) -> Result<Vec<InvitationCode>, LbError> {
    let rows = sqlx::query_as::<_, InvitationCodeRow>(
        "SELECT id, code_hash, created_by, created_at, expires_at, status, used_by, used_at
         FROM invitation_codes ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| LbError::Database(format!("Failed to list invitation codes: {}", e)))?;

    rows.into_iter()
        .map(|r| r.try_into_invitation_code())
        .collect()
}

/// 招待コードを使用済みにする
///
/// # Arguments
/// * `pool` - データベース接続プール
/// * `id` - 招待コードID
/// * `used_by` - 使用したユーザーID
///
/// # Returns
/// * `Ok(())` - 更新成功
/// * `Err(LbError)` - 更新失敗
pub async fn mark_as_used(pool: &SqlitePool, id: Uuid, used_by: Uuid) -> Result<(), LbError> {
    let used_at = Utc::now();

    let result = sqlx::query(
        "UPDATE invitation_codes SET status = ?, used_by = ?, used_at = ? WHERE id = ? AND status = ?",
    )
    .bind("used")
    .bind(used_by.to_string())
    .bind(used_at.to_rfc3339())
    .bind(id.to_string())
    .bind("active")
    .execute(pool)
    .await
    .map_err(|e| LbError::Database(format!("Failed to mark invitation as used: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(CommonError::Validation(
            "Invitation code is not active or does not exist".to_string(),
        )
        .into());
    }

    Ok(())
}

/// 招待コードを無効化
///
/// # Arguments
/// * `pool` - データベース接続プール
/// * `id` - 招待コードID
///
/// # Returns
/// * `Ok(bool)` - 無効化成功したらtrue、見つからなければfalse
/// * `Err(LbError)` - 無効化失敗
pub async fn revoke(pool: &SqlitePool, id: Uuid) -> Result<bool, LbError> {
    let result = sqlx::query("UPDATE invitation_codes SET status = ? WHERE id = ? AND status = ?")
        .bind("revoked")
        .bind(id.to_string())
        .bind("active")
        .execute(pool)
        .await
        .map_err(|e| LbError::Database(format!("Failed to revoke invitation code: {}", e)))?;

    Ok(result.rows_affected() > 0)
}

/// 招待コードを削除
///
/// # Arguments
/// * `pool` - データベース接続プール
/// * `id` - 招待コードID
///
/// # Returns
/// * `Ok(())` - 削除成功
/// * `Err(LbError)` - 削除失敗
pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<(), LbError> {
    sqlx::query("DELETE FROM invitation_codes WHERE id = ?")
        .bind(id.to_string())
        .execute(pool)
        .await
        .map_err(|e| LbError::Database(format!("Failed to delete invitation code: {}", e)))?;

    Ok(())
}

/// 招待コードが有効かチェック
///
/// # Arguments
/// * `invitation` - 招待コード
///
/// # Returns
/// * `Ok(())` - 有効
/// * `Err(LbError)` - 無効（期限切れ、使用済み、無効化済み）
pub fn validate_invitation(invitation: &InvitationCode) -> Result<(), LbError> {
    // ステータスチェック
    if invitation.status != InvitationStatus::Active {
        return Err(
            CommonError::Validation(format!("Invitation code is {}", invitation.status)).into(),
        );
    }

    // 有効期限チェック
    if invitation.expires_at < Utc::now() {
        return Err(CommonError::Validation("Invitation code has expired".to_string()).into());
    }

    Ok(())
}

/// 招待コードを生成（`inv_` + 16文字のランダム英数字）
fn generate_invitation_code() -> String {
    let charset: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::rng();

    let random_part: String = (0..16)
        .map(|_| {
            let idx = rng.random_range(0..charset.len());
            charset[idx] as char
        })
        .collect();

    format!("inv_{}", random_part)
}

/// SHA-256ハッシュ化
pub fn hash_with_sha256(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
}

// SQLiteからの行取得用の内部型
#[derive(sqlx::FromRow)]
struct InvitationCodeRow {
    id: String,
    code_hash: String,
    created_by: String,
    created_at: String,
    expires_at: String,
    status: String,
    used_by: Option<String>,
    used_at: Option<String>,
}

impl InvitationCodeRow {
    fn try_into_invitation_code(self) -> Result<InvitationCode, LbError> {
        let id = Uuid::parse_str(&self.id)
            .map_err(|e| LbError::Database(format!("Invalid UUID: {}", e)))?;
        let created_by = Uuid::parse_str(&self.created_by)
            .map_err(|e| LbError::Database(format!("Invalid created_by UUID: {}", e)))?;
        let created_at = DateTime::parse_from_rfc3339(&self.created_at)
            .map_err(|e| LbError::Database(format!("Invalid created_at: {}", e)))?
            .with_timezone(&Utc);
        let expires_at = DateTime::parse_from_rfc3339(&self.expires_at)
            .map_err(|e| LbError::Database(format!("Invalid expires_at: {}", e)))?
            .with_timezone(&Utc);
        let status: InvitationStatus = self.status.parse()?;
        let used_by = self
            .used_by
            .as_ref()
            .map(|s| Uuid::parse_str(s))
            .transpose()
            .map_err(|e| LbError::Database(format!("Invalid used_by UUID: {}", e)))?;
        let used_at = self
            .used_at
            .as_ref()
            .map(|s| {
                DateTime::parse_from_rfc3339(s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| LbError::Database(format!("Invalid used_at: {}", e)))
            })
            .transpose()?;

        Ok(InvitationCode {
            id,
            code_hash: self.code_hash,
            created_by,
            created_at,
            expires_at,
            status,
            used_by,
            used_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::auth::UserRole;
    use crate::db::users;

    async fn setup_test_db() -> SqlitePool {
        crate::db::test_utils::test_db_pool().await
    }

    #[tokio::test]
    async fn test_generate_invitation_code() {
        let code = generate_invitation_code();
        assert!(code.starts_with("inv_"));
        assert_eq!(code.len(), 4 + 16); // "inv_" + 16文字
    }

    #[tokio::test]
    async fn test_create_and_find_invitation() {
        let pool = setup_test_db().await;

        // テスト用ユーザーを作成
        let user = users::create(&pool, "admin", "hash", UserRole::Admin, false)
            .await
            .unwrap();

        // 招待コードを作成
        let invitation = create(&pool, user.id, None).await.unwrap();

        assert!(invitation.code.starts_with("inv_"));

        // 平文コードで検索
        let found = find_by_code(&pool, &invitation.code).await.unwrap();
        assert!(found.is_some());

        let found = found.unwrap();
        assert_eq!(found.id, invitation.id);
        assert_eq!(found.status, InvitationStatus::Active);
        assert_eq!(found.created_by, user.id);
    }

    #[tokio::test]
    async fn test_mark_as_used() {
        let pool = setup_test_db().await;

        let admin = users::create(&pool, "admin", "hash", UserRole::Admin, false)
            .await
            .unwrap();

        let invitation = create(&pool, admin.id, None).await.unwrap();

        // 新しいユーザーを作成して招待コードを使用
        let new_user = users::create(&pool, "newuser", "hash", UserRole::Viewer, false)
            .await
            .unwrap();

        mark_as_used(&pool, invitation.id, new_user.id)
            .await
            .unwrap();

        // 再度検索してステータスを確認
        let found = find_by_code(&pool, &invitation.code)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.status, InvitationStatus::Used);
        assert_eq!(found.used_by, Some(new_user.id));
        assert!(found.used_at.is_some());
    }

    #[tokio::test]
    async fn test_cannot_use_twice() {
        let pool = setup_test_db().await;

        let admin = users::create(&pool, "admin", "hash", UserRole::Admin, false)
            .await
            .unwrap();

        let invitation = create(&pool, admin.id, None).await.unwrap();

        let user1 = users::create(&pool, "user1", "hash", UserRole::Viewer, false)
            .await
            .unwrap();
        let user2 = users::create(&pool, "user2", "hash", UserRole::Viewer, false)
            .await
            .unwrap();

        // 1回目は成功
        mark_as_used(&pool, invitation.id, user1.id).await.unwrap();

        // 2回目は失敗
        let result = mark_as_used(&pool, invitation.id, user2.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_revoke_invitation() {
        let pool = setup_test_db().await;

        let admin = users::create(&pool, "admin", "hash", UserRole::Admin, false)
            .await
            .unwrap();

        let invitation = create(&pool, admin.id, None).await.unwrap();

        // 無効化
        let revoked = revoke(&pool, invitation.id).await.unwrap();
        assert!(revoked);

        // ステータス確認
        let found = find_by_code(&pool, &invitation.code)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.status, InvitationStatus::Revoked);

        // 無効化済みは使用不可
        let user = users::create(&pool, "user", "hash", UserRole::Viewer, false)
            .await
            .unwrap();
        let result = mark_as_used(&pool, invitation.id, user.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_validate_expired_invitation() {
        let invitation = InvitationCode {
            id: Uuid::new_v4(),
            code_hash: "hash".to_string(),
            created_by: Uuid::new_v4(),
            created_at: Utc::now() - Duration::hours(100),
            expires_at: Utc::now() - Duration::hours(1), // 1時間前に期限切れ
            status: InvitationStatus::Active,
            used_by: None,
            used_at: None,
        };

        let result = validate_invitation(&invitation);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("expired"));
    }

    #[tokio::test]
    async fn test_list_invitations() {
        let pool = setup_test_db().await;

        let admin = users::create(&pool, "admin", "hash", UserRole::Admin, false)
            .await
            .unwrap();

        create(&pool, admin.id, None).await.unwrap();
        create(&pool, admin.id, Some(24)).await.unwrap();

        let invitations = list(&pool).await.unwrap();
        assert_eq!(invitations.len(), 2);
    }

    // --- Additional tests ---

    #[test]
    fn test_invitation_status_display() {
        assert_eq!(InvitationStatus::Active.to_string(), "active");
        assert_eq!(InvitationStatus::Used.to_string(), "used");
        assert_eq!(InvitationStatus::Revoked.to_string(), "revoked");
    }

    #[test]
    fn test_invitation_status_from_str() {
        assert_eq!(
            "active".parse::<InvitationStatus>().unwrap(),
            InvitationStatus::Active
        );
        assert_eq!(
            "used".parse::<InvitationStatus>().unwrap(),
            InvitationStatus::Used
        );
        assert_eq!(
            "revoked".parse::<InvitationStatus>().unwrap(),
            InvitationStatus::Revoked
        );
        assert!("invalid".parse::<InvitationStatus>().is_err());
    }

    #[test]
    fn test_hash_with_sha256_deterministic() {
        let h1 = hash_with_sha256("test_code");
        let h2 = hash_with_sha256("test_code");
        assert_eq!(h1, h2);
        // Different input produces different hash
        let h3 = hash_with_sha256("other_code");
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_validate_used_invitation() {
        let invitation = InvitationCode {
            id: Uuid::new_v4(),
            code_hash: "hash".to_string(),
            created_by: Uuid::new_v4(),
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::hours(72),
            status: InvitationStatus::Used,
            used_by: Some(Uuid::new_v4()),
            used_at: Some(Utc::now()),
        };
        let result = validate_invitation(&invitation);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("used"));
    }

    #[test]
    fn test_validate_revoked_invitation() {
        let invitation = InvitationCode {
            id: Uuid::new_v4(),
            code_hash: "hash".to_string(),
            created_by: Uuid::new_v4(),
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::hours(72),
            status: InvitationStatus::Revoked,
            used_by: None,
            used_at: None,
        };
        let result = validate_invitation(&invitation);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("revoked"));
    }

    #[test]
    fn test_validate_active_not_expired_invitation() {
        let invitation = InvitationCode {
            id: Uuid::new_v4(),
            code_hash: "hash".to_string(),
            created_by: Uuid::new_v4(),
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::hours(72),
            status: InvitationStatus::Active,
            used_by: None,
            used_at: None,
        };
        assert!(validate_invitation(&invitation).is_ok());
    }

    #[tokio::test]
    async fn test_delete_invitation() {
        let pool = setup_test_db().await;

        let admin = users::create(&pool, "admin", "hash", UserRole::Admin, false)
            .await
            .unwrap();

        let invitation = create(&pool, admin.id, None).await.unwrap();

        // Delete the invitation
        delete(&pool, invitation.id).await.unwrap();

        // Should no longer be found
        let found = find_by_code(&pool, &invitation.code).await.unwrap();
        assert!(found.is_none());

        // List should be empty
        let all = list(&pool).await.unwrap();
        assert!(all.is_empty());
    }

    #[tokio::test]
    async fn test_find_by_hash_not_found() {
        let pool = setup_test_db().await;

        let found = find_by_hash(&pool, "nonexistent_hash").await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_revoke_already_used_invitation_fails() {
        let pool = setup_test_db().await;

        let admin = users::create(&pool, "admin", "hash", UserRole::Admin, false)
            .await
            .unwrap();

        let invitation = create(&pool, admin.id, None).await.unwrap();

        // Use the invitation first
        let user = users::create(&pool, "user", "hash", UserRole::Viewer, false)
            .await
            .unwrap();
        mark_as_used(&pool, invitation.id, user.id).await.unwrap();

        // Revoking a used invitation should return false (no rows affected)
        let revoked = revoke(&pool, invitation.id).await.unwrap();
        assert!(!revoked);
    }

    // =====================================================================
    // 追加テスト: generate_invitation_code
    // =====================================================================

    #[test]
    fn test_generate_invitation_code_format() {
        let code = generate_invitation_code();
        assert!(code.starts_with("inv_"));
        assert_eq!(code.len(), 20); // "inv_" (4) + 16 chars

        // All characters after prefix are alphanumeric
        let suffix = &code[4..];
        assert!(suffix.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_generate_invitation_code_uniqueness() {
        let code1 = generate_invitation_code();
        let code2 = generate_invitation_code();
        assert_ne!(code1, code2);
    }

    // =====================================================================
    // 追加テスト: hash_with_sha256
    // =====================================================================

    #[test]
    fn test_hash_with_sha256_length() {
        let hash = hash_with_sha256("test");
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_hash_with_sha256_hex_chars() {
        let hash = hash_with_sha256("test");
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_hash_with_sha256_empty_input() {
        let hash = hash_with_sha256("");
        assert_eq!(hash.len(), 64);
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    // =====================================================================
    // 追加テスト: InvitationStatus serde
    // =====================================================================

    #[test]
    fn test_invitation_status_serialize() {
        let json = serde_json::to_string(&InvitationStatus::Active).unwrap();
        assert_eq!(json, "\"active\"");

        let json = serde_json::to_string(&InvitationStatus::Used).unwrap();
        assert_eq!(json, "\"used\"");

        let json = serde_json::to_string(&InvitationStatus::Revoked).unwrap();
        assert_eq!(json, "\"revoked\"");
    }

    #[test]
    fn test_invitation_status_deserialize() {
        let active: InvitationStatus = serde_json::from_str("\"active\"").unwrap();
        assert_eq!(active, InvitationStatus::Active);

        let used: InvitationStatus = serde_json::from_str("\"used\"").unwrap();
        assert_eq!(used, InvitationStatus::Used);

        let revoked: InvitationStatus = serde_json::from_str("\"revoked\"").unwrap();
        assert_eq!(revoked, InvitationStatus::Revoked);
    }

    #[test]
    fn test_invitation_status_from_str_case_sensitive() {
        // Must be lowercase
        assert!("Active".parse::<InvitationStatus>().is_err());
        assert!("ACTIVE".parse::<InvitationStatus>().is_err());
        assert!("USED".parse::<InvitationStatus>().is_err());
    }

    // =====================================================================
    // 追加テスト: validate_invitation edge cases
    // =====================================================================

    #[test]
    fn test_validate_invitation_just_expired() {
        // Exactly at expiry boundary (already expired)
        let invitation = InvitationCode {
            id: Uuid::new_v4(),
            code_hash: "hash".to_string(),
            created_by: Uuid::new_v4(),
            created_at: Utc::now() - Duration::hours(73),
            expires_at: Utc::now() - Duration::seconds(1),
            status: InvitationStatus::Active,
            used_by: None,
            used_at: None,
        };
        let result = validate_invitation(&invitation);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_invitation_far_future_expiry() {
        let invitation = InvitationCode {
            id: Uuid::new_v4(),
            code_hash: "hash".to_string(),
            created_by: Uuid::new_v4(),
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::days(365),
            status: InvitationStatus::Active,
            used_by: None,
            used_at: None,
        };
        assert!(validate_invitation(&invitation).is_ok());
    }

    // =====================================================================
    // 追加テスト: InvitationCodeRow -> InvitationCode conversion
    // =====================================================================

    #[test]
    fn test_invitation_code_row_conversion() {
        let id = Uuid::new_v4();
        let created_by = Uuid::new_v4();
        let now = Utc::now();
        let row = InvitationCodeRow {
            id: id.to_string(),
            code_hash: "hash123".to_string(),
            created_by: created_by.to_string(),
            created_at: now.to_rfc3339(),
            expires_at: (now + Duration::hours(72)).to_rfc3339(),
            status: "active".to_string(),
            used_by: None,
            used_at: None,
        };

        let invitation = row.try_into_invitation_code().unwrap();
        assert_eq!(invitation.id, id);
        assert_eq!(invitation.code_hash, "hash123");
        assert_eq!(invitation.created_by, created_by);
        assert_eq!(invitation.status, InvitationStatus::Active);
        assert!(invitation.used_by.is_none());
        assert!(invitation.used_at.is_none());
    }

    #[test]
    fn test_invitation_code_row_with_used_fields() {
        let id = Uuid::new_v4();
        let created_by = Uuid::new_v4();
        let used_by = Uuid::new_v4();
        let now = Utc::now();
        let row = InvitationCodeRow {
            id: id.to_string(),
            code_hash: "hash".to_string(),
            created_by: created_by.to_string(),
            created_at: now.to_rfc3339(),
            expires_at: (now + Duration::hours(72)).to_rfc3339(),
            status: "used".to_string(),
            used_by: Some(used_by.to_string()),
            used_at: Some(now.to_rfc3339()),
        };

        let invitation = row.try_into_invitation_code().unwrap();
        assert_eq!(invitation.status, InvitationStatus::Used);
        assert_eq!(invitation.used_by, Some(used_by));
        assert!(invitation.used_at.is_some());
    }

    #[test]
    fn test_invitation_code_row_invalid_uuid() {
        let now = Utc::now();
        let row = InvitationCodeRow {
            id: "not-a-uuid".to_string(),
            code_hash: "hash".to_string(),
            created_by: Uuid::new_v4().to_string(),
            created_at: now.to_rfc3339(),
            expires_at: (now + Duration::hours(72)).to_rfc3339(),
            status: "active".to_string(),
            used_by: None,
            used_at: None,
        };

        let result = row.try_into_invitation_code();
        assert!(result.is_err());
    }

    #[test]
    fn test_invitation_code_row_invalid_created_at() {
        let row = InvitationCodeRow {
            id: Uuid::new_v4().to_string(),
            code_hash: "hash".to_string(),
            created_by: Uuid::new_v4().to_string(),
            created_at: "not-a-date".to_string(),
            expires_at: Utc::now().to_rfc3339(),
            status: "active".to_string(),
            used_by: None,
            used_at: None,
        };

        let result = row.try_into_invitation_code();
        assert!(result.is_err());
    }

    #[test]
    fn test_invitation_code_row_invalid_status() {
        let now = Utc::now();
        let row = InvitationCodeRow {
            id: Uuid::new_v4().to_string(),
            code_hash: "hash".to_string(),
            created_by: Uuid::new_v4().to_string(),
            created_at: now.to_rfc3339(),
            expires_at: (now + Duration::hours(72)).to_rfc3339(),
            status: "unknown_status".to_string(),
            used_by: None,
            used_at: None,
        };

        let result = row.try_into_invitation_code();
        assert!(result.is_err());
    }

    // =====================================================================
    // 追加テスト: DB操作 - create with custom expiry
    // =====================================================================

    #[tokio::test]
    async fn test_create_with_custom_expiry() {
        let pool = setup_test_db().await;
        let admin = users::create(&pool, "admin", "hash", UserRole::Admin, false)
            .await
            .unwrap();

        let invitation = create(&pool, admin.id, Some(1)).await.unwrap();

        // Should expire in ~1 hour
        let diff = invitation.expires_at - invitation.created_at;
        assert_eq!(diff.num_hours(), 1);
    }

    #[tokio::test]
    async fn test_create_with_default_expiry() {
        let pool = setup_test_db().await;
        let admin = users::create(&pool, "admin", "hash", UserRole::Admin, false)
            .await
            .unwrap();

        let invitation = create(&pool, admin.id, None).await.unwrap();

        // Default: 72 hours
        let diff = invitation.expires_at - invitation.created_at;
        assert_eq!(diff.num_hours(), 72);
    }

    // =====================================================================
    // 追加テスト: DB操作 - revoke nonexistent
    // =====================================================================

    #[tokio::test]
    async fn test_revoke_nonexistent_invitation() {
        let pool = setup_test_db().await;

        let revoked = revoke(&pool, Uuid::new_v4()).await.unwrap();
        assert!(!revoked);
    }

    // =====================================================================
    // 追加テスト: DB操作 - mark_as_used nonexistent
    // =====================================================================

    #[tokio::test]
    async fn test_mark_as_used_nonexistent_invitation() {
        let pool = setup_test_db().await;

        let result = mark_as_used(&pool, Uuid::new_v4(), Uuid::new_v4()).await;
        assert!(result.is_err());
    }

    // =====================================================================
    // 追加テスト: DB操作 - delete nonexistent (no-op)
    // =====================================================================

    #[tokio::test]
    async fn test_delete_nonexistent_invitation_is_noop() {
        let pool = setup_test_db().await;
        delete(&pool, Uuid::new_v4()).await.unwrap();
    }

    // =====================================================================
    // 追加テスト: DB操作 - list empty
    // =====================================================================

    #[tokio::test]
    async fn test_list_empty_db() {
        let pool = setup_test_db().await;
        let invitations = list(&pool).await.unwrap();
        assert!(invitations.is_empty());
    }

    // =====================================================================
    // 追加テスト: DB操作 - find_by_code not found
    // =====================================================================

    #[tokio::test]
    async fn test_find_by_code_not_found() {
        let pool = setup_test_db().await;
        let found = find_by_code(&pool, "nonexistent_code").await.unwrap();
        assert!(found.is_none());
    }

    // =====================================================================
    // 追加テスト: DB操作 - revoke already revoked
    // =====================================================================

    #[tokio::test]
    async fn test_revoke_already_revoked_invitation() {
        let pool = setup_test_db().await;
        let admin = users::create(&pool, "admin", "hash", UserRole::Admin, false)
            .await
            .unwrap();

        let invitation = create(&pool, admin.id, None).await.unwrap();

        // Revoke first time
        let revoked = revoke(&pool, invitation.id).await.unwrap();
        assert!(revoked);

        // Revoke second time should return false
        let revoked_again = revoke(&pool, invitation.id).await.unwrap();
        assert!(!revoked_again);
    }

    // =====================================================================
    // 追加テスト: DB操作 - mark_as_used on revoked invitation
    // =====================================================================

    #[tokio::test]
    async fn test_mark_as_used_revoked_invitation() {
        let pool = setup_test_db().await;
        let admin = users::create(&pool, "admin", "hash", UserRole::Admin, false)
            .await
            .unwrap();

        let invitation = create(&pool, admin.id, None).await.unwrap();
        revoke(&pool, invitation.id).await.unwrap();

        let user = users::create(&pool, "user", "hash", UserRole::Viewer, false)
            .await
            .unwrap();
        let result = mark_as_used(&pool, invitation.id, user.id).await;
        assert!(result.is_err());
    }

    // =====================================================================
    // 追加テスト: DB操作 - create multiple from same admin
    // =====================================================================

    #[tokio::test]
    async fn test_create_multiple_invitations_same_admin() {
        let pool = setup_test_db().await;
        let admin = users::create(&pool, "admin", "hash", UserRole::Admin, false)
            .await
            .unwrap();

        let inv1 = create(&pool, admin.id, None).await.unwrap();
        let inv2 = create(&pool, admin.id, None).await.unwrap();
        let inv3 = create(&pool, admin.id, Some(24)).await.unwrap();

        assert_ne!(inv1.id, inv2.id);
        assert_ne!(inv2.id, inv3.id);

        let all = list(&pool).await.unwrap();
        assert_eq!(all.len(), 3);

        // All should have same created_by
        for inv in &all {
            assert_eq!(inv.created_by, admin.id);
        }
    }

    // =====================================================================
    // 追加テスト: InvitationCodeWithPlaintext serialize
    // =====================================================================

    #[test]
    fn test_invitation_code_with_plaintext_serialize() {
        let inv = InvitationCodeWithPlaintext {
            id: Uuid::new_v4(),
            code: "inv_test1234567890ab".to_string(),
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::hours(72),
        };
        let json = serde_json::to_value(&inv).unwrap();
        assert!(json["id"].is_string());
        assert_eq!(json["code"], "inv_test1234567890ab");
        assert!(json["created_at"].is_string());
        assert!(json["expires_at"].is_string());
    }
}
