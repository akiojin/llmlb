// T040-T041: データベースマイグレーション実行とJSONインポート

use crate::common::error::LbError;
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
use std::path::Path;

/// SQLiteデータベース接続プールを作成してマイグレーションを実行
///
/// # Arguments
/// * `database_url` - データベースURL（例: "sqlite:data/load balancer.db"）
///
/// # Returns
/// * `Ok(SqlitePool)` - 初期化済みデータベースプール
/// * `Err(LbError)` - 初期化失敗
pub async fn initialize_database(database_url: &str) -> Result<SqlitePool, LbError> {
    // データベースファイルが存在しない場合は作成
    if !Sqlite::database_exists(database_url)
        .await
        .map_err(|e| LbError::Database(format!("Failed to check database: {}", e)))?
    {
        tracing::info!("Creating database: {}", database_url);
        Sqlite::create_database(database_url)
            .await
            .map_err(|e| LbError::Database(format!("Failed to create database: {}", e)))?;
    }

    // 接続プールを作成
    let pool = SqlitePool::connect(database_url)
        .await
        .map_err(|e| LbError::Database(format!("Failed to connect to database: {}", e)))?;

    // マイグレーションを実行
    run_migrations(&pool).await?;

    Ok(pool)
}

/// マイグレーションを実行（sqlx::migrate!マクロを使用）
///
/// # Arguments
/// * `pool` - データベース接続プール
///
/// # Returns
/// * `Ok(())` - マイグレーション成功
/// * `Err(LbError)` - マイグレーション失敗
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), LbError> {
    tracing::info!("Running database migrations");

    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|e| LbError::Database(format!("Failed to run migrations: {}", e)))?;

    tracing::info!("Database migrations completed successfully");
    Ok(())
}

