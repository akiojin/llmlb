//! ロードバランサーモジュール
//!
//! エンドポイントのメトリクスとリクエスト統計を集約し、
//! レイテンシベースのロードバランシングを提供する。
//!
//! # EndpointRegistry統合
//!
//! このモジュールはEndpointRegistryを使用してエンドポイント情報を管理します。
//! 負荷分散はレイテンシ優先（EMA α=0.2）で行われます。

use crate::common::{
    error::{LbError, RouterResult},
    types::HealthMetrics,
};
use crate::registry::endpoints::EndpointRegistry;
use chrono::{DateTime, Duration as ChronoDuration, Timelike, Utc};
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicUsize, Ordering as AtomicOrdering},
        Arc,
    },
    time::Duration as StdDuration,
};
use tokio::sync::{Notify, RwLock};
use uuid::Uuid;

/// メトリクスを新鮮とみなすための許容秒数
const METRICS_STALE_THRESHOLD_SECS: i64 = 120;
/// リクエスト履歴の保持分数
const REQUEST_HISTORY_WINDOW_MINUTES: i64 = 60;
/// ノードメトリクス履歴の最大保持件数
const METRICS_HISTORY_CAPACITY: usize = 360;

/// リクエスト結果
#[derive(Debug, Clone, Copy)]
pub enum RequestOutcome {
    /// 正常終了
    Success,
    /// エラー終了
    Error,
    /// キュー待ち
    Queued,
}

/// 待機結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitResult {
    /// ノードが利用可能になった
    Ready,
    /// タイムアウト
    Timeout,
    /// 待機キュー容量超過
    CapacityExceeded,
}

#[derive(Debug)]
struct QueueWaiterGuard {
    waiters: Arc<AtomicUsize>,
}

impl QueueWaiterGuard {
    fn new(waiters: Arc<AtomicUsize>) -> Self {
        Self { waiters }
    }
}

impl Drop for QueueWaiterGuard {
    fn drop(&mut self) {
        self.waiters.fetch_sub(1, AtomicOrdering::SeqCst);
    }
}

/// アドミッション制御の判断結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionDecision {
    /// 即座に受け入れ
    Accept,
    /// 遅延付きで受け入れ
    AcceptWithDelay(StdDuration),
    /// リジェクト
    Reject,
}

// SPEC-f8e3a1b7: Node依存のヘルパー関数は削除されました
// - node_spec_score, compare_spec_levels, compare_spec_by_state
// - compare_option_f32, compare_average_ms, usage_snapshot, compare_usage_levels
// 新しい負荷分散はレイテンシベース（EMA α=0.2）を使用

/// エンドポイント×モデル単位のTPS EMA状態（SPEC-4bb5b55f）
#[derive(Debug, Clone, Default)]
pub struct ModelTpsState {
    /// EMA平滑化されたTPS値（None=未計測）
    pub tps_ema: Option<f64>,
    /// リクエスト完了数
    pub request_count: u64,
    /// 出力トークン累計
    pub total_output_tokens: u64,
    /// 処理時間累計（ミリ秒）
    pub total_duration_ms: u64,
}

impl ModelTpsState {
    /// TPS計測値を更新（EMA α=0.2）
    ///
    /// TPS = output_tokens / (duration_ms / 1000)
    /// EMA: new_ema = α × current_tps + (1 - α) × previous_ema
    pub fn update_tps(&mut self, output_tokens: u64, duration_ms: u64) {
        if duration_ms == 0 {
            return;
        }

        let current_tps = output_tokens as f64 / (duration_ms as f64 / 1000.0);

        const ALPHA: f64 = 0.2;
        self.tps_ema = Some(match self.tps_ema {
            Some(prev) => ALPHA * current_tps + (1.0 - ALPHA) * prev,
            None => current_tps,
        });

        self.request_count += 1;
        self.total_output_tokens += output_tokens;
        self.total_duration_ms += duration_ms;
    }
}

/// エンドポイント×モデル単位のTPS情報（API応答用）（SPEC-4bb5b55f）
#[derive(Debug, Clone, Serialize)]
pub struct ModelTpsInfo {
    /// モデルID
    pub model_id: String,
    /// EMA平滑化されたTPS値（None=未計測）
    pub tps: Option<f64>,
    /// リクエスト完了数
    pub request_count: u64,
    /// 出力トークン累計
    pub total_output_tokens: u64,
    /// 平均処理時間（ミリ秒、None=未計測）
    pub average_duration_ms: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_utils::TEST_LOCK;
    use crate::types::endpoint::{
        Endpoint, EndpointModel, EndpointStatus, EndpointType, SupportedAPI,
    };
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use std::time::Duration as StdDuration;
    use tokio::time::{sleep, Duration};

    async fn setup_test_load_manager() -> (LoadManager, Uuid) {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to create test database");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        let registry = EndpointRegistry::new(pool)
            .await
            .expect("Failed to create endpoint registry");
        let endpoint = Endpoint::new(
            "lease-test-endpoint".to_string(),
            "http://localhost:11434".to_string(),
            EndpointType::OpenaiCompatible,
        );
        let endpoint_id = endpoint.id;
        registry
            .add(endpoint)
            .await
            .expect("Failed to add test endpoint");

        let load_manager = LoadManager::new(Arc::new(registry));
        (load_manager, endpoint_id)
    }

    // NOTE: SPEC-e8e9326eによりNodeRegistryは廃止されました。
    // SPEC-f8e3a1b7によりNode型は削除され、Endpoint型に移行しました。
    // compare_average_ms_orders_values テストは関数削除に伴い削除されました。

    #[test]
    fn effective_average_ms_prefers_metrics_value() {
        let timestamp = Utc::now();
        let state = EndpointLoadState {
            success_count: 5,
            total_latency_ms: 500,
            last_metrics: Some(HealthMetrics {
                node_id: Uuid::new_v4(),
                cpu_usage: 10.0,
                memory_usage: 20.0,
                gpu_usage: None,
                gpu_memory_usage: None,
                gpu_memory_total_mb: None,
                gpu_memory_used_mb: None,
                gpu_temperature: None,
                gpu_model_name: None,
                gpu_compute_capability: None,
                gpu_capability_score: None,
                active_requests: 1,
                total_requests: 5,
                average_response_time_ms: Some(80.0),
                timestamp,
            }),
            ..Default::default()
        };

        assert_eq!(state.effective_average_ms(), Some(80.0));
    }

    // SPEC-f8e3a1b7: NodeRegistry依存のテストは削除されました
    // - load_manager_prefers_lower_latency_when_active_equal
    // - metrics_history_tracks_recent_points
    // - wait_for_ready_unblocks_when_node_becomes_ready
    // - wait_for_ready_limits_waiters_and_notifies_first
    // 新しいEndpointRegistryベースのテストは tests/integration/ に追加してください

    // T004: WaitResult enum テスト
    #[test]
    fn wait_result_enum_variants_exist() {
        // WaitResultの3つのバリアントが存在することを確認
        let ready = WaitResult::Ready;
        let timeout = WaitResult::Timeout;
        let capacity_exceeded = WaitResult::CapacityExceeded;

        // PartialEq実装の確認
        assert_eq!(ready, WaitResult::Ready);
        assert_eq!(timeout, WaitResult::Timeout);
        assert_eq!(capacity_exceeded, WaitResult::CapacityExceeded);
        assert_ne!(ready, timeout);

        // Debug実装の確認
        assert!(!format!("{:?}", ready).is_empty());
    }

    // T004: AdmissionDecision enum テスト
    #[test]
    fn admission_decision_enum_variants_exist() {
        // AdmissionDecisionの3つのバリアントが存在することを確認
        let accept = AdmissionDecision::Accept;
        let accept_with_delay = AdmissionDecision::AcceptWithDelay(StdDuration::from_millis(100));
        let reject = AdmissionDecision::Reject;

        // PartialEq実装の確認
        assert_eq!(accept, AdmissionDecision::Accept);
        assert_eq!(reject, AdmissionDecision::Reject);
        assert_ne!(accept, reject);

        // AcceptWithDelayのDuration値を確認
        if let AdmissionDecision::AcceptWithDelay(duration) = accept_with_delay {
            assert_eq!(duration, StdDuration::from_millis(100));
        } else {
            panic!("Expected AcceptWithDelay variant");
        }

        // Debug実装の確認
        assert!(!format!("{:?}", accept).is_empty());
    }

    // SPEC-f8e3a1b7: wait_for_ready_with_timeout / admission_control テストは削除されました
    // - wait_for_ready_with_timeout_returns_timeout_when_no_ready_nodes
    // - wait_for_ready_with_timeout_returns_capacity_exceeded
    // - wait_for_ready_with_timeout_returns_ready_when_node_becomes_available
    // - admission_control_returns_accept_when_below_50_percent
    // - admission_control_returns_accept_with_delay_when_between_50_and_80_percent
    // - admission_control_returns_reject_when_above_80_percent
    // - admission_control_boundary_values

