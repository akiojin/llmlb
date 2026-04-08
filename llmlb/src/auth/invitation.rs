// 招待キー検証モジュール

use crate::common::error::LbError;
use sqlx::SqlitePool;

/// 招待キーを検証し、有効性を確認する
///
/// # Arguments
/// * `pool` - データベース接続プール
/// * `key` - 招待キー（平文）
///
/// # Returns
/// * `Ok(true)` - 招待キーが有効
/// * `Ok(false)` - 招待キーが無効（使用済みまたは期限切れ）
/// * `Err(LbError)` - 招待キーが存在しない、またはDB操作エラー
pub async fn verify_invitation_key(pool: &SqlitePool, key: &str) -> Result<bool, LbError> {
    let invitation = crate::db::invitations::find_by_code(pool, key)
        .await?
        .ok_or_else(|| LbError::NotFound(format!("Invitation code not found: {}", key)))?;

    // 既存の validate_invitation 関数を使用
    match crate::db::invitations::validate_invitation(&invitation) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::auth::UserRole;
    use crate::db::users;
    use chrono::Utc;

    async fn setup_test_db() -> SqlitePool {
        crate::db::test_utils::test_db_pool().await
    }

    #[tokio::test]
    async fn test_verify_valid_invitation_key() {
        let pool = setup_test_db().await;

        // テスト用ユーザーを作成
        let admin = users::create(&pool, "admin", "hash", UserRole::Admin, false)
            .await
            .unwrap();

        // 招待コードを生成
        let invitation = crate::db::invitations::create(&pool, admin.id, None)
            .await
            .unwrap();

        // 有効な招待キーを検証
        let result = verify_invitation_key(&pool, &invitation.code)
            .await
            .unwrap();
        assert_eq!(result, true);
    }

    #[tokio::test]
    async fn test_verify_used_invitation_key() {
        let pool = setup_test_db().await;

        let admin = users::create(&pool, "admin", "hash", UserRole::Admin, false)
            .await
            .unwrap();

        let invitation = crate::db::invitations::create(&pool, admin.id, None)
            .await
            .unwrap();

        // 招待コードを使用済みにする
        let user = users::create(&pool, "user1", "hash", UserRole::Viewer, false)
            .await
            .unwrap();

        crate::db::invitations::mark_as_used(&pool, invitation.id, user.id)
            .await
            .unwrap();

        // 使用済み招待キーを検証
        let result = verify_invitation_key(&pool, &invitation.code)
            .await
            .unwrap();
        assert_eq!(result, false);
    }

    #[tokio::test]
    async fn test_verify_expired_invitation_key() {
        let pool = setup_test_db().await;

        let admin = users::create(&pool, "admin", "hash", UserRole::Admin, false)
            .await
            .unwrap();

        // 有効期限が過去のコードを作成（1時間前）
        let invitation = crate::db::invitations::create(&pool, admin.id, Some(-1))
            .await
            .unwrap();

        // 期限切れ招待キーを検証
        let result = verify_invitation_key(&pool, &invitation.code)
            .await
            .unwrap();
        assert_eq!(result, false);
    }

    #[tokio::test]
    async fn test_verify_nonexistent_invitation_key() {
        let pool = setup_test_db().await;

        // 存在しない招待キーを検証
        let result = verify_invitation_key(&pool, "inv_nonexistent").await;

        assert!(result.is_err());
        match result {
            Err(LbError::NotFound(_)) => {} // 期待通り
            _ => panic!("Expected NotFound error"),
        }
    }
}
