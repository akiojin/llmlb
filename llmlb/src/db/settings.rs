//! 設定テーブルのストレージ層
//!
//! SQLiteベースのキーバリュー設定を永続化

use crate::common::error::{LbError, RouterResult};
use sqlx::SqlitePool;

/// 設定ストレージ
#[derive(Clone)]
pub struct SettingsStorage {
    pool: SqlitePool,
}

impl SettingsStorage {
    /// 新しいストレージインスタンスを作成
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 設定値を取得
    pub async fn get_setting(&self, key: &str) -> RouterResult<Option<String>> {
        let result = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| LbError::Database(format!("Failed to get setting: {}", e)))?;

        Ok(result)
    }

    /// 設定値を保存（INSERT OR REPLACE）
    pub async fn set_setting(&self, key: &str, value: &str) -> RouterResult<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now'))",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await
        .map_err(|e| LbError::Database(format!("Failed to set setting: {}", e)))?;

        Ok(())
    }
}