    #[test]
    fn test_node_load_state_token_accumulation() {
        // T-2: EndpointLoadStateトークン累積テスト
        let mut state = EndpointLoadState::default();

        // 初期値は0
        assert_eq!(state.total_input_tokens, 0);
        assert_eq!(state.total_output_tokens, 0);
        assert_eq!(state.total_tokens, 0);

        // トークンを累積
        state.total_input_tokens += 100;
        state.total_output_tokens += 50;
        state.total_tokens += 150;

        assert_eq!(state.total_input_tokens, 100);
        assert_eq!(state.total_output_tokens, 50);
        assert_eq!(state.total_tokens, 150);

        // 追加の累積
        state.total_input_tokens += 200;
        state.total_output_tokens += 100;
        state.total_tokens += 300;

        assert_eq!(state.total_input_tokens, 300);
        assert_eq!(state.total_output_tokens, 150);
        assert_eq!(state.total_tokens, 450);
    }

    #[test]
    fn test_node_load_state_average_tokens_per_request() {
        // トークン/リクエスト平均計算テスト
        let state = EndpointLoadState {
            total_assigned: 10,
            total_input_tokens: 1000,
            total_output_tokens: 500,
            total_tokens: 1500,
            ..Default::default()
        };

        // 平均トークン数の計算（total_assignedが0でない場合）
        let avg = if state.total_assigned > 0 {
            state.total_tokens as f32 / state.total_assigned as f32
        } else {
            0.0
        };
        assert_eq!(avg, 150.0);
    }

    #[tokio::test]
    async fn request_lease_complete_releases_active_counter() {
        let _lock = TEST_LOCK.lock().await;
        let (load_manager, endpoint_id) = setup_test_load_manager().await;

        let lease = load_manager
            .begin_request(endpoint_id)
            .await
            .expect("begin_request should succeed");

        let active_before = load_manager
            .snapshot(endpoint_id)
            .await
            .expect("snapshot should succeed")
            .active_requests;
        assert_eq!(active_before, 1);

        lease
            .complete(RequestOutcome::Success, StdDuration::from_millis(3))
            .await
            .expect("complete should succeed");

        let snapshot_after = load_manager
            .snapshot(endpoint_id)
            .await
            .expect("snapshot should succeed");
        assert_eq!(snapshot_after.active_requests, 0);
        assert_eq!(snapshot_after.successful_requests, 1);
    }

    #[tokio::test]
    async fn request_lease_drop_auto_releases_active_counter() {
        let _lock = TEST_LOCK.lock().await;
        let (load_manager, endpoint_id) = setup_test_load_manager().await;

        {
            let _lease = load_manager
                .begin_request(endpoint_id)
                .await
                .expect("begin_request should succeed");
        }

        let mut final_snapshot = None;
        for _ in 0..30 {
            let snapshot = load_manager
                .snapshot(endpoint_id)
                .await
                .expect("snapshot should succeed");
            if snapshot.active_requests == 0 {
                final_snapshot = Some(snapshot);
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }

        let snapshot = final_snapshot.expect("lease auto-complete should drain active requests");
        assert_eq!(snapshot.active_requests, 0);
        assert_eq!(snapshot.failed_requests, 1);
    }

    #[tokio::test]
    async fn select_endpoint_round_robin_ready_for_model_excludes_initializing_endpoints() {
        let _lock = TEST_LOCK.lock().await;
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to create test database");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        let registry = EndpointRegistry::new(pool)
            .await
            .expect("Failed to create endpoint registry");

        let mut ready_endpoint = Endpoint::new(
            "ready-endpoint".to_string(),
            "http://localhost:11080".to_string(),
            EndpointType::OpenaiCompatible,
        );
        ready_endpoint.status = EndpointStatus::Online;
        let ready_endpoint_id = ready_endpoint.id;
        registry
            .add(ready_endpoint)
            .await
            .expect("Failed to add ready endpoint");

        let mut initializing_endpoint = Endpoint::new(
            "initializing-endpoint".to_string(),
            "http://localhost:11081".to_string(),
            EndpointType::OpenaiCompatible,
        );
        initializing_endpoint.status = EndpointStatus::Online;
        let initializing_endpoint_id = initializing_endpoint.id;
        registry
            .add(initializing_endpoint)
            .await
            .expect("Failed to add initializing endpoint");

        let model_id = "gpt-oss:latest".to_string();
        for endpoint_id in [ready_endpoint_id, initializing_endpoint_id] {
            registry
                .add_model(&EndpointModel {
                    endpoint_id,
                    model_id: model_id.clone(),
                    capabilities: None,
                    max_tokens: None,
                    last_checked: None,
                    supported_apis: vec![SupportedAPI::ChatCompletions],
                })
                .await
                .expect("Failed to add endpoint model");
        }

        let load_manager = LoadManager::new(Arc::new(registry));
        load_manager
            .upsert_initial_state(ready_endpoint_id, false, Some((1, 1)))
            .await;
        load_manager
            .upsert_initial_state(initializing_endpoint_id, true, Some((0, 1)))
            .await;

        for _ in 0..4 {
            let selected = load_manager
                .select_endpoint_round_robin_ready_for_model(&model_id)
                .await
                .expect("selection should succeed");
            assert_eq!(
                selected.id, ready_endpoint_id,
                "initializing endpoint must not be selected"
            );
        }
    }

    // SPEC-f8e3a1b7: NodeRegistry依存のトークン・ルーティングテストは削除されました
    // - test_finish_request_accumulates_tokens
    // - test_finish_request_accumulates_multiple_tokens
    // - test_finish_request_accumulates_tokens_on_error
    // - test_offline_node_retains_token_statistics
    // - test_pending_node_excluded_from_routing
    // - test_registering_node_excluded_from_routing

    // SPEC-4bb5b55f T002: ModelTpsState EMA計算テスト

    #[test]
    fn test_model_tps_state_initial_none() {
        // 初期状態ではtps_emaはNone
        let state = ModelTpsState::default();
        assert!(state.tps_ema.is_none());
        assert_eq!(state.request_count, 0);
        assert_eq!(state.total_output_tokens, 0);
        assert_eq!(state.total_duration_ms, 0);
    }

    #[test]
    fn test_model_tps_state_first_update() {
        // 初回計測でNone→Some値になること
        let mut state = ModelTpsState::default();
        // 100 tokens in 2000ms = 50 tok/s
        state.update_tps(100, 2000);
        assert!(state.tps_ema.is_some());
        let tps = state.tps_ema.unwrap();
        assert!(
            (tps - 50.0).abs() < 0.01,
            "初回TPS: expected 50.0, got {tps}"
        );
        assert_eq!(state.request_count, 1);
        assert_eq!(state.total_output_tokens, 100);
        assert_eq!(state.total_duration_ms, 2000);
    }

    #[test]
    fn test_model_tps_state_ema_smoothing() {
        // 複数回更新でEMA平滑化されること (α=0.2)
        let mut state = ModelTpsState::default();

        // 1回目: 100 tokens / 2000ms = 50.0 tok/s → EMA = 50.0
        state.update_tps(100, 2000);
        assert!((state.tps_ema.unwrap() - 50.0).abs() < 0.01);

        // 2回目: 200 tokens / 2000ms = 100.0 tok/s
        // EMA = 0.2 * 100.0 + 0.8 * 50.0 = 20.0 + 40.0 = 60.0
        state.update_tps(200, 2000);
        assert!(
            (state.tps_ema.unwrap() - 60.0).abs() < 0.01,
            "2回目EMA: expected 60.0, got {}",
            state.tps_ema.unwrap()
        );

        // 3回目: 50 tokens / 1000ms = 50.0 tok/s
        // EMA = 0.2 * 50.0 + 0.8 * 60.0 = 10.0 + 48.0 = 58.0
        state.update_tps(50, 1000);
        assert!(
            (state.tps_ema.unwrap() - 58.0).abs() < 0.01,
            "3回目EMA: expected 58.0, got {}",
            state.tps_ema.unwrap()
        );

        // 累計値の確認
        assert_eq!(state.request_count, 3);
        assert_eq!(state.total_output_tokens, 350); // 100 + 200 + 50
        assert_eq!(state.total_duration_ms, 5000); // 2000 + 2000 + 1000
    }

    #[test]
    fn test_model_tps_state_zero_duration_skipped() {
        // duration_ms=0の場合はTPS更新をスキップ（ゼロ除算防止）
        let mut state = ModelTpsState::default();
        state.update_tps(100, 0);
        assert!(state.tps_ema.is_none(), "duration=0ではTPS更新しない");
    }

    // T013: LoadManager::get_model_tps() / update_tps() テスト（SPEC-4bb5b55f Phase 3）

    #[tokio::test]
    async fn test_get_model_tps_empty_for_unknown_endpoint() {
        let _lock = TEST_LOCK.lock().await;
        let (load_manager, _) = setup_test_load_manager().await;
        let unknown_id = Uuid::new_v4();
        let result = load_manager.get_model_tps(unknown_id).await;
        assert!(result.is_empty(), "未計測エンドポイントは空Vecを返す");
    }

    #[tokio::test]
    async fn test_get_model_tps_returns_entries_after_update() {
        let _lock = TEST_LOCK.lock().await;
        let (load_manager, endpoint_id) = setup_test_load_manager().await;

        // TPS更新
        load_manager
            .update_tps(endpoint_id, "model-a".to_string(), 100, 2000)
            .await;

        let result = load_manager.get_model_tps(endpoint_id).await;
        assert_eq!(result.len(), 1, "1モデル分のTPS情報が返る");
        assert_eq!(result[0].model_id, "model-a");
        assert_eq!(result[0].request_count, 1);
        assert_eq!(result[0].total_output_tokens, 100);
        // TPS = 100 / (2000 / 1000) = 50.0
        let tps = result[0].tps.expect("TPS値がSomeであること");
        assert!((tps - 50.0).abs() < 0.01, "TPS = 50.0");
    }

    #[tokio::test]
    async fn test_get_model_tps_multiple_models() {
        let _lock = TEST_LOCK.lock().await;
        let (load_manager, endpoint_id) = setup_test_load_manager().await;

        load_manager
            .update_tps(endpoint_id, "model-a".to_string(), 100, 2000)
            .await;
        load_manager
            .update_tps(endpoint_id, "model-b".to_string(), 200, 1000)
            .await;

        let result = load_manager.get_model_tps(endpoint_id).await;
        assert_eq!(result.len(), 2, "2モデル分のTPS情報が返る");

        let model_ids: Vec<&str> = result.iter().map(|e| e.model_id.as_str()).collect();
        assert!(model_ids.contains(&"model-a"));
        assert!(model_ids.contains(&"model-b"));
    }

    #[tokio::test]
    async fn test_update_tps_skips_zero_tokens() {
        let _lock = TEST_LOCK.lock().await;
        let (load_manager, endpoint_id) = setup_test_load_manager().await;

        // output_tokens=0は無視
        load_manager
            .update_tps(endpoint_id, "model-a".to_string(), 0, 2000)
            .await;

        let result = load_manager.get_model_tps(endpoint_id).await;
        assert!(result.is_empty(), "output_tokens=0はTPS更新しない");
    }

    #[tokio::test]
    async fn test_get_model_tps_isolates_endpoints() {
        let _lock = TEST_LOCK.lock().await;
        let (load_manager, endpoint_id) = setup_test_load_manager().await;
        let other_endpoint_id = Uuid::new_v4();

        load_manager
            .update_tps(endpoint_id, "model-a".to_string(), 100, 2000)
            .await;
        load_manager
            .update_tps(other_endpoint_id, "model-b".to_string(), 200, 1000)
            .await;

        let result = load_manager.get_model_tps(endpoint_id).await;
        assert_eq!(result.len(), 1, "他エンドポイントのデータは含まない");
        assert_eq!(result[0].model_id, "model-a");
    }
}

/// エンドポイントの負荷状態
#[derive(Debug, Clone, Default)]
struct EndpointLoadState {
    last_metrics: Option<HealthMetrics>,
    assigned_active: u32,
    total_assigned: u64,
    success_count: u64,
    error_count: u64,
    total_latency_ms: u128,
    metrics_history: VecDeque<HealthMetrics>,
    initializing: bool,
    ready_models: Option<(u8, u8)>,
    /// 入力トークン累計
    total_input_tokens: u64,
    /// 出力トークン累計
    total_output_tokens: u64,
    /// 総トークン累計
    total_tokens: u64,
}

// SPEC-f8e3a1b7: NodeLoadState型エイリアスは削除されました

impl EndpointLoadState {
    fn combined_active(&self) -> u32 {
        let heartbeat_active = self
            .last_metrics
            .as_ref()
            .map(|m| m.active_requests)
            .unwrap_or(0);
        // Avoid double counting when node heartbeat mirrors lb-assigned requests.
        heartbeat_active.max(self.assigned_active)
    }

