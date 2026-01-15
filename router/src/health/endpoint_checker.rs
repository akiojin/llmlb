//! エンドポイントヘルスチェッカー
//!
//! プル型ヘルスチェックでエンドポイントの稼働状況を監視

use crate::db::endpoints as db;
use crate::registry::endpoints::EndpointRegistry;
use crate::types::endpoint::{Endpoint, EndpointHealthCheck, EndpointStatus};
use chrono::Utc;
use reqwest::Client;
use std::time::{Duration, Instant};
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// ヘルスチェックのタイムアウト（秒）
const HEALTH_CHECK_TIMEOUT_SECS: u64 = 5;

/// デフォルトのチェック間隔（秒）
const DEFAULT_CHECK_INTERVAL_SECS: u64 = 30;

/// オフライン判定までの連続失敗回数
const CONSECUTIVE_FAILURES_FOR_OFFLINE: u32 = 2;

/// エンドポイントヘルスチェッカー
///
/// 定期的にエンドポイントにGET /v1/modelsリクエストを送信し、
/// 稼働状況を監視する。
#[derive(Clone)]
pub struct EndpointHealthChecker {
    /// エンドポイントレジストリ
    registry: EndpointRegistry,
    /// HTTPクライアント
    client: Client,
    /// チェック間隔（秒）
    check_interval_secs: u64,
}

