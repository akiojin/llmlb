// SPEC-62ac4b68 T002: settingsテーブルCRUDのテスト（RED）

#[cfg(test)]
mod settings_tests {
    use llmlb::db::settings::SettingsStorage;

    async fn create_settings_storage() -> SettingsStorage {
        let pool = crate::support::lb::create_test_db_pool().await;
        SettingsStorage::new(pool)
    }

    #[tokio::test]
    async fn test_get_setting_returns_none_for_nonexistent_key() {
        let storage = create_settings_storage().await;
        let result = storage.get_setting("nonexistent_key").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_set_and_get_setting_roundtrip() {
        let storage = create_settings_storage().await;
        storage.set_setting("test_key", "test_value").await.unwrap();
        let result = storage.get_setting("test_key").await.unwrap();
        assert_eq!(result, Some("test_value".to_string()));
    }

    #[tokio::test]
    async fn test_set_setting_overwrites_existing() {
        let storage = create_settings_storage().await;
        storage
            .set_setting("overwrite_key", "original")
            .await
            .unwrap();
        storage
            .set_setting("overwrite_key", "updated")
            .await
            .unwrap();
        let result = storage.get_setting("overwrite_key").await.unwrap();
        assert_eq!(result, Some("updated".to_string()));
    }

    #[tokio::test]
    async fn test_default_ip_alert_threshold_exists() {
        let storage = create_settings_storage().await;
        let result = storage.get_setting("ip_alert_threshold").await.unwrap();
        assert_eq!(result, Some("100".to_string()));
    }
}