    fn average_latency_ms(&self) -> Option<f32> {
        let completed = self.success_count + self.error_count;
        if completed == 0 {
            None
        } else {
            Some((self.total_latency_ms as f64 / completed as f64) as f32)
        }
    }

    fn last_updated(&self) -> Option<DateTime<Utc>> {
        self.last_metrics.as_ref().map(|m| m.timestamp)
    }

    fn is_stale(&self, now: DateTime<Utc>) -> bool {
        match self.last_updated() {
            Some(ts) => (now - ts).num_seconds() > METRICS_STALE_THRESHOLD_SECS,
            None => true,
        }
    }

    fn effective_average_ms(&self) -> Option<f32> {
        self.last_metrics
            .as_ref()
            .and_then(|m| m.average_response_time_ms)
            .or_else(|| self.average_latency_ms())
    }

    fn push_metrics(&mut self, metrics: HealthMetrics) {
        self.metrics_history.push_back(metrics);
        if self.metrics_history.len() > METRICS_HISTORY_CAPACITY {
            self.metrics_history.pop_front();
        }
    }
}

/// エンドポイント/ノードのロードスナップショット
///
/// エンドポイントの負荷スナップショット
///
/// SPEC-f8e3a1b7: Node型依存を削除し、Endpoint型を直接使用
#[derive(Debug, Clone, Serialize)]
pub struct EndpointLoadSnapshot {
    /// エンドポイントID（API互換性のためnode_idとしてシリアライズ）
    #[serde(rename = "node_id")]
    pub endpoint_id: Uuid,
    /// エンドポイント名
    pub machine_name: String,
    /// エンドポイント状態
    pub status: crate::types::endpoint::EndpointStatus,
    /// CPU使用率
    pub cpu_usage: Option<f32>,
    /// メモリ使用率
    pub memory_usage: Option<f32>,
    /// GPU使用率
    pub gpu_usage: Option<f32>,
    /// GPUメモリ使用率
    pub gpu_memory_usage: Option<f32>,
    /// GPUメモリ総容量 (MB)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_memory_total_mb: Option<u64>,
    /// GPU使用メモリ (MB)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_memory_used_mb: Option<u64>,
    /// GPU温度 (℃)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_temperature: Option<f32>,
    /// GPUモデル名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_model_name: Option<String>,
    /// GPU計算能力
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_compute_capability: Option<String>,
    /// GPU能力スコア
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_capability_score: Option<u32>,
    /// 処理中リクエスト数（load balancer観点+ノード自己申告）
    pub active_requests: u32,
    /// 累積リクエスト数
    pub total_requests: u64,
    /// 成功リクエスト数
    pub successful_requests: u64,
    /// 失敗リクエスト数
    pub failed_requests: u64,
    /// 平均レスポンスタイム (ms)
    pub average_response_time_ms: Option<f32>,
    /// メトリクス最終更新時刻
    pub last_updated: Option<DateTime<Utc>>,
    /// メトリクスが鮮度閾値を超えているか
    pub is_stale: bool,
    /// 入力トークン累計
    pub total_input_tokens: u64,
    /// 出力トークン累計
    pub total_output_tokens: u64,
    /// 総トークン累計
    pub total_tokens: u64,
}

/// ノードのロードスナップショット（後方互換エイリアス）
///
/// NodeRegistry廃止移行のための後方互換エイリアス。
/// 新規コードは`EndpointLoadSnapshot`を使用すること。
#[deprecated(note = "Use EndpointLoadSnapshot instead")]
pub type NodeLoadSnapshot = EndpointLoadSnapshot;

/// システム全体の統計サマリー
#[derive(Debug, Clone, Serialize, Default)]
pub struct SystemSummary {
    /// 登録ノード総数
    pub total_nodes: usize,
    /// オンラインノード数
    pub online_nodes: usize,
    /// 承認待ちノード数
    pub pending_nodes: usize,
    /// 登録中ノード数（モデル同期中）
    pub registering_nodes: usize,
    /// オフラインノード数
    pub offline_nodes: usize,
    /// 累積リクエスト数
    pub total_requests: u64,
    /// 成功リクエスト数
    pub successful_requests: u64,
    /// 失敗リクエスト数
    pub failed_requests: u64,
    /// 平均レスポンスタイム (ms)
    pub average_response_time_ms: Option<f32>,
    /// 平均GPU使用率 (0-100)
    pub average_gpu_usage: Option<f32>,
    /// 平均GPUメモリ使用率 (0-100)
    pub average_gpu_memory_usage: Option<f32>,
    /// 処理中リクエスト総数
    pub total_active_requests: u32,
    /// 待機中リクエスト総数
    pub queued_requests: usize,
    /// 最新メトリクス更新時刻
    pub last_metrics_updated_at: Option<DateTime<Utc>>,
    /// 入力トークン累計
    pub total_input_tokens: u64,
    /// 出力トークン累計
    pub total_output_tokens: u64,
    /// 総トークン累計
    pub total_tokens: u64,
}

/// ロードマネージャー
///
/// # EndpointRegistry統合
///
/// EndpointRegistryを使用してエンドポイント情報を管理します。
#[derive(Clone)]
pub struct LoadManager {
    /// エンドポイントレジストリ
    endpoint_registry: Arc<EndpointRegistry>,
    state: Arc<RwLock<HashMap<Uuid, EndpointLoadState>>>,
    round_robin: Arc<AtomicUsize>,
    history: Arc<RwLock<VecDeque<RequestHistoryPoint>>>,
    /// 待機中リクエスト数（簡易カウンタ）
    #[allow(dead_code)]
    pending: Arc<AtomicUsize>,
    /// ready通知
    ready_notify: Arc<Notify>,
    /// 待機中リクエスト数（上限判定用）
    waiters: Arc<AtomicUsize>,
    /// リクエストキュー待機中の通知
    queue_notify: Arc<Notify>,
    /// リクエストキュー待機数
    queue_waiters: Arc<AtomicUsize>,
    /// エンドポイント×モデル単位のTPS状態（SPEC-4bb5b55f）
    tps_tracker: Arc<RwLock<HashMap<(Uuid, String), ModelTpsState>>>,
}

/// リクエスト処理中のlease
///
/// `complete*` が呼ばれずに破棄された場合でも、Drop時にエラーとして
/// activeカウンタを減算することでカウンタ残留を防ぐ。
pub struct RequestLease {
    load_manager: Option<LoadManager>,
    endpoint_id: Uuid,
    started_at: std::time::Instant,
}

impl RequestLease {
    fn new(load_manager: LoadManager, endpoint_id: Uuid) -> Self {
        Self {
            load_manager: Some(load_manager),
            endpoint_id,
            started_at: std::time::Instant::now(),
        }
    }