impl EndpointHealthChecker {
    /// 新しいヘルスチェッカーを作成
    pub fn new(registry: EndpointRegistry) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(HEALTH_CHECK_TIMEOUT_SECS))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            registry,
            client,
            check_interval_secs: DEFAULT_CHECK_INTERVAL_SECS,
        }
    }

    /// チェック間隔を設定
    pub fn with_interval(mut self, interval_secs: u64) -> Self {
        self.check_interval_secs = interval_secs;
        self
    }

    /// バックグラウンドで監視を開始
    pub fn start(self) {
        tokio::spawn(async move {
            self.monitor_loop().await;
        });
    }

    /// 監視ループ
    async fn monitor_loop(&self) {
        let mut timer = interval(Duration::from_secs(self.check_interval_secs));

        info!(
            interval_secs = self.check_interval_secs,
            "Endpoint health checker started"
        );

        loop {
            timer.tick().await;

            if let Err(e) = self.check_all_endpoints().await {
                error!("Health check error: {}", e);
            }

            // 古いヘルスチェック履歴をクリーンアップ
            if let Err(e) = db::cleanup_old_health_checks(self.registry.pool()).await {
                error!("Failed to cleanup old health checks: {}", e);
            }
        }
    }

    /// 全エンドポイントのヘルスチェック
    pub async fn check_all_endpoints(
        &self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let endpoints = self.registry.list().await;

        for endpoint in endpoints {
            if let Err(e) = self.check_endpoint(&endpoint).await {
                debug!(
                    endpoint_id = %endpoint.id,
                    endpoint_name = %endpoint.name,
                    error = %e,
                    "Health check failed"
                );
            }
        }

        Ok(())
    }

    /// 全エンドポイントを並列チェック（起動時用）
    pub async fn check_all_endpoints_parallel(
        &self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let endpoints = self.registry.list().await;

        if endpoints.is_empty() {
            info!("No endpoints to check");
            return Ok(());
        }

        info!(
            count = endpoints.len(),
            "Starting parallel health check for all endpoints"
        );

        let mut handles = Vec::with_capacity(endpoints.len());

        for endpoint in endpoints {
            let checker = self.clone();
            handles.push(tokio::spawn(async move {
                let result = checker.check_endpoint(&endpoint).await;
                (endpoint.id, endpoint.name.clone(), result)
            }));
        }

        let mut success_count = 0;
        let mut failure_count = 0;

        for handle in handles {
            match handle.await {
                Ok((id, name, result)) => {
                    if result.is_ok() {
                        success_count += 1;
                    } else {
                        failure_count += 1;
                        debug!(
                            endpoint_id = %id,
                            endpoint_name = %name,
                            "Parallel health check failed"
                        );
                    }
                }
                Err(e) => {
                    error!("Task join error: {}", e);
                    failure_count += 1;
                }
            }
        }

        info!(
            success = success_count,
            failure = failure_count,
            "Parallel health check completed"
        );

        Ok(())
    }

    /// 単一エンドポイントのヘルスチェック
    pub async fn check_endpoint(
        &self,
        endpoint: &Endpoint,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let status_before = endpoint.status;
        let start = Instant::now();

        // GET /v1/models でヘルスチェック
        let url = format!("{}/v1/models", endpoint.base_url.trim_end_matches('/'));

        let mut request = self.client.get(&url);

        // APIキーがあれば認証ヘッダーを追加
        if let Some(ref api_key) = endpoint.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        let result = request.send().await;
        let elapsed = start.elapsed();
        let latency_ms = elapsed.as_millis() as u32;

        let (success, error_message, new_status) = match result {
            Ok(response) if response.status().is_success() => {
                // 成功 → online
                (true, None, EndpointStatus::Online)
            }
            Ok(response) => {
                // HTTPエラー
                let error = format!("HTTP {}", response.status());
                let new_status = self.determine_failure_status(endpoint, status_before);
                (false, Some(error), new_status)
            }
            Err(e) => {
                // 接続エラー
                let error = e.to_string();
                let new_status = self.determine_failure_status(endpoint, status_before);
                (false, Some(error), new_status)
            }
        };

        // ステータス更新
        if success {
            self.registry
                .update_status(endpoint.id, new_status, Some(latency_ms), None)
                .await?;
        } else {
            self.registry
                .update_status(endpoint.id, new_status, None, error_message.as_deref())
                .await?;
        }

        // ヘルスチェック履歴を記録
        let health_check = EndpointHealthCheck {
            id: 0, // DBで自動採番
            endpoint_id: endpoint.id,
            checked_at: Utc::now(),
            success,
            latency_ms: if success { Some(latency_ms) } else { None },
            error_message: error_message.clone(),
            status_before,
            status_after: new_status,
        };

        db::record_health_check(self.registry.pool(), &health_check).await?;

        // ログ出力
        if success {
            debug!(
                endpoint_id = %endpoint.id,
                endpoint_name = %endpoint.name,
                latency_ms = latency_ms,
                status = %new_status.as_str(),
                "Health check succeeded"
            );
        } else {
            warn!(
                endpoint_id = %endpoint.id,
                endpoint_name = %endpoint.name,
                error = ?error_message,
                status = %new_status.as_str(),
                "Health check failed"
            );
        }

        if success {
            Ok(())
        } else {
            Err(error_message
                .unwrap_or_else(|| "Unknown error".to_string())
                .into())
        }
    }

    /// 失敗時の新ステータスを決定
    fn determine_failure_status(
        &self,
        endpoint: &Endpoint,
        status_before: EndpointStatus,
    ) -> EndpointStatus {
        match status_before {
            // pending状態は初回失敗で即offline
            EndpointStatus::Pending => EndpointStatus::Offline,
            // online状態は連続失敗でerror→offline
            EndpointStatus::Online => {
                if endpoint.error_count + 1 >= CONSECUTIVE_FAILURES_FOR_OFFLINE {
                    EndpointStatus::Offline
                } else {
                    EndpointStatus::Error
                }
            }
            // error状態は連続失敗でoffline
            EndpointStatus::Error => {
                if endpoint.error_count + 1 >= CONSECUTIVE_FAILURES_FOR_OFFLINE {
                    EndpointStatus::Offline
                } else {
                    EndpointStatus::Error
                }
            }
            // offline状態はそのまま
            EndpointStatus::Offline => EndpointStatus::Offline,
        }
    }

    /// 特定エンドポイントの手動チェック（APIから呼び出し用）
    pub async fn check_endpoint_by_id(
        &self,
        endpoint_id: Uuid,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let endpoint = self
            .registry
            .get(endpoint_id)
            .await
            .ok_or("Endpoint not found")?;

        self.check_endpoint(&endpoint).await?;
        Ok(true)
    }
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
    async fn test_health_checker_creation() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;
        let registry = EndpointRegistry::new(pool).await.unwrap();
        let checker = EndpointHealthChecker::new(registry);

        assert_eq!(checker.check_interval_secs, DEFAULT_CHECK_INTERVAL_SECS);
    }

    #[tokio::test]
    async fn test_health_checker_with_interval() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;
        let registry = EndpointRegistry::new(pool).await.unwrap();
        let checker = EndpointHealthChecker::new(registry).with_interval(60);

        assert_eq!(checker.check_interval_secs, 60);
    }

    #[tokio::test]
    async fn test_determine_failure_status_pending() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;
        let registry = EndpointRegistry::new(pool).await.unwrap();
        let checker = EndpointHealthChecker::new(registry);

        let endpoint = Endpoint::new("Test".to_string(), "http://localhost:11434".to_string());

        // pending → offline（即時）
        let new_status = checker.determine_failure_status(&endpoint, EndpointStatus::Pending);
        assert_eq!(new_status, EndpointStatus::Offline);
    }

    #[tokio::test]
    async fn test_determine_failure_status_online_first_failure() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;
        let registry = EndpointRegistry::new(pool).await.unwrap();
        let checker = EndpointHealthChecker::new(registry);

        let endpoint = Endpoint::new("Test".to_string(), "http://localhost:11434".to_string());

        // online + error_count=0 → error（1回目の失敗）
        let new_status = checker.determine_failure_status(&endpoint, EndpointStatus::Online);
        assert_eq!(new_status, EndpointStatus::Error);
    }

    #[tokio::test]
    async fn test_determine_failure_status_online_second_failure() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;
        let registry = EndpointRegistry::new(pool).await.unwrap();
        let checker = EndpointHealthChecker::new(registry);

        let mut endpoint = Endpoint::new("Test".to_string(), "http://localhost:11434".to_string());
        endpoint.error_count = 1; // 既に1回失敗

        // online + error_count=1 → offline（2回目の失敗でoffline）
        let new_status = checker.determine_failure_status(&endpoint, EndpointStatus::Online);
        assert_eq!(new_status, EndpointStatus::Offline);
    }
}
