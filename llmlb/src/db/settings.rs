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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_utils::TEST_LOCK;

    async fn setup() -> SettingsStorage {
        let pool = crate::db::test_utils::test_db_pool().await;
        SettingsStorage::new(pool)
    }

    #[tokio::test]
    async fn get_nonexistent_key_returns_none() {
        let _lock = TEST_LOCK.lock().await;
        let storage = setup().await;
        let result = storage.get_setting("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn set_and_get_roundtrip() {
        let _lock = TEST_LOCK.lock().await;
        let storage = setup().await;
        storage.set_setting("theme", "dark").await.unwrap();
        let result = storage.get_setting("theme").await.unwrap();
        assert_eq!(result, Some("dark".to_string()));
    }

    #[tokio::test]
    async fn set_overwrites_existing_value() {
        let _lock = TEST_LOCK.lock().await;
        let storage = setup().await;
        storage.set_setting("lang", "en").await.unwrap();
        storage.set_setting("lang", "ja").await.unwrap();
        let result = storage.get_setting("lang").await.unwrap();
        assert_eq!(result, Some("ja".to_string()));
    }

    #[tokio::test]
    async fn multiple_keys_are_independent() {
        let _lock = TEST_LOCK.lock().await;
        let storage = setup().await;
        storage.set_setting("key_a", "value_a").await.unwrap();
        storage.set_setting("key_b", "value_b").await.unwrap();
        assert_eq!(
            storage.get_setting("key_a").await.unwrap(),
            Some("value_a".to_string())
        );
        assert_eq!(
            storage.get_setting("key_b").await.unwrap(),
            Some("value_b".to_string())
        );
    }

    #[tokio::test]
    async fn set_empty_string_value() {
        let _lock = TEST_LOCK.lock().await;
        let storage = setup().await;
        storage.set_setting("empty", "").await.unwrap();
        let result = storage.get_setting("empty").await.unwrap();
        assert_eq!(result, Some(String::new()));
    }

    #[tokio::test]
    async fn set_and_get_unicode_value() {
        let _lock = TEST_LOCK.lock().await;
        let storage = setup().await;
        storage
            .set_setting("greeting", "こんにちは世界")
            .await
            .unwrap();
        let result = storage.get_setting("greeting").await.unwrap();
        assert_eq!(result, Some("こんにちは世界".to_string()));
    }

    #[tokio::test]
    async fn set_long_value() {
        let _lock = TEST_LOCK.lock().await;
        let storage = setup().await;
        let long_val = "x".repeat(10_000);
        storage.set_setting("long", &long_val).await.unwrap();
        let result = storage.get_setting("long").await.unwrap();
        assert_eq!(result, Some(long_val));
    }

    #[tokio::test]
    async fn set_json_value() {
        let _lock = TEST_LOCK.lock().await;
        let storage = setup().await;
        let json_val = r#"{"key": "value", "number": 42}"#;
        storage.set_setting("config", json_val).await.unwrap();
        let result = storage.get_setting("config").await.unwrap();
        assert_eq!(result, Some(json_val.to_string()));
    }

    #[tokio::test]
    async fn set_special_characters_key() {
        let _lock = TEST_LOCK.lock().await;
        let storage = setup().await;
        storage
            .set_setting("key.with.dots", "dotted")
            .await
            .unwrap();
        storage
            .set_setting("key-with-dashes", "dashed")
            .await
            .unwrap();
        assert_eq!(
            storage.get_setting("key.with.dots").await.unwrap(),
            Some("dotted".to_string())
        );
        assert_eq!(
            storage.get_setting("key-with-dashes").await.unwrap(),
            Some("dashed".to_string())
        );
    }
}