/// JSONファイルからノードデータをインポート（マイグレーション用）
///
/// 注: この機能は将来的にノードデータもSQLiteに移行する際に使用
/// 現在のところ、認証機能はノードデータとは独立して動作
///
/// # Arguments
/// * `json_path` - nodes.jsonのパス
///
/// # Returns
/// * `Ok(())` - インポート成功、元ファイルを.migratedにリネーム
/// * `Err(LbError)` - インポート失敗
pub async fn import_nodes_from_json(json_path: &str) -> Result<(), LbError> {
    let path = Path::new(json_path);

    // ファイルが存在しない場合はスキップ
    if !path.exists() {
        tracing::info!("No nodes.json found at {}, skipping import", json_path);
        return Ok(());
    }

    // TODO: 将来的にノードデータをSQLiteに移行する場合、ここで実装
    // 現在は認証機能のみSQLiteを使用し、ノードデータは既存のJSONベース実装を継続

    tracing::info!("Node data import not yet implemented (nodes remain in JSON format)");

    // マイグレーション完了マーク（ファイルリネーム）
    let migrated_path = format!("{}.migrated", json_path);
    if let Err(e) = std::fs::rename(path, &migrated_path) {
        tracing::warn!("Failed to rename {} to {}: {}", json_path, migrated_path, e);
    } else {
        tracing::info!("Renamed {} to {}", json_path, migrated_path);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_initialize_database() {
        // テスト用の一時データベース
        let db_url = "sqlite::memory:";

        let pool = initialize_database(db_url)
            .await
            .expect("Failed to initialize database");

        // usersテーブルが作成されているか確認
        let result =
            sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='users'")
                .fetch_one(&pool)
                .await;

        assert!(result.is_ok(), "users table should exist");
    }

    #[tokio::test]
    async fn test_run_migrations() {
        let db_url = "sqlite::memory:";
        let pool = SqlitePool::connect(db_url)
            .await
            .expect("Failed to connect");

        run_migrations(&pool)
            .await
            .expect("Failed to run migrations");

        // api_keysテーブルが作成されているか確認
        let result =
            sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='api_keys'")
                .fetch_one(&pool)
                .await;

        assert!(result.is_ok(), "api_keys table should exist");
    }

    #[tokio::test]
    async fn test_import_nodes_from_json_no_file() {
        // 存在しないファイルの場合はエラーなく完了
        let result = import_nodes_from_json("/nonexistent/nodes.json").await;
        assert!(result.is_ok());
    }

    // --- 追加テスト ---

    #[tokio::test]
    async fn test_migrations_create_endpoints_table() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        run_migrations(&pool).await.unwrap();

        let result =
            sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='endpoints'")
                .fetch_one(&pool)
                .await;
        assert!(result.is_ok(), "endpoints table should exist");
    }

    #[tokio::test]
    async fn test_migrations_create_settings_table() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        run_migrations(&pool).await.unwrap();

        let result =
            sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='settings'")
                .fetch_one(&pool)
                .await;
        assert!(result.is_ok(), "settings table should exist");
    }

    #[tokio::test]
    async fn test_migrations_create_audit_log_table() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        run_migrations(&pool).await.unwrap();

        let result = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='audit_log_entries'",
        )
        .fetch_one(&pool)
        .await;
        assert!(result.is_ok(), "audit_log_entries table should exist");
    }

    #[tokio::test]
    async fn test_migrations_idempotent() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        run_migrations(&pool).await.unwrap();
        // Running twice should not error
        run_migrations(&pool).await.unwrap();

        let result =
            sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='users'")
                .fetch_one(&pool)
                .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_migrations_create_invitation_codes_table() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        run_migrations(&pool).await.unwrap();

        let result = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='invitation_codes'",
        )
        .fetch_one(&pool)
        .await;
        assert!(result.is_ok(), "invitation_codes table should exist");
    }

    #[tokio::test]
    async fn test_migrations_create_endpoint_models_table() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        run_migrations(&pool).await.unwrap();

        let result = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='endpoint_models'",
        )
        .fetch_one(&pool)
        .await;
        assert!(result.is_ok(), "endpoint_models table should exist");
    }

    #[tokio::test]
    async fn test_migrations_create_endpoint_health_checks_table() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        run_migrations(&pool).await.unwrap();

        let result = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='endpoint_health_checks'",
        )
        .fetch_one(&pool)
        .await;
        assert!(result.is_ok(), "endpoint_health_checks table should exist");
    }

    #[tokio::test]
    async fn test_migrations_create_model_download_tasks_table() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        run_migrations(&pool).await.unwrap();

        let result = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='model_download_tasks'",
        )
        .fetch_one(&pool)
        .await;
        assert!(result.is_ok(), "model_download_tasks table should exist");
    }

    #[tokio::test]
    async fn test_migrations_create_endpoint_daily_stats_table() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        run_migrations(&pool).await.unwrap();

        let result = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='endpoint_daily_stats'",
        )
        .fetch_one(&pool)
        .await;
        assert!(result.is_ok(), "endpoint_daily_stats table should exist");
    }

    #[tokio::test]
    async fn test_import_nodes_from_json_with_temp_file() {
        // Create a temporary file to test actual rename behavior
        let tmp_dir = std::env::temp_dir();
        let tmp_file = tmp_dir.join("test_nodes_import.json");
        std::fs::write(&tmp_file, "{}").expect("Failed to create temp file");

        let result = import_nodes_from_json(tmp_file.to_str().unwrap()).await;
        assert!(result.is_ok());

        // The original file should be renamed to .migrated
        assert!(!tmp_file.exists());
        let migrated = format!("{}.migrated", tmp_file.display());
        assert!(std::path::Path::new(&migrated).exists());

        // Cleanup
        let _ = std::fs::remove_file(&migrated);
    }

    #[tokio::test]
    async fn test_migrations_create_models_table() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        run_migrations(&pool).await.unwrap();

        let result =
            sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='models'")
                .fetch_one(&pool)
                .await;
        assert!(result.is_ok(), "models table should exist");
    }
}