    /// 紐づくエンドポイントIDを返す。
    pub fn endpoint_id(&self) -> Uuid {
        self.endpoint_id
    }

    /// lease開始からの経過時間を返す。
    pub fn elapsed(&self) -> StdDuration {
        self.started_at.elapsed()
    }

    /// リクエストを指定結果で明示的に完了する。
    pub async fn complete(
        mut self,
        outcome: RequestOutcome,
        duration: StdDuration,
    ) -> RouterResult<()> {
        let Some(load_manager) = self.load_manager.take() else {
            return Ok(());
        };
        load_manager
            .finish_request(self.endpoint_id, outcome, duration)
            .await
    }

    /// トークン使用量付きでリクエストを明示的に完了する。
    pub async fn complete_with_tokens(
        mut self,
        outcome: RequestOutcome,
        duration: StdDuration,
        token_usage: Option<crate::token::TokenUsage>,
    ) -> RouterResult<()> {
        let Some(load_manager) = self.load_manager.take() else {
            return Ok(());
        };
        load_manager
            .finish_request_with_tokens(self.endpoint_id, outcome, duration, token_usage)
            .await
    }
}

impl Drop for RequestLease {
    fn drop(&mut self) {
        let Some(load_manager) = self.load_manager.take() else {
            return;
        };

        let endpoint_id = self.endpoint_id;
        let duration = self.started_at.elapsed();

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                if let Err(err) = load_manager
                    .finish_request(endpoint_id, RequestOutcome::Error, duration)
                    .await
                {
                    tracing::warn!(
                        endpoint_id = %endpoint_id,
                        error = %err,
                        "Failed to auto-complete leaked request lease"
                    );
                }
            });
        } else {
            tracing::warn!(
                endpoint_id = %endpoint_id,
                "Request lease dropped without runtime; skipping auto-complete"
            );
        }
    }
}

/// ハートビートから記録するメトリクス値
#[derive(Debug, Clone)]
pub struct MetricsUpdate {
    /// 対象ノードのID
    pub node_id: Uuid,
    /// CPU使用率（パーセンテージ）
    pub cpu_usage: f32,
    /// メモリ使用率（パーセンテージ）
    pub memory_usage: f32,
    /// GPU使用率（パーセンテージ）
    pub gpu_usage: Option<f32>,
    /// GPUメモリ使用率（パーセンテージ）
    pub gpu_memory_usage: Option<f32>,
    /// GPUメモリ総容量 (MB)
    pub gpu_memory_total_mb: Option<u64>,
    /// GPU使用メモリ (MB)
    pub gpu_memory_used_mb: Option<u64>,
    /// GPU温度 (℃)
    pub gpu_temperature: Option<f32>,
    /// GPUモデル名
    pub gpu_model_name: Option<String>,
    /// GPU計算能力
    pub gpu_compute_capability: Option<String>,
    /// GPU能力スコア
    pub gpu_capability_score: Option<u32>,
    /// アクティブなリクエスト数
    pub active_requests: u32,
    /// 平均レスポンスタイム（ミリ秒）
    pub average_response_time_ms: Option<f32>,
    /// 初期化中フラグ
    pub initializing: bool,
    /// 起動済みモデル数/総数
    pub ready_models: Option<(u8, u8)>,
}

