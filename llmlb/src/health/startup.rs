//! ルーター起動時のヘルスチェック
//!
//! 起動時に全エンドポイントを並列チェック

use super::EndpointHealthChecker;
use crate::registry::endpoints::EndpointRegistry;
use tracing::info;

/// ルーター起動時の初期ヘルスチェック
///
/// 全エンドポイントを並列にヘルスチェックし、
/// 正確な状態を把握してからルーティングを開始する。
pub async fn run_startup_health_check(
    registry: &EndpointRegistry,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Running startup health check...");

    let checker = EndpointHealthChecker::new(registry.clone());
    checker.check_all_endpoints_parallel().await?;

    info!("Startup health check completed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_utils::TEST_LOCK;
    use sqlx::SqlitePool;

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to create test database");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");
        pool
    }

    #[tokio::test]
    async fn test_startup_health_check_no_endpoints() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;
        let registry = EndpointRegistry::new(pool).await.unwrap();

        // エンドポイントがない場合でもエラーにならない
        let result = run_startup_health_check(&registry).await;
        assert!(result.is_ok());
    }
}
