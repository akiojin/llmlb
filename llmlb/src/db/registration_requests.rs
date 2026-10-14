//! 未処理の登録申請。承認時だけ users に移し、申請の消化と作成を原子的に行う。

use crate::common::{
    auth::{User, UserRole},
    error::{CommonError, LbError},
};
use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

/// API に公開する申請情報。パスワードハッシュは含めない。
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct RegistrationRequest {
    /// 申請ID
    pub id: String,
    /// 申請ユーザー名
    pub username: String,
    /// 申請日時（RFC 3339）
    pub created_at: String,
}

/// 申請数の上限は HTTP 429 に変換するため独立して扱う。
#[derive(Debug)]
pub enum CreateRequestError {
    /// 未処理申請数が上限に達した
    Full,
    /// ストレージまたは一意性制約のエラー
    Storage(LbError),
}

/// DB 制約のエラーを API でも扱えるドメインエラーに変換する。
pub(crate) fn map_write_error(error: sqlx::Error) -> LbError {
    if let Some(database) = error.as_database_error() {
        if database.is_unique_violation() || database.message() == "username_reserved" {
            return LbError::Conflict("Username already exists or has a pending request".into());
        }
        if database.message() == "last_administrator" {
            return LbError::Common(CommonError::Validation(
                "Cannot remove or demote the last administrator".into(),
            ));
        }
    }
    LbError::Database(format!("Registration/user write failed: {error}"))
}

/// ハッシュ化済みパスワードで新規申請を保存する。
pub async fn create(
    pool: &SqlitePool,
    username: &str,
    password_hash: &str,
) -> Result<RegistrationRequest, CreateRequestError> {
    let request = RegistrationRequest {
        id: Uuid::new_v4().to_string(),
        username: username.to_string(),
        created_at: Utc::now().to_rfc3339(),
    };
    sqlx::query("INSERT INTO registration_requests (id, username, password_hash, created_at) VALUES (?, ?, ?, ?)")
        .bind(&request.id).bind(&request.username).bind(password_hash).bind(&request.created_at)
        .execute(pool).await.map_err(|error| {
            if error.as_database_error().is_some_and(|database| database.message() == "registration_requests_full") {
                CreateRequestError::Full
            } else {
                CreateRequestError::Storage(map_write_error(error))
            }
        })?;
    Ok(request)
}

/// ハッシュを含めず、未処理申請を作成日時順に返す。
pub async fn list(pool: &SqlitePool) -> Result<Vec<RegistrationRequest>, LbError> {
    sqlx::query_as(
        "SELECT id, username, created_at FROM registration_requests ORDER BY created_at, id",
    )
    .fetch_all(pool)
    .await
    .map_err(|error| LbError::Database(format!("Failed to list registration requests: {error}")))
}

/// 申請を消費し、指定ロールのユーザーを同一トランザクションで作成する。
pub async fn approve(pool: &SqlitePool, id: Uuid, role: UserRole) -> Result<User, LbError> {
    // 先に書き込み権を取得し、並行承認による read -> write のロック昇格を避ける。
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(map_write_error)?;
    let (username, password_hash): (String, String) = sqlx::query_as(
        "DELETE FROM registration_requests WHERE id = ? RETURNING username, password_hash",
    )
    .bind(id.to_string())
    .fetch_optional(&mut *transaction)
    .await
    .map_err(map_write_error)?
    .ok_or_else(|| LbError::NotFound("Registration request not found".into()))?;
    let user = User {
        id: Uuid::new_v4(),
        username,
        password_hash,
        role,
        created_at: Utc::now(),
        last_login: None,
        must_change_password: false,
        password_changed_at: 0,
        email: None,
    };
    sqlx::query("INSERT INTO users (id, username, password_hash, role, created_at, must_change_password, password_changed_at) VALUES (?, ?, ?, ?, ?, 0, 0)")
        .bind(user.id.to_string()).bind(&user.username).bind(&user.password_hash)
        .bind(match role { UserRole::Admin => "admin", UserRole::Viewer => "viewer" })
        .bind(user.created_at.to_rfc3339()).execute(&mut *transaction).await.map_err(map_write_error)?;
    transaction.commit().await.map_err(map_write_error)?;
    Ok(user)
}

/// 申請を削除し、そのユーザー名の予約を解除する。
pub async fn reject(pool: &SqlitePool, id: Uuid) -> Result<(), LbError> {
    let result = sqlx::query("DELETE FROM registration_requests WHERE id = ?")
        .bind(id.to_string())
        .execute(pool)
        .await
        .map_err(map_write_error)?;
    if result.rows_affected() == 0 {
        return Err(LbError::NotFound("Registration request not found".into()));
    }
    Ok(())
}