impl LoadManager {
    /// 新しいロードマネージャーを作成
    pub fn new(endpoint_registry: Arc<EndpointRegistry>) -> Self {
        Self {
            endpoint_registry,
            state: Arc::new(RwLock::new(HashMap::new())),
            round_robin: Arc::new(AtomicUsize::new(0)),
            history: Arc::new(RwLock::new(VecDeque::new())),
            pending: Arc::new(AtomicUsize::new(0)),
            ready_notify: Arc::new(Notify::new()),
            waiters: Arc::new(AtomicUsize::new(0)),
            queue_notify: Arc::new(Notify::new()),
            queue_waiters: Arc::new(AtomicUsize::new(0)),
            tps_tracker: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// TPS計測値を更新（SPEC-4bb5b55f）
    pub async fn update_tps(
        &self,
        endpoint_id: Uuid,
        model_id: String,
        output_tokens: u64,
        duration_ms: u64,
    ) {
        if duration_ms == 0 || output_tokens == 0 {
            return;
        }
        let mut tracker = self.tps_tracker.write().await;
        let state = tracker.entry((endpoint_id, model_id)).or_default();
        state.update_tps(output_tokens, duration_ms);
    }

    /// エンドポイントのモデル別TPS情報を取得（SPEC-4bb5b55f）
    pub async fn get_model_tps(&self, endpoint_id: Uuid) -> Vec<ModelTpsInfo> {
        let tracker = self.tps_tracker.read().await;
        tracker
            .iter()
            .filter(|((eid, _), _)| *eid == endpoint_id)
            .map(|((_, model_id), state)| ModelTpsInfo {
                model_id: model_id.clone(),
                tps: state.tps_ema,
                request_count: state.request_count,
                total_output_tokens: state.total_output_tokens,
                average_duration_ms: if state.request_count > 0 {
                    Some(state.total_duration_ms as f64 / state.request_count as f64)
                } else {
                    None
                },
            })
            .collect()
    }

    /// テスト用: 指定エンドポイントがアクティブになるまで待機する
    #[cfg(test)]
    pub async fn wait_for_endpoint_active(
        &self,
        endpoint_id: Uuid,
        timeout_duration: StdDuration,
    ) -> bool {
        let start = std::time::Instant::now();
        loop {
            if let Ok(snapshot) = self.snapshot(endpoint_id).await {
                if snapshot.active_requests > 0 {
                    return true;
                }
            }

            if start.elapsed() > timeout_duration {
                return false;
            }

            tokio::time::sleep(StdDuration::from_millis(10)).await;
        }
    }

    /// エンドポイントレジストリへの参照を取得
    pub fn endpoint_registry(&self) -> &Arc<EndpointRegistry> {
        &self.endpoint_registry
    }

    /// ヘルスメトリクスを記録
    pub async fn record_metrics(&self, update: MetricsUpdate) -> RouterResult<()> {
        let MetricsUpdate {
            node_id,
            cpu_usage,
            memory_usage,
            gpu_usage,
            gpu_memory_usage,
            gpu_memory_total_mb,
            gpu_memory_used_mb,
            gpu_temperature,
            gpu_model_name,
            gpu_compute_capability,
            gpu_capability_score,
            active_requests,
            average_response_time_ms,
            initializing,
            ready_models,
        } = update;

        // エンドポイントが存在することを確認
        if self.endpoint_registry.get(node_id).await.is_none() {
            return Err(LbError::NodeNotFound(node_id));
        }

        // GPU情報をEndpointRegistryに更新（initializingやready_modelsはLoadManager内部状態で管理）
        let _ = self
            .endpoint_registry
            .update_gpu_info(
                node_id,
                None,                                           // gpu_device_count
                gpu_memory_total_mb.map(|mb| mb * 1024 * 1024), // bytes
                gpu_memory_used_mb.map(|mb| mb * 1024 * 1024),  // bytes
                gpu_capability_score.map(|s| s as f32),
                Some(active_requests),
            )
            .await;

        let mut state = self.state.write().await;
        let entry = state.entry(node_id).or_default();
        let was_active = entry.combined_active() > 0;
        let was_initializing = entry.initializing;

        let derived_average = average_response_time_ms.or_else(|| entry.average_latency_ms());
        let timestamp = Utc::now();
        let metrics = HealthMetrics {
            node_id,
            cpu_usage,
            memory_usage,
            gpu_usage,
            gpu_memory_usage,
            gpu_memory_total_mb,
            gpu_memory_used_mb,
            gpu_temperature,
            gpu_model_name,
            gpu_compute_capability,
            gpu_capability_score,
            active_requests,
            total_requests: entry.total_assigned,
            average_response_time_ms: derived_average,
            timestamp,
        };

        entry.last_metrics = Some(metrics.clone());
        entry.push_metrics(metrics);
        entry.initializing = initializing;
        entry.ready_models = ready_models;
        if !entry.initializing {
            self.ready_notify.notify_waiters();
        }
        if (was_active && entry.combined_active() == 0)
            || (was_initializing && !entry.initializing && entry.combined_active() == 0)
        {
            self.queue_notify.notify_waiters();
        }

        Ok(())
    }

    /// ノード登録時に初期状態を同期
    pub async fn upsert_initial_state(
        &self,
        node_id: Uuid,
        initializing: bool,
        ready_models: Option<(u8, u8)>,
    ) {
        let mut state = self.state.write().await;
        let entry = state.entry(node_id).or_default();
        entry.initializing = initializing;
        entry.ready_models = ready_models;
        if !initializing {
            self.ready_notify.notify_waiters();
            if entry.combined_active() == 0 {
                self.queue_notify.notify_waiters();
            }
        }
    }

    /// 初期化完了しているノードが存在するか
    pub async fn has_ready_nodes(&self) -> bool {
        let state = self.state.read().await;
        state.values().any(|s| !s.initializing)
    }

    /// 全ノードが初期化中かを判定
    pub async fn all_initializing(&self) -> bool {
        let state = self.state.read().await;
        !state.is_empty() && state.values().all(|s| s.initializing)
    }

    /// readyなノードが出るまで待機。待ち人数が上限を超えたらfalse。
    pub async fn wait_for_ready(&self, max_waiters: usize) -> bool {
        let current = self.waiters.fetch_add(1, AtomicOrdering::SeqCst) + 1;
        if current > max_waiters {
            self.waiters.fetch_sub(1, AtomicOrdering::SeqCst);
            return false;
        }
        if self.has_ready_nodes().await {
            self.waiters.fetch_sub(1, AtomicOrdering::SeqCst);
            return true;
        }
        self.ready_notify.notified().await;
        self.waiters.fetch_sub(1, AtomicOrdering::SeqCst);
        true
    }

    /// タイムアウト付きでreadyなノードが出るまで待機
    ///
    /// # 戻り値
    /// - `WaitResult::Ready`: ノードが利用可能になった
    /// - `WaitResult::Timeout`: タイムアウト
    /// - `WaitResult::CapacityExceeded`: 待機キュー容量超過
    pub async fn wait_for_ready_with_timeout(
        &self,
        max_waiters: usize,
        timeout_duration: StdDuration,
    ) -> WaitResult {
        // 待ち人数チェック
        let current = self.waiters.fetch_add(1, AtomicOrdering::SeqCst) + 1;
        if current > max_waiters {
            self.waiters.fetch_sub(1, AtomicOrdering::SeqCst);
            return WaitResult::CapacityExceeded;
        }

        // 既にreadyなノードがあれば即座に返す
        if self.has_ready_nodes().await {
            self.waiters.fetch_sub(1, AtomicOrdering::SeqCst);
            return WaitResult::Ready;
        }

        // タイムアウト付きで待機
        let result = tokio::time::timeout(timeout_duration, self.ready_notify.notified()).await;

        self.waiters.fetch_sub(1, AtomicOrdering::SeqCst);

        match result {
            Ok(_) => WaitResult::Ready,
            Err(_) => WaitResult::Timeout,
        }
    }

    /// リクエストキュー待機数を取得
    pub fn queue_waiters(&self) -> usize {
        self.queue_waiters.load(AtomicOrdering::Relaxed)
    }

    /// アイドルノードが存在するか
    async fn has_idle_nodes(&self) -> bool {
        let endpoints = self.endpoint_registry.list_online().await;
        if endpoints.is_empty() {
            return false;
        }

        let state = self.state.read().await;
        endpoints.iter().any(|endpoint| {
            let load = state.get(&endpoint.id);
            // 初期化中でないかつアイドル状態
            let is_not_initializing = load.map(|l| !l.initializing).unwrap_or(true);
            let is_idle = load.map(|l| l.combined_active() == 0).unwrap_or(true);
            is_not_initializing && is_idle
        })
    }

    /// 指定モデルに対応するアイドルノードが存在するか
    async fn has_idle_nodes_for_model(&self, model_id: &str) -> bool {
        let endpoints = self.endpoint_registry.find_by_model(model_id).await;
        if endpoints.is_empty() {
            return false;
        }

        let state = self.state.read().await;
        endpoints.iter().any(|endpoint| {
            let load = state.get(&endpoint.id);
            // 初期化中でないかつアイドル状態
            let is_not_initializing = load.map(|l| !l.initializing).unwrap_or(true);
            let is_idle = load.map(|l| l.combined_active() == 0).unwrap_or(true);
            is_not_initializing && is_idle
        })
    }

    // SPEC-f8e3a1b7: select_idle_node / select_idle_node_for_model は
    // select_idle_endpoint / select_idle_endpoint_for_model に置き換えられました

    /// タイムアウト付きでアイドルノード待機
    pub async fn wait_for_idle_node_with_timeout(
        &self,
        max_waiters: usize,
        timeout_duration: StdDuration,
    ) -> WaitResult {
        let current = self.queue_waiters.fetch_add(1, AtomicOrdering::SeqCst) + 1;
        if current > max_waiters {
            self.queue_waiters.fetch_sub(1, AtomicOrdering::SeqCst);
            return WaitResult::CapacityExceeded;
        }

        let _guard = QueueWaiterGuard::new(self.queue_waiters.clone());

        if self.has_idle_nodes().await {
            return WaitResult::Ready;
        }

        let result = tokio::time::timeout(timeout_duration, self.queue_notify.notified()).await;

        match result {
            Ok(_) => WaitResult::Ready,
            Err(_) => WaitResult::Timeout,
        }
    }

    /// タイムアウト付きでモデル対応のアイドルノード待機
    pub async fn wait_for_idle_node_with_timeout_for_model(
        &self,
        model_id: &str,
        max_waiters: usize,
        timeout_duration: StdDuration,
    ) -> WaitResult {
        let current = self.queue_waiters.fetch_add(1, AtomicOrdering::SeqCst) + 1;
        if current > max_waiters {
            self.queue_waiters.fetch_sub(1, AtomicOrdering::SeqCst);
            return WaitResult::CapacityExceeded;
        }

        let _guard = QueueWaiterGuard::new(self.queue_waiters.clone());

        if self.has_idle_nodes_for_model(model_id).await {
            return WaitResult::Ready;
        }

        let result = tokio::time::timeout(timeout_duration, self.queue_notify.notified()).await;

        match result {
            Ok(_) => WaitResult::Ready,
            Err(_) => WaitResult::Timeout,
        }
    }

    /// アドミッション制御（段階的バックプレッシャー）
    ///
    /// 待機キューの使用率に応じて、リクエストの受け入れ判断を行う。
    /// - 50%未満: 即座に受け入れ
    /// - 50-80%: 遅延付きで受け入れ（負荷に比例した遅延）
    /// - 80%以上: リジェクト
    pub fn admission_control(&self, max_waiters: usize) -> AdmissionDecision {
        let waiters = self.waiters.load(AtomicOrdering::Relaxed);
        let threshold_accept = max_waiters / 2; // 50%
        let threshold_reject = max_waiters * 4 / 5; // 80%

        if waiters < threshold_accept {
            AdmissionDecision::Accept
        } else if waiters < threshold_reject {
            // 50-80%: 負荷に比例した遅延（10ms〜100ms）
            let load_ratio =
                (waiters - threshold_accept) as f64 / (threshold_reject - threshold_accept) as f64;
            let delay_ms = 10 + (load_ratio * 90.0) as u64;
            AdmissionDecision::AcceptWithDelay(StdDuration::from_millis(delay_ms))
        } else {
            AdmissionDecision::Reject
        }
    }

    /// リクエスト開始を記録
    pub async fn begin_request(&self, node_id: Uuid) -> RouterResult<RequestLease> {
        // エンドポイントが存在することを確認
        if self.endpoint_registry.get(node_id).await.is_none() {
            return Err(LbError::NodeNotFound(node_id));
        }

        let mut state = self.state.write().await;
        let entry = state.entry(node_id).or_default();
        entry.assigned_active = entry.assigned_active.saturating_add(1);
        entry.total_assigned = entry.total_assigned.saturating_add(1);

        Ok(RequestLease::new(self.clone(), node_id))
    }

    /// リクエスト完了を記録
    pub async fn finish_request(
        &self,
        node_id: Uuid,
        outcome: RequestOutcome,
        duration: StdDuration,
    ) -> RouterResult<()> {
        // エンドポイントが存在することを確認
        if self.endpoint_registry.get(node_id).await.is_none() {
            return Err(LbError::NodeNotFound(node_id));
        }

        let mut state = self.state.write().await;
        let entry = state.entry(node_id).or_default();

        if let RequestOutcome::Queued = outcome {
            // キューに積んだだけのものは active を増減させない
        } else {
            if entry.assigned_active > 0 {
                entry.assigned_active -= 1;
            }

            match outcome {
                RequestOutcome::Success => {
                    entry.success_count = entry.success_count.saturating_add(1)
                }
                RequestOutcome::Error => entry.error_count = entry.error_count.saturating_add(1),
                RequestOutcome::Queued => {}
            }

            entry.total_latency_ms = entry.total_latency_ms.saturating_add(duration.as_millis());
        }

        let updated_average = entry.average_latency_ms();

        if let Some(metrics) = entry.last_metrics.as_mut() {
            metrics.total_requests = entry.total_assigned;
            if updated_average.is_some() {
                metrics.average_response_time_ms = updated_average;
            }
            if let Some(latest) = entry.metrics_history.back_mut() {
                latest.total_requests = metrics.total_requests;
                if let Some(avg) = metrics.average_response_time_ms {
                    latest.average_response_time_ms = Some(avg);
                }
                latest.gpu_usage = metrics.gpu_usage;
                latest.gpu_memory_usage = metrics.gpu_memory_usage;
            }
        }

        let should_notify_idle = entry.combined_active() == 0;

        drop(state);
        if should_notify_idle {
            self.queue_notify.notify_waiters();
        }
        self.record_request_history(outcome, Utc::now()).await;

        Ok(())
    }

    /// リクエスト完了を記録（トークン使用量含む）
    pub async fn finish_request_with_tokens(
        &self,
        node_id: Uuid,
        outcome: RequestOutcome,
        duration: StdDuration,
        token_usage: Option<crate::token::TokenUsage>,
    ) -> RouterResult<()> {
        // エンドポイントが存在することを確認
        if self.endpoint_registry.get(node_id).await.is_none() {
            return Err(LbError::NodeNotFound(node_id));
        }

        let mut state = self.state.write().await;
        let entry = state.entry(node_id).or_default();

        if let RequestOutcome::Queued = outcome {
            // キューに積んだだけのものは active を増減させない
        } else {
            if entry.assigned_active > 0 {
                entry.assigned_active -= 1;
            }

            match outcome {
                RequestOutcome::Success => {
                    entry.success_count = entry.success_count.saturating_add(1)
                }
                RequestOutcome::Error => entry.error_count = entry.error_count.saturating_add(1),
                RequestOutcome::Queued => {}
            }

            entry.total_latency_ms = entry.total_latency_ms.saturating_add(duration.as_millis());

            // トークン使用量を累積
            if let Some(ref usage) = token_usage {
                if let Some(input) = usage.input_tokens {
                    entry.total_input_tokens =
                        entry.total_input_tokens.saturating_add(input as u64);
                }
                if let Some(output) = usage.output_tokens {
                    entry.total_output_tokens =
                        entry.total_output_tokens.saturating_add(output as u64);
                }
                // total_tokensはinput + outputで計算するか、明示的に渡されたものを使用
                let total = usage.total_tokens.or_else(|| {
                    match (usage.input_tokens, usage.output_tokens) {
                        (Some(i), Some(o)) => Some(i + o),
                        (Some(i), None) => Some(i),
                        (None, Some(o)) => Some(o),
                        (None, None) => None,
                    }
                });
                if let Some(t) = total {
                    entry.total_tokens = entry.total_tokens.saturating_add(t as u64);
                }
            }
        }

        let updated_average = entry.average_latency_ms();

        if let Some(metrics) = entry.last_metrics.as_mut() {
            metrics.total_requests = entry.total_assigned;
            if updated_average.is_some() {
                metrics.average_response_time_ms = updated_average;
            }
            if let Some(latest) = entry.metrics_history.back_mut() {
                latest.total_requests = metrics.total_requests;
                if let Some(avg) = metrics.average_response_time_ms {
                    latest.average_response_time_ms = Some(avg);
                }
                latest.gpu_usage = metrics.gpu_usage;
                latest.gpu_memory_usage = metrics.gpu_memory_usage;
            }
        }

        let should_notify_idle = entry.combined_active() == 0;

        drop(state);
        if should_notify_idle {
            self.queue_notify.notify_waiters();
        }
        self.record_request_history(outcome, Utc::now()).await;

        Ok(())
    }

    // SPEC-f8e3a1b7: Node依存の選択関数は削除されました
    // - collect_online_nodes → collect_online_endpoints
    // - select_node_from_candidates → select_endpoint_round_robin_from_endpoints
    // - select_endpoint/select_node → select_endpoint_direct
    // - select_endpoint_for_model/select_node_for_model → select_endpoint_direct_for_model

    /// 指定されたエンドポイントのロードスナップショットを取得
    pub async fn snapshot(&self, endpoint_id: Uuid) -> RouterResult<EndpointLoadSnapshot> {
        let endpoint = self
            .endpoint_registry
            .get(endpoint_id)
            .await
            .ok_or(LbError::NodeNotFound(endpoint_id))?;
        let state = self.state.read().await;
        let load_state = state.get(&endpoint_id).cloned().unwrap_or_default();

        Ok(self.build_snapshot_from_endpoint(&endpoint, load_state, Utc::now()))
    }

    /// すべてのエンドポイントのロードスナップショットを取得
    pub async fn snapshots(&self) -> Vec<EndpointLoadSnapshot> {
        let endpoints = self.endpoint_registry.list().await;
        let state = self.state.read().await;

        let now = Utc::now();

        endpoints
            .iter()
            .map(|endpoint| {
                let load_state = state.get(&endpoint.id).cloned().unwrap_or_default();
                self.build_snapshot_from_endpoint(endpoint, load_state, now)
            })
            .collect()
    }

    /// 指定されたエンドポイントのメトリクス履歴を取得
    pub async fn metrics_history(&self, node_id: Uuid) -> RouterResult<Vec<HealthMetrics>> {
        // エンドポイントが存在することを確認
        if self.endpoint_registry.get(node_id).await.is_none() {
            return Err(LbError::NodeNotFound(node_id));
        }
        let state = self.state.read().await;
        let history = state
            .get(&node_id)
            .map(|load_state| load_state.metrics_history.iter().cloned().collect())
            .unwrap_or_else(Vec::new);
        Ok(history)
    }

    /// システム全体の統計サマリーを取得
    /// システム全体の統計サマリーを取得（SPEC-f8e3a1b7: Endpoint版）
    pub async fn summary(&self) -> SystemSummary {
        use crate::types::endpoint::EndpointStatus;

        let endpoints = self.endpoint_registry.list().await;
        let state = self.state.read().await;

        let mut summary = SystemSummary {
            total_nodes: endpoints.len(),
            online_nodes: endpoints
                .iter()
                .filter(|ep| ep.status == EndpointStatus::Online)
                .count(),
            pending_nodes: endpoints
                .iter()
                .filter(|ep| ep.status == EndpointStatus::Pending)
                .count(),
            registering_nodes: 0, // EndpointStatusにはRegisteringがないため常に0
            offline_nodes: endpoints
                .iter()
                .filter(|ep| {
                    ep.status == EndpointStatus::Offline || ep.status == EndpointStatus::Error
                })
                .count(),
            queued_requests: self.queue_waiters.load(AtomicOrdering::Relaxed),
            ..Default::default()
        };

        let mut total_latency_ms = 0u128;
        let mut latency_samples = 0u64;
        let mut weighted_average_sum = 0f64;
        let mut weighted_average_weight = 0f64;
        let mut latest_timestamp: Option<DateTime<Utc>> = None;
        let mut gpu_usage_total = 0f64;
        let mut gpu_usage_samples = 0u64;
        let mut gpu_memory_total = 0f64;
        let mut gpu_memory_samples = 0u64;
        let now = Utc::now();

        for endpoint in &endpoints {
            if let Some(load_state) = state.get(&endpoint.id) {
                let is_fresh = !load_state.is_stale(now);
                if is_fresh {
                    summary.total_active_requests = summary
                        .total_active_requests
                        .saturating_add(load_state.combined_active());
                }
                summary.total_requests = summary
                    .total_requests
                    .saturating_add(load_state.total_assigned);
                summary.successful_requests = summary
                    .successful_requests
                    .saturating_add(load_state.success_count);
                summary.failed_requests = summary
                    .failed_requests
                    .saturating_add(load_state.error_count);

                // トークン統計を集計
                summary.total_input_tokens = summary
                    .total_input_tokens
                    .saturating_add(load_state.total_input_tokens);
                summary.total_output_tokens = summary
                    .total_output_tokens
                    .saturating_add(load_state.total_output_tokens);
                summary.total_tokens = summary.total_tokens.saturating_add(load_state.total_tokens);

                let completed = load_state.success_count + load_state.error_count;
                if completed > 0 {
                    total_latency_ms = total_latency_ms.saturating_add(load_state.total_latency_ms);
                    latency_samples = latency_samples.saturating_add(completed);
                }

                if is_fresh {
                    if let Some(timestamp) = load_state.last_updated() {
                        if latest_timestamp.is_none_or(|current| timestamp > current) {
                            latest_timestamp = Some(timestamp);
                        }
                    }
                    if let Some(avg) = load_state.effective_average_ms() {
                        let weight = load_state.total_assigned.max(1) as f64;
                        weighted_average_sum += avg as f64 * weight;
                        weighted_average_weight += weight;
                    }
                    if let Some(metrics) = load_state.last_metrics.as_ref() {
                        if let Some(gpu) = metrics.gpu_usage {
                            gpu_usage_total += gpu as f64;
                            gpu_usage_samples = gpu_usage_samples.saturating_add(1);
                        }
                        if let Some(gpu_mem) = metrics.gpu_memory_usage {
                            gpu_memory_total += gpu_mem as f64;
                            gpu_memory_samples = gpu_memory_samples.saturating_add(1);
                        }
                    }
                } else if latest_timestamp.is_none() {
                    // フレッシュなメトリクスがない場合でも最も新しい値を保持
                    if let Some(timestamp) = load_state.last_updated() {
                        latest_timestamp = Some(timestamp);
                    }
                }
            }
        }

        if weighted_average_weight > 0.0 {
            summary.average_response_time_ms =
                Some((weighted_average_sum / weighted_average_weight) as f32);
        } else if latency_samples > 0 {
            summary.average_response_time_ms =
                Some((total_latency_ms as f64 / latency_samples as f64) as f32);
        }

        if gpu_usage_samples > 0 {
            summary.average_gpu_usage = Some((gpu_usage_total / gpu_usage_samples as f64) as f32);
        }
        if gpu_memory_samples > 0 {
            summary.average_gpu_memory_usage =
                Some((gpu_memory_total / gpu_memory_samples as f64) as f32);
        }

        summary.last_metrics_updated_at = latest_timestamp;

        summary
    }

    /// リクエスト履歴を取得
    pub async fn request_history(&self) -> Vec<RequestHistoryPoint> {
        let history = self.history.read().await;
        build_history_window(&history)
    }

    /// リクエスト履歴にアウトカムを記録（分単位で集計）
    pub async fn record_request_history(&self, outcome: RequestOutcome, timestamp: DateTime<Utc>) {
        let minute = align_to_minute(timestamp);
        let mut history = self.history.write().await;

        if let Some(last) = history.back_mut() {
            if last.minute == minute {
                increment_history(last, outcome);
            } else {
                history.push_back(new_history_point(minute, outcome));
            }
        } else {
            history.push_back(new_history_point(minute, outcome));
        }

        prune_history(&mut history, minute);
    }

    /// エンドポイントのスナップショットを構築（SPEC-f8e3a1b7: Endpoint版）
    fn build_snapshot_from_endpoint(
        &self,
        endpoint: &crate::types::endpoint::Endpoint,
        load_state: EndpointLoadState,
        now: DateTime<Utc>,
    ) -> EndpointLoadSnapshot {
        let cpu_usage = load_state
            .last_metrics
            .as_ref()
            .map(|metrics| metrics.cpu_usage);
        let memory_usage = load_state
            .last_metrics
            .as_ref()
            .map(|metrics| metrics.memory_usage);
        let gpu_usage = load_state
            .last_metrics
            .as_ref()
            .and_then(|metrics| metrics.gpu_usage);
        let gpu_memory_usage = load_state
            .last_metrics
            .as_ref()
            .and_then(|metrics| metrics.gpu_memory_usage);
        let gpu_memory_total_mb = load_state
            .last_metrics
            .as_ref()
            .and_then(|metrics| metrics.gpu_memory_total_mb);
        let gpu_memory_used_mb = load_state
            .last_metrics
            .as_ref()
            .and_then(|metrics| metrics.gpu_memory_used_mb);
        let gpu_temperature = load_state
            .last_metrics
            .as_ref()
            .and_then(|metrics| metrics.gpu_temperature);
        let gpu_model_name = load_state
            .last_metrics
            .as_ref()
            .and_then(|metrics| metrics.gpu_model_name.clone());
        let gpu_compute_capability = load_state
            .last_metrics
            .as_ref()
            .and_then(|metrics| metrics.gpu_compute_capability.clone());
        let gpu_capability_score = load_state
            .last_metrics
            .as_ref()
            .and_then(|metrics| metrics.gpu_capability_score);
        let active_requests = load_state.combined_active();

        EndpointLoadSnapshot {
            endpoint_id: endpoint.id,
            machine_name: endpoint.name.clone(),
            status: endpoint.status,
            cpu_usage,
            memory_usage,
            gpu_usage,
            gpu_memory_usage,
            gpu_memory_total_mb,
            gpu_memory_used_mb,
            gpu_temperature,
            gpu_model_name,
            gpu_compute_capability,
            gpu_capability_score,
            active_requests,
            total_requests: load_state.total_assigned,
            successful_requests: load_state.success_count,
            failed_requests: load_state.error_count,
            average_response_time_ms: load_state.effective_average_ms(),
            last_updated: load_state.last_updated(),
            is_stale: load_state.is_stale(now),
            total_input_tokens: load_state.total_input_tokens,
            total_output_tokens: load_state.total_output_tokens,
            total_tokens: load_state.total_tokens,
        }
    }

    // ========================================================================
    // ラウンドロビン選択（SPEC-f8e3a1b7: Endpoint版に移行完了）
    // ========================================================================
    // 古いNode版関数は削除されました:
    // - select_endpoint_round_robin → select_endpoint_round_robin_direct
    // - select_endpoint_round_robin_for_model → select_endpoint_round_robin_direct_for_model
    // - select_node_round_robin_from_candidates → select_endpoint_round_robin_from_endpoints

    /// オンラインエンドポイントを収集（Endpoint版）
    async fn collect_online_endpoints(
        &self,
        model_id: Option<&str>,
    ) -> RouterResult<Vec<crate::types::endpoint::Endpoint>> {
        if let Some(model_id) = model_id {
            let endpoints = self.endpoint_registry.find_by_model(model_id).await;
            if endpoints.is_empty() {
                return Err(LbError::NoCapableNodes(model_id.to_string()));
            }
            return Ok(endpoints);
        }

        let endpoints = self.endpoint_registry.list_online().await;
        if endpoints.is_empty() {
            return Err(LbError::NoNodesAvailable);
        }

        Ok(endpoints)
    }

    /// エンドポイントを直接選択（ラウンドロビン - Endpoint版）
    pub async fn select_endpoint_direct(&self) -> RouterResult<crate::types::endpoint::Endpoint> {
        let endpoints = self.collect_online_endpoints(None).await?;
        self.select_endpoint_round_robin_from_endpoints(endpoints)
    }

    /// 指定モデルに対応するエンドポイントを直接選択（ラウンドロビン - Endpoint版）
    pub async fn select_endpoint_direct_for_model(
        &self,
        model_id: &str,
    ) -> RouterResult<crate::types::endpoint::Endpoint> {
        let endpoints = self.collect_online_endpoints(Some(model_id)).await?;
        self.select_endpoint_round_robin_from_endpoints(endpoints)
    }

    /// アイドルエンドポイントを選択（Endpoint版）
    pub async fn select_idle_endpoint(
        &self,
    ) -> RouterResult<Option<crate::types::endpoint::Endpoint>> {
        let endpoints = self.endpoint_registry.list_online().await;
        if endpoints.is_empty() {
            return Err(LbError::NoNodesAvailable);
        }

        let state = self.state.read().await;
        // 初期化中でないエンドポイントをフィルタリング
        let non_initializing: Vec<_> = endpoints
            .iter()
            .filter(|ep| {
                state
                    .get(&ep.id)
                    .map(|load| !load.initializing)
                    .unwrap_or(true)
            })
            .cloned()
            .collect();

        let idle_endpoints: Vec<_> = non_initializing
            .iter()
            .filter(|ep| {
                state
                    .get(&ep.id)
                    .map(|load| load.combined_active() == 0)
                    .unwrap_or(true)
            })
            .cloned()
            .collect();

        if idle_endpoints.is_empty() {
            return Ok(None);
        }

        let round_robin_cursor = self.round_robin.fetch_add(1, AtomicOrdering::SeqCst);
        let round_robin_start = round_robin_cursor % non_initializing.len().max(1);
        let round_robin_priority =
            compute_round_robin_priority_for_endpoints(&non_initializing, round_robin_start);

        let mut ordered = idle_endpoints;
        ordered.sort_by(|a, b| {
            let a_rank = round_robin_priority
                .get(&a.id)
                .copied()
                .unwrap_or(usize::MAX);
            let b_rank = round_robin_priority
                .get(&b.id)
                .copied()
                .unwrap_or(usize::MAX);
            a_rank.cmp(&b_rank)
        });

        Ok(ordered.first().cloned())
    }

    /// モデル対応のアイドルエンドポイントを選択（Endpoint版）
    pub async fn select_idle_endpoint_for_model(
        &self,
        model_id: &str,
    ) -> RouterResult<Option<crate::types::endpoint::Endpoint>> {
        let endpoints = self.collect_online_endpoints(Some(model_id)).await?;
        let state = self.state.read().await;

        // 初期化中でないエンドポイントをフィルタリング
        let non_initializing: Vec<_> = endpoints
            .into_iter()
            .filter(|ep| {
                state
                    .get(&ep.id)
                    .map(|load| !load.initializing)
                    .unwrap_or(true)
            })
            .collect();

        let idle_endpoints: Vec<_> = non_initializing
            .iter()
            .filter(|ep| {
                state
                    .get(&ep.id)
                    .map(|load| load.combined_active() == 0)
                    .unwrap_or(true)
            })
            .cloned()
            .collect();

        if idle_endpoints.is_empty() {
            return Ok(None);
        }

        let round_robin_cursor = self.round_robin.fetch_add(1, AtomicOrdering::SeqCst);
        let round_robin_start = round_robin_cursor % non_initializing.len().max(1);
        let round_robin_priority =
            compute_round_robin_priority_for_endpoints(&non_initializing, round_robin_start);

        let mut ordered = idle_endpoints;
        ordered.sort_by(|a, b| {
            let a_rank = round_robin_priority
                .get(&a.id)
                .copied()
                .unwrap_or(usize::MAX);
            let b_rank = round_robin_priority
                .get(&b.id)
                .copied()
                .unwrap_or(usize::MAX);
            a_rank.cmp(&b_rank)
        });

        Ok(ordered.first().cloned())
    }

    /// 純粋なラウンドロビンでエンドポイントを選択（Endpoint版）
    pub async fn select_endpoint_round_robin_direct(
        &self,
    ) -> RouterResult<crate::types::endpoint::Endpoint> {
        let endpoints = self.collect_online_endpoints(None).await?;
        self.select_endpoint_round_robin_from_endpoints(endpoints)
    }

    /// 指定モデルに対応するエンドポイントを純粋なラウンドロビンで選択（Endpoint版）
    pub async fn select_endpoint_round_robin_direct_for_model(
        &self,
        model_id: &str,
    ) -> RouterResult<crate::types::endpoint::Endpoint> {
        let endpoints = self.collect_online_endpoints(Some(model_id)).await?;
        self.select_endpoint_round_robin_from_endpoints(endpoints)
    }

    /// 指定モデルに対応する初期化完了エンドポイントをラウンドロビンで選択（Endpoint版）
    ///
    /// 初期化中ノードを除外し、未準備ノードへの転送を避ける。
    pub async fn select_endpoint_round_robin_ready_for_model(
        &self,
        model_id: &str,
    ) -> RouterResult<crate::types::endpoint::Endpoint> {
        let endpoints = self.collect_online_endpoints(Some(model_id)).await?;
        let ready_endpoints: Vec<_> = {
            let state = self.state.read().await;
            endpoints
                .into_iter()
                .filter(|ep| {
                    state
                        .get(&ep.id)
                        .map(|load| !load.initializing)
                        .unwrap_or(true)
                })
                .collect()
        };

        self.select_endpoint_round_robin_from_endpoints(ready_endpoints)
    }

    /// 候補エンドポイントから純粋なラウンドロビンで選択（Endpoint版）
    fn select_endpoint_round_robin_from_endpoints(
        &self,
        endpoints: Vec<crate::types::endpoint::Endpoint>,
    ) -> RouterResult<crate::types::endpoint::Endpoint> {
        if endpoints.is_empty() {
            return Err(LbError::NoNodesAvailable);
        }

        let cursor = self.round_robin.fetch_add(1, AtomicOrdering::SeqCst);
        let index = cursor % endpoints.len();

        Ok(endpoints[index].clone())
    }
}

fn align_to_minute(ts: DateTime<Utc>) -> DateTime<Utc> {
    ts.with_second(0).unwrap().with_nanosecond(0).unwrap()
}

fn prune_history(history: &mut VecDeque<RequestHistoryPoint>, newest: DateTime<Utc>) {
    let cutoff = newest - ChronoDuration::minutes(REQUEST_HISTORY_WINDOW_MINUTES - 1);
    while let Some(front) = history.front() {
        if front.minute < cutoff {
            history.pop_front();
        } else {
            break;
        }
    }
}

fn new_history_point(minute: DateTime<Utc>, outcome: RequestOutcome) -> RequestHistoryPoint {
    let mut point = RequestHistoryPoint {
        minute,
        success: 0,
        error: 0,
    };
    increment_history(&mut point, outcome);
    point
}

fn increment_history(point: &mut RequestHistoryPoint, outcome: RequestOutcome) {
    match outcome {
        RequestOutcome::Success => point.success = point.success.saturating_add(1),
        RequestOutcome::Error => point.error = point.error.saturating_add(1),
        RequestOutcome::Queued => {} // キューは履歴ではカウントしない
    }
}

/// ラウンドロビン優先度計算（SPEC-f8e3a1b7: Endpoint版）
fn compute_round_robin_priority_for_endpoints(
    endpoints: &[crate::types::endpoint::Endpoint],
    start_index: usize,
) -> HashMap<Uuid, usize> {
    let len = endpoints.len();
    let mut priority = HashMap::with_capacity(len);
    if len == 0 {
        return priority;
    }

    for offset in 0..len {
        let idx = (start_index + offset) % len;
        priority.insert(endpoints[idx].id, offset);
    }

    priority
}

// SPEC-f8e3a1b7: メトリクスベース比較関数は削除されました
// - usage_snapshot
// - compare_usage_levels
// 新しい負荷分散はレイテンシベース（EMA α=0.2）を使用

fn build_history_window(history: &VecDeque<RequestHistoryPoint>) -> Vec<RequestHistoryPoint> {
    let now = align_to_minute(Utc::now());
    let mut map: HashMap<DateTime<Utc>, RequestHistoryPoint> = history
        .iter()
        .cloned()
        .map(|point| (point.minute, point))
        .collect();
    fill_history(now, &mut map)
}

fn fill_history(
    now: DateTime<Utc>,
    map: &mut HashMap<DateTime<Utc>, RequestHistoryPoint>,
) -> Vec<RequestHistoryPoint> {
    let start = now - ChronoDuration::minutes(REQUEST_HISTORY_WINDOW_MINUTES - 1);
    let mut cursor = start;
    let mut result = Vec::with_capacity(REQUEST_HISTORY_WINDOW_MINUTES as usize);

    while cursor <= now {
        if let Some(point) = map.remove(&cursor) {
            result.push(point);
        } else {
            result.push(RequestHistoryPoint {
                minute: cursor,
                success: 0,
                error: 0,
            });
        }
        cursor += ChronoDuration::minutes(1);
    }

    result
}

/// リクエスト履歴ポイント
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
pub struct RequestHistoryPoint {
    /// 分単位のタイムスタンプ
    pub minute: DateTime<Utc>,
    /// 成功数
    pub success: u64,
    /// 失敗数
    pub error: u64,
}
