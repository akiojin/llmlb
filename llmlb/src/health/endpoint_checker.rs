//! エンドポイントヘルスチェッカー
//!
//! プル型ヘルスチェックでエンドポイントの稼働状況を監視
//!
//! ## Phase 1.4: `/api/health`対応
//!
//! - `/api/health`を優先的に呼び出し、GPU情報を取得
//! - `/api/health`が失敗した場合は`/v1/models`にフォールバック

use crate::db::endpoints as db;
use crate::detection::detect_endpoint_type_with_client;
use crate::registry::endpoints::EndpointRegistry;
use crate::types::endpoint::{Endpoint, EndpointHealthCheck, EndpointStatus, EndpointType};
use chrono::Utc;
use reqwest::Client;

/// `/api/health`から取得したGPU情報
#[derive(Debug, Clone, Default)]
pub struct GpuInfo {
    /// GPUデバイス数
    pub gpu_device_count: Option<u32>,
    /// GPU総メモリ（バイト）
    pub gpu_total_memory_bytes: Option<u64>,
    /// GPU使用中メモリ（バイト）
    pub gpu_used_memory_bytes: Option<u64>,
    /// GPU能力スコア
    pub gpu_capability_score: Option<f32>,
    /// 現在のアクティブリクエスト数
    pub active_requests: Option<u32>,
}
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
            // Run an initial parallel check to converge quickly without delaying server startup.
            if let Err(e) = self.check_all_endpoints_parallel().await {
                error!("Startup health check error: {}", e);
            }
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

        // `interval()` ticks immediately on the first call. Since we already performed an initial
        // startup check, wait a full interval before the next periodic check.
        timer.tick().await;

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
    ///
    /// Phase 1.4: `/api/health`を優先的に呼び出し、GPU情報を取得。
    /// `/api/health`が失敗した場合は`/v1/models`にフォールバック。
    pub async fn check_endpoint(
        &self,
        endpoint: &Endpoint,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let status_before = endpoint.status;
        let start = Instant::now();

        // まず /api/health を試行（GPU情報取得可能）
        let health_result = self.try_v0_health(endpoint).await;
        let elapsed = start.elapsed();
        let latency_ms = elapsed.as_millis() as u32;

        let (success, error_message, new_status, gpu_info) = match health_result {
            Ok(gpu_info) => {
                // /api/health 成功 → online、GPU情報あり
                (true, None, EndpointStatus::Online, Some(gpu_info))
            }
            Err(_v0_error) => {
                // /api/health 失敗 → /v1/models にフォールバック
                debug!(
                    endpoint_id = %endpoint.id,
                    endpoint_name = %endpoint.name,
                    "/api/health failed, falling back to /v1/models"
                );
                match self.try_v1_models(endpoint).await {
                    Ok(()) => {
                        // /v1/models 成功 → online、GPU情報なし
                        (true, None, EndpointStatus::Online, None)
                    }
                    Err(e) => {
                        // 両方失敗
                        let error = e.to_string();
                        let new_status = self.determine_failure_status(endpoint, status_before);
                        (false, Some(error), new_status, None)
                    }
                }
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

        // GPU情報更新（/api/healthから取得した場合のみ）
        if let Some(info) = gpu_info {
            self.registry
                .update_gpu_info(
                    endpoint.id,
                    info.gpu_device_count,
                    info.gpu_total_memory_bytes,
                    info.gpu_used_memory_bytes,
                    info.gpu_capability_score,
                    info.active_requests,
                )
                .await;
        }

        // SPEC-66555000: タイプ再判別（Unknown→オンライン時に再判別）
        if success && endpoint.endpoint_type == EndpointType::Unknown {
            let detection = detect_endpoint_type_with_client(
                &self.client,
                &endpoint.base_url,
                endpoint.api_key.as_deref(),
            )
            .await;

            if detection.endpoint_type != EndpointType::Unknown {
                info!(
                    endpoint_id = %endpoint.id,
                    endpoint_name = %endpoint.name,
                    detected_type = %detection.endpoint_type.as_str(),
                    "Endpoint type re-detected on health check"
                );
                if let Err(e) = self
                    .registry
                    .update_endpoint_type(
                        endpoint.id,
                        detection.endpoint_type,
                        crate::types::endpoint::EndpointTypeSource::Auto,
                        detection.reason,
                        Some(chrono::Utc::now()),
                    )
                    .await
                {
                    warn!(
                        endpoint_id = %endpoint.id,
                        error = %e,
                        "Failed to update endpoint type"
                    );
                }
            }
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

    /// `/api/health`を呼び出してGPU情報を取得
    async fn try_v0_health(
        &self,
        endpoint: &Endpoint,
    ) -> Result<GpuInfo, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/api/health", endpoint.base_url.trim_end_matches('/'));

        let mut request = self.client.get(&url);
        if let Some(ref api_key) = endpoint.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = request.send().await?;
        if !response.status().is_success() {
            return Err(format!("HTTP {}", response.status()).into());
        }

        let body: serde_json::Value = response.json().await?;

        // GPU情報を抽出
        let gpu = body.get("gpu");
        let load = body.get("load");

        Ok(GpuInfo {
            gpu_device_count: gpu
                .and_then(|g| g.get("device_count"))
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
            gpu_total_memory_bytes: gpu
                .and_then(|g| g.get("total_memory_bytes"))
                .and_then(|v| v.as_u64()),
            gpu_used_memory_bytes: gpu
                .and_then(|g| g.get("used_memory_bytes"))
                .and_then(|v| v.as_u64()),
            gpu_capability_score: gpu
                .and_then(|g| g.get("capability_score"))
                .and_then(|v| v.as_f64())
                .map(|v| v as f32),
            active_requests: load
                .and_then(|l| l.get("active_requests"))
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
        })
    }

    /// `/v1/models`を呼び出してヘルスチェック（フォールバック用）
    async fn try_v1_models(
        &self,
        endpoint: &Endpoint,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/v1/models", endpoint.base_url.trim_end_matches('/'));

        let mut request = self.client.get(&url);
        if let Some(ref api_key) = endpoint.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = request.send().await?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!("HTTP {}", response.status()).into())
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
