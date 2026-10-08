//! パスワードリセットトークン（SPEC #580 US-008）
//!
//! forgot-password で発行し、reset-password で一度だけ消費する。
//! DB には平文を保存せず SHA-256 ハッシュのみを保持する。

use crate::common::error::LbError;
use chrono::{Duration, Utc};
use sqlx::SqlitePool;
use uuid::Uuid;

/// 平文トークンの長さ（英数字62種 × 48文字 ≒ 285bit のエントロピー）
const TOKEN_LENGTH: usize = 48;

fn hash_token(token: &str) -> String {
    crate::db::invitations::hash_with_sha256(token)
}

/// ユーザーのリセットトークンを発行し、平文トークンを返す
///
/// 同ユーザーの未使用トークンは発行前に全て失効させるため、有効なトークンは常に最新の1つだけ。
pub async fn issue(pool: &SqlitePool, user_id: Uuid, ttl: Duration) -> Result<String, LbError> {
    let token = crate::auth::generate_random_token(TOKEN_LENGTH);
    let now = Utc::now().timestamp_millis();
    let expires_at = now + ttl.num_milliseconds();

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| LbError::Database(format!("Failed to begin transaction: {}", e)))?;

    sqlx::query("DELETE FROM password_reset_tokens WHERE user_id = ? AND used_at IS NULL")
        .bind(user_id.to_string())
        .execute(&mut *tx)
        .await
        .map_err(|e| LbError::Database(format!("Failed to revoke reset tokens: {}", e)))?;

    sqlx::query(
        "INSERT INTO password_reset_tokens (id, user_id, token_hash, created_at, expires_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(user_id.to_string())
    .bind(hash_token(&token))
    .bind(now)
    .bind(expires_at)
    .execute(&mut *tx)
    .await
    .map_err(|e| LbError::Database(format!("Failed to create reset token: {}", e)))?;

    tx.commit()
        .await
        .map_err(|e| LbError::Database(format!("Failed to commit reset token: {}", e)))?;

    Ok(token)
}

/// 有効なトークンを原子的に消費し、対象ユーザーIDを返す
///
/// 不明・期限切れ・使用済みのトークンは `Ok(None)`。
pub async fn consume(pool: &SqlitePool, token: &str) -> Result<Option<Uuid>, LbError> {
    let now = Utc::now().timestamp_millis();
    let user_id: Option<String> = sqlx::query_scalar(
        "UPDATE password_reset_tokens SET used_at = ?
         WHERE token_hash = ? AND used_at IS NULL AND expires_at > ?
         RETURNING user_id",
    )
    .bind(now)
    .bind(hash_token(token))
    .bind(now)
    .fetch_optional(pool)
    .await
    .map_err(|e| LbError::Database(format!("Failed to consume reset token: {}", e)))?;

    user_id
        .map(|id| {
            Uuid::parse_str(&id)
                .map_err(|e| LbError::Database(format!("Invalid user id in reset token: {}", e)))
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::auth::UserRole;
    use crate::db::users;

    async fn setup() -> (SqlitePool, Uuid) {
        let pool = crate::db::test_utils::test_db_pool().await;
        let user = users::create(&pool, "alice@example.com", "hash", UserRole::Viewer, false)
            .await
            .unwrap();
        (pool, user.id)
    }

    #[tokio::test]
    async fn issued_token_is_consumed_once() {
        let (pool, user_id) = setup().await;
        let token = issue(&pool, user_id, Duration::minutes(30)).await.unwrap();
        assert_eq!(token.len(), TOKEN_LENGTH);

        assert_eq!(consume(&pool, &token).await.unwrap(), Some(user_id));
        assert_eq!(consume(&pool, &token).await.unwrap(), None);
    }

    #[tokio::test]
    async fn plaintext_token_is_not_stored() {
        let (pool, user_id) = setup().await;
        let token = issue(&pool, user_id, Duration::minutes(30)).await.unwrap();

        let stored: String = sqlx::query_scalar("SELECT token_hash FROM password_reset_tokens")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_ne!(stored, token);
        assert_eq!(stored, hash_token(&token));
    }

    #[tokio::test]
    async fn expired_token_is_rejected() {
        let (pool, user_id) = setup().await;
        let token = issue(&pool, user_id, Duration::seconds(-1)).await.unwrap();

        assert_eq!(consume(&pool, &token).await.unwrap(), None);
    }

    #[tokio::test]
    async fn unknown_token_is_rejected() {
        let (pool, _user_id) = setup().await;

        assert_eq!(consume(&pool, "unknown").await.unwrap(), None);
    }

    #[tokio::test]
    async fn reissue_revokes_previous_unused_token() {
        let (pool, user_id) = setup().await;
        let first = issue(&pool, user_id, Duration::minutes(30)).await.unwrap();
        let second = issue(&pool, user_id, Duration::minutes(30)).await.unwrap();

        assert_eq!(consume(&pool, &first).await.unwrap(), None);
        assert_eq!(consume(&pool, &second).await.unwrap(), Some(user_id));
    }

    #[tokio::test]
    async fn tokens_are_removed_with_user() {
        let (pool, user_id) = setup().await;
        let token = issue(&pool, user_id, Duration::minutes(30)).await.unwrap();
        users::delete(&pool, user_id).await.unwrap();

        assert_eq!(consume(&pool, &token).await.unwrap(), None);
    }
}
