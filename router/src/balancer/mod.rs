//! ロードバランサーモジュール
//!
//! ノードに関する最新メトリクスとリクエスト統計を集約し、
//! 高度なロードバランシング戦略を提供する。
//!
//! # EndpointRegistry統合
//!
//! このモジュールはEndpointRegistryを使用してエンドポイント情報を管理します。
//! 内部的に一部レガシーNode型を使用している箇所がありますが、
//! EndpointRegistryから取得したEndpointを`to_legacy_node()`で変換しています。

#![allow(deprecated)] // Using deprecated Node type during migration

use crate::registry::endpoints::EndpointRegistry;
use chrono::{DateTime, Duration as ChronoDuration, Timelike, Utc};
use llm_router_common::{
    error::{RouterError, RouterResult},
    types::{HealthMetrics, Node, NodeStatus},
};
use serde::Serialize;
use std::{
    cmp::Ordering,
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
/// メトリクススコア比較時の許容誤差
const LOAD_SCORE_EPSILON: f64 = 0.0001;

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

fn compare_option_f32(a: Option<f32>, b: Option<f32>) -> Ordering {
    match (a, b) {
        (Some(ax), Some(bx)) => ax.partial_cmp(&bx).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn compare_average_ms(a: Option<f32>, b: Option<f32>) -> Ordering {
    compare_option_f32(a, b)
}

fn node_spec_score(node: &Node, load_state: Option<&EndpointLoadState>) -> u32 {
    node.gpu_capability_score
        .or_else(|| {
            load_state.and_then(|state| {
                state
                    .last_metrics
                    .as_ref()
                    .and_then(|metrics| metrics.gpu_capability_score)
            })
        })
        .unwrap_or(0)
}

fn compare_spec_levels(
    a_node: &Node,
    a_load: &EndpointLoadState,
    b_node: &Node,
    b_load: &EndpointLoadState,
) -> Ordering {
    let a_score = node_spec_score(a_node, Some(a_load));
    let b_score = node_spec_score(b_node, Some(b_load));
    b_score.cmp(&a_score)
}

fn compare_spec_by_state(
    a_node: &Node,
    b_node: &Node,
    state: &HashMap<Uuid, EndpointLoadState>,
) -> Ordering {
    let a_score = node_spec_score(a_node, state.get(&a_node.id));
    let b_score = node_spec_score(b_node, state.get(&b_node.id));
    b_score.cmp(&a_score)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration as StdDuration;

    // NOTE: SPEC-66555000によりNodeRegistryは廃止されました。
    // NodeRegistryを使用するテストは#[ignore]でマークされています。
    // 今後EndpointRegistryベースに移行予定です。

    #[test]
    fn compare_average_ms_orders_values() {
        assert_eq!(compare_average_ms(Some(120.0), Some(180.0)), Ordering::Less);
        assert_eq!(
            compare_average_ms(Some(220.0), Some(180.0)),
            Ordering::Greater
        );
        assert_eq!(compare_average_ms(Some(100.0), None), Ordering::Less);
        assert_eq!(compare_average_ms(None, Some(90.0)), Ordering::Greater);
        assert_eq!(compare_average_ms(None, None), Ordering::Equal);
    }

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

    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn load_manager_prefers_lower_latency_when_active_equal() {
        // TODO: EndpointRegistryベースに移行
    }

    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn metrics_history_tracks_recent_points() {
        // TODO: EndpointRegistryベースに移行
    }

    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn select_node_by_metrics_prefers_lower_load() {
        // TODO: EndpointRegistryベースに移行
    }

    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn select_node_prefers_lower_usage_even_with_same_activity() {
        // TODO: EndpointRegistryベースに移行
    }

    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn select_node_prefers_lower_usage_when_all_high_cpu() {
        // TODO: EndpointRegistryベースに移行
    }

    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn select_node_handles_partial_metrics_with_spec_priority() {
        // TODO: EndpointRegistryベースに移行
    }

    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn select_node_prefers_higher_spec_until_it_becomes_busy() {
        // TODO: EndpointRegistryベースに移行
    }

    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn select_node_by_metrics_deprioritizes_nodes_without_metrics() {
        // TODO: EndpointRegistryベースに移行
    }

    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn select_node_by_metrics_considers_gpu_usage() {
        // TODO: EndpointRegistryベースに移行
    }

    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn select_node_by_metrics_handles_partial_metrics_with_spec_priority() {
        // TODO: EndpointRegistryベースに移行
    }

    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn select_node_by_metrics_prefers_higher_spec_until_busy() {
        // TODO: EndpointRegistryベースに移行
    }

    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn wait_for_ready_unblocks_when_node_becomes_ready() {
        // TODO: EndpointRegistryベースに移行
    }

    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn wait_for_ready_limits_waiters_and_notifies_first() {
        // TODO: EndpointRegistryベースに移行
    }

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

    // T005: wait_for_ready_with_timeout - タイムアウトテスト
    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn wait_for_ready_with_timeout_returns_timeout_when_no_ready_nodes() {
        // TODO: EndpointRegistryベースに移行
    }

    // T006: wait_for_ready_with_timeout - 容量超過テスト
    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn wait_for_ready_with_timeout_returns_capacity_exceeded() {
        // TODO: EndpointRegistryベースに移行
    }

    // T007: wait_for_ready_with_timeout - Ready成功テスト
    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn wait_for_ready_with_timeout_returns_ready_when_node_becomes_available() {
        // TODO: EndpointRegistryベースに移行
    }

    // T009: admission_control - 負荷50%未満ならAccept
    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn admission_control_returns_accept_when_below_50_percent() {
        // TODO: EndpointRegistryベースに移行
    }

    // T010: admission_control - 負荷50-80%ならAcceptWithDelay
    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn admission_control_returns_accept_with_delay_when_between_50_and_80_percent() {
        // TODO: EndpointRegistryベースに移行
    }

    // T011: admission_control - 負荷80%以上ならReject
    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn admission_control_returns_reject_when_above_80_percent() {
        // TODO: EndpointRegistryベースに移行
    }

    // T012: admission_control - 境界値テスト
    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn admission_control_boundary_values() {
        // TODO: EndpointRegistryベースに移行
    }

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
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn test_finish_request_accumulates_tokens() {
        // TODO: EndpointRegistryベースに移行
    }

    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn test_finish_request_accumulates_multiple_tokens() {
        // TODO: EndpointRegistryベースに移行
    }

    // T-13: エラー応答時のトークンカウントテスト
    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn test_finish_request_accumulates_tokens_on_error() {
        // TODO: EndpointRegistryベースに移行
    }

    // T-14: オフラインノードの統計保持テスト
    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn test_offline_node_retains_token_statistics() {
        // TODO: EndpointRegistryベースに移行
    }

    /// T007: Pending状態のノードがルーティングから除外されることを検証
    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn test_pending_node_excluded_from_routing() {
        // TODO: EndpointRegistryベースに移行
    }

    /// T008: Registering状態のノードがルーティングから除外されることを検証
    #[tokio::test]
    #[ignore = "SPEC-66555000: NodeRegistry is deprecated, migrate to EndpointRegistry"]
    async fn test_registering_node_excluded_from_routing() {
        // TODO: EndpointRegistryベースに移行
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

/// NodeLoadState は EndpointLoadState の後方互換エイリアス
#[deprecated(note = "Use EndpointLoadState instead")]
#[allow(dead_code)]
type NodeLoadState = EndpointLoadState;

impl EndpointLoadState {
    fn combined_active(&self) -> u32 {
        let heartbeat_active = self
            .last_metrics
            .as_ref()
            .map(|m| m.active_requests)
            .unwrap_or(0);
        // Avoid double counting when node heartbeat mirrors router-assigned requests.
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
/// NodeRegistry廃止移行中。内部的には`endpoint_id`を使用するが、
/// API互換性のため`node_id`としてシリアライズする。
#[derive(Debug, Clone, Serialize)]
pub struct EndpointLoadSnapshot {
    /// エンドポイントID（API互換性のためnode_idとしてシリアライズ）
    #[serde(rename = "node_id")]
    pub endpoint_id: Uuid,
    /// マシン名
    pub machine_name: String,
    /// ノード状態
    pub status: NodeStatus,
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
    /// 処理中リクエスト数（Router観点+ノード自己申告）
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
            return Err(RouterError::NodeNotFound(node_id));
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

    /// アイドルノードを選択（なければ None）
    pub async fn select_idle_node(&self) -> RouterResult<Option<Node>> {
        let endpoints = self.endpoint_registry.list_online().await;
        // Endpointから従来のNode型へ変換
        let nodes: Vec<Node> = endpoints
            .into_iter()
            .map(|e| e.to_legacy_node(vec![]))
            .collect();

        if nodes.is_empty() {
            return Err(RouterError::NoNodesAvailable);
        }

        let state = self.state.read().await;
        // 初期化中でないノードをフィルタリング
        let non_initializing_nodes: Vec<_> = nodes
            .iter()
            .filter(|node| {
                state
                    .get(&node.id)
                    .map(|load| !load.initializing)
                    .unwrap_or(true)
            })
            .cloned()
            .collect();

        let idle_nodes: Vec<_> = non_initializing_nodes
            .iter()
            .filter(|node| {
                state
                    .get(&node.id)
                    .map(|load| load.combined_active() == 0)
                    .unwrap_or(true)
            })
            .cloned()
            .collect();

        if idle_nodes.is_empty() {
            return Ok(None);
        }

        let round_robin_cursor = self.round_robin.fetch_add(1, AtomicOrdering::SeqCst);
        let round_robin_start = round_robin_cursor % non_initializing_nodes.len().max(1);
        let round_robin_priority =
            compute_round_robin_priority(&non_initializing_nodes, round_robin_start);

        let mut ordered = idle_nodes;
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

    /// モデル対応のアイドルノードを選択（なければ None）
    pub async fn select_idle_node_for_model(&self, model_id: &str) -> RouterResult<Option<Node>> {
        let online_nodes = self.collect_online_nodes(Some(model_id)).await?;
        let online_nodes: Vec<_> = online_nodes
            .into_iter()
            .filter(|node| !node.initializing)
            .collect();

        let state = self.state.read().await;
        let idle_nodes: Vec<_> = online_nodes
            .iter()
            .filter(|node| {
                state
                    .get(&node.id)
                    .map(|load| load.combined_active() == 0)
                    .unwrap_or(true)
            })
            .cloned()
            .collect();

        if idle_nodes.is_empty() {
            return Ok(None);
        }

        let round_robin_cursor = self.round_robin.fetch_add(1, AtomicOrdering::SeqCst);
        let round_robin_start = round_robin_cursor % online_nodes.len();
        let round_robin_priority = compute_round_robin_priority(&online_nodes, round_robin_start);

        let mut ordered = idle_nodes;
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
    pub async fn begin_request(&self, node_id: Uuid) -> RouterResult<()> {
        // エンドポイントが存在することを確認
        if self.endpoint_registry.get(node_id).await.is_none() {
            return Err(RouterError::NodeNotFound(node_id));
        }

        let mut state = self.state.write().await;
        let entry = state.entry(node_id).or_default();
        entry.assigned_active = entry.assigned_active.saturating_add(1);
        entry.total_assigned = entry.total_assigned.saturating_add(1);

        Ok(())
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
            return Err(RouterError::NodeNotFound(node_id));
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
            return Err(RouterError::NodeNotFound(node_id));
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

    /// オンラインノードを収集（EndpointRegistryから）
    #[allow(deprecated)] // to_legacy_node is deprecated but needed for internal use
    async fn collect_online_nodes(&self, model_id: Option<&str>) -> RouterResult<Vec<Node>> {
        if let Some(model_id) = model_id {
            let endpoints = self.endpoint_registry.find_by_model(model_id).await;
            if endpoints.is_empty() {
                return Err(RouterError::NoCapableNodes(model_id.to_string()));
            }
            // EndpointからNodeへ変換
            let nodes: Vec<Node> = endpoints
                .into_iter()
                .map(|e| e.to_legacy_node(vec![model_id.to_string()]))
                .collect();
            return Ok(nodes);
        }

        let endpoints = self.endpoint_registry.list_online().await;
        if endpoints.is_empty() {
            return Err(RouterError::NoNodesAvailable);
        }

        // EndpointからNodeへ変換（モデル情報は空で初期化）
        let nodes: Vec<Node> = endpoints
            .into_iter()
            .map(|e| e.to_legacy_node(vec![]))
            .collect();

        Ok(nodes)
    }

    async fn select_node_from_candidates(&self, online_nodes: Vec<Node>) -> RouterResult<Node> {
        if online_nodes.is_empty() {
            return Err(RouterError::NoNodesAvailable);
        }

        let round_robin_cursor = self.round_robin.fetch_add(1, AtomicOrdering::SeqCst);
        let round_robin_start = round_robin_cursor % online_nodes.len();
        let round_robin_priority = compute_round_robin_priority(&online_nodes, round_robin_start);

        let state = self.state.read().await;
        let now = Utc::now();

        let mut fresh_states: Vec<(Node, EndpointLoadState)> = Vec::new();
        for node in &online_nodes {
            match state.get(&node.id) {
                Some(load_state) if !load_state.is_stale(now) => {
                    fresh_states.push((node.clone(), load_state.clone()));
                }
                _ => {}
            }
        }

        let have_full_fresh_metrics = fresh_states.len() == online_nodes.len();

        if have_full_fresh_metrics && !fresh_states.is_empty() {
            let mut load_based_candidates: Vec<(Node, EndpointLoadState)> = fresh_states
                .iter()
                .filter_map(|(node, load_state)| {
                    if let Some(metrics) = &load_state.last_metrics {
                        if metrics.cpu_usage <= 80.0 {
                            return Some((node.clone(), load_state.clone()));
                        }
                    }
                    None
                })
                .collect();

            if !load_based_candidates.is_empty() {
                load_based_candidates.sort_by(|a, b| {
                    let a_active = a.1.combined_active();
                    let b_active = b.1.combined_active();
                    let a_avg = a.1.effective_average_ms();
                    let b_avg = b.1.effective_average_ms();
                    a_active
                        .cmp(&b_active)
                        .then_with(|| compare_usage_levels(&a.1, &b.1))
                        .then_with(|| compare_spec_levels(&a.0, &a.1, &b.0, &b.1))
                        .then_with(|| compare_average_ms(a_avg, b_avg))
                        .then_with(|| a.1.total_assigned.cmp(&b.1.total_assigned))
                        .then_with(|| {
                            let a_rank = round_robin_priority
                                .get(&a.0.id)
                                .copied()
                                .unwrap_or(usize::MAX);
                            let b_rank = round_robin_priority
                                .get(&b.0.id)
                                .copied()
                                .unwrap_or(usize::MAX);
                            a_rank.cmp(&b_rank)
                        })
                });

                return Ok(load_based_candidates[0].0.clone());
            }

            let mut usage_candidates = fresh_states.clone();
            usage_candidates.sort_by(|a, b| {
                compare_usage_levels(&a.1, &b.1)
                    .then_with(|| compare_spec_levels(&a.0, &a.1, &b.0, &b.1))
                    .then_with(|| {
                        let a_rank = round_robin_priority
                            .get(&a.0.id)
                            .copied()
                            .unwrap_or(usize::MAX);
                        let b_rank = round_robin_priority
                            .get(&b.0.id)
                            .copied()
                            .unwrap_or(usize::MAX);
                        a_rank.cmp(&b_rank)
                    })
            });

            return Ok(usage_candidates[0].0.clone());
        }

        // メトリクスが不足している場合は「ビジー度 → GPUスペック → ラウンドロビン」で決定
        let mut spec_sorted = online_nodes.clone();
        spec_sorted.sort_by(|a, b| {
            let a_active = state
                .get(&a.id)
                .map(|load| load.combined_active())
                .unwrap_or(0);
            let b_active = state
                .get(&b.id)
                .map(|load| load.combined_active())
                .unwrap_or(0);
            a_active
                .cmp(&b_active)
                .then_with(|| compare_spec_by_state(a, b, &state))
                .then_with(|| {
                    let a_rank = round_robin_priority
                        .get(&a.id)
                        .copied()
                        .unwrap_or(usize::MAX);
                    let b_rank = round_robin_priority
                        .get(&b.id)
                        .copied()
                        .unwrap_or(usize::MAX);
                    a_rank.cmp(&b_rank)
                })
        });

        Ok(spec_sorted[0].clone())
    }

    /// 適切なエンドポイントを選択
    pub async fn select_endpoint(&self) -> RouterResult<Node> {
        let online_nodes = self.collect_online_nodes(None).await?;
        self.select_node_from_candidates(online_nodes).await
    }

    /// select_endpoint のエイリアス（後方互換）
    #[deprecated(note = "Use select_endpoint instead")]
    pub async fn select_node(&self) -> RouterResult<Node> {
        self.select_endpoint().await
    }

    /// 指定モデルに対応するエンドポイントを選択
    pub async fn select_endpoint_for_model(&self, model_id: &str) -> RouterResult<Node> {
        let online_nodes = self.collect_online_nodes(Some(model_id)).await?;
        self.select_node_from_candidates(online_nodes).await
    }

    /// select_endpoint_for_model のエイリアス（後方互換）
    #[deprecated(note = "Use select_endpoint_for_model instead")]
    pub async fn select_node_for_model(&self, model_id: &str) -> RouterResult<Node> {
        self.select_endpoint_for_model(model_id).await
    }

    /// 指定されたエンドポイントのロードスナップショットを取得
    #[allow(deprecated)] // to_legacy_node is deprecated but needed for internal use
    pub async fn snapshot(&self, node_id: Uuid) -> RouterResult<EndpointLoadSnapshot> {
        let endpoint = self
            .endpoint_registry
            .get(node_id)
            .await
            .ok_or(RouterError::NodeNotFound(node_id))?;
        let node = endpoint.to_legacy_node(vec![]);
        let state = self.state.read().await;
        let load_state = state.get(&node_id).cloned().unwrap_or_default();

        Ok(self.build_snapshot(node, load_state, Utc::now()))
    }

    /// すべてのエンドポイントのロードスナップショットを取得
    #[allow(deprecated)] // to_legacy_node is deprecated but needed for internal use
    pub async fn snapshots(&self) -> Vec<EndpointLoadSnapshot> {
        let endpoints = self.endpoint_registry.list().await;
        let state = self.state.read().await;

        let now = Utc::now();

        endpoints
            .into_iter()
            .map(|endpoint| {
                let node = endpoint.to_legacy_node(vec![]);
                let load_state = state.get(&node.id).cloned().unwrap_or_default();
                self.build_snapshot(node, load_state, now)
            })
            .collect()
    }

    /// 指定されたエンドポイントのメトリクス履歴を取得
    pub async fn metrics_history(&self, node_id: Uuid) -> RouterResult<Vec<HealthMetrics>> {
        // エンドポイントが存在することを確認
        if self.endpoint_registry.get(node_id).await.is_none() {
            return Err(RouterError::NodeNotFound(node_id));
        }
        let state = self.state.read().await;
        let history = state
            .get(&node_id)
            .map(|load_state| load_state.metrics_history.iter().cloned().collect())
            .unwrap_or_else(Vec::new);
        Ok(history)
    }

    /// システム全体の統計サマリーを取得
    #[allow(deprecated)] // to_legacy_node is deprecated but needed for internal use
    pub async fn summary(&self) -> SystemSummary {
        let endpoints = self.endpoint_registry.list().await;
        // EndpointからNodeへ変換してステータス集計
        let nodes: Vec<Node> = endpoints
            .into_iter()
            .map(|e| e.to_legacy_node(vec![]))
            .collect();
        let state = self.state.read().await;

        let mut summary = SystemSummary {
            total_nodes: nodes.len(),
            online_nodes: nodes
                .iter()
                .filter(|node| node.status == NodeStatus::Online)
                .count(),
            pending_nodes: nodes
                .iter()
                .filter(|node| node.status == NodeStatus::Pending)
                .count(),
            registering_nodes: nodes
                .iter()
                .filter(|node| node.status == NodeStatus::Registering)
                .count(),
            offline_nodes: nodes
                .iter()
                .filter(|node| node.status == NodeStatus::Offline)
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

        for node in &nodes {
            if let Some(load_state) = state.get(&node.id) {
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

    fn build_snapshot(
        &self,
        node: Node,
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
            endpoint_id: node.id,
            machine_name: node.machine_name,
            status: node.status,
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

    /// メトリクスベースのノード選択
    ///
    /// ノードの最新メトリクス（CPU使用率、メモリ使用率、アクティブリクエスト数）を基に
    /// 負荷スコアを計算し、最も低いスコアのノードを選択します。
    ///
    /// # 負荷スコア計算式
    ///
    /// ```text
    /// score = cpu_usage + memory_usage + gpu_usage + gpu_memory_usage + (active_requests × 10)
    /// ```
    ///
    /// - `cpu_usage`: CPU使用率（0.0～100.0）
    /// - `memory_usage`: メモリ使用率（0.0～100.0）
    /// - `gpu_usage`: GPU使用率（0.0～100.0、未報告時は0.0として扱う）
    /// - `gpu_memory_usage`: GPUメモリ使用率（0.0～100.0、未報告時は0.0として扱う）
    /// - スコアが同じ場合はGPU能力スコアの高いノードを優先
    /// - `active_requests`: アクティブリクエスト数（重み付け：×10）
    ///
    /// # フォールバック戦略
    ///
    /// 以下のいずれかの条件に該当する場合、ラウンドロビン選択にフォールバックします：
    ///
    /// - すべてのノードのCPU使用率が80%を超えている
    /// - メトリクスを持つノードが存在しない
    /// - いずれかのノードが鮮度のあるメトリクスを報告していない
    /// - すべてのメトリクスが古い（120秒以上前）
    ///
    /// # 戻り値
    ///
    /// - `Ok(Node)`: 選択されたノード
    /// - `Err(RouterError::NoNodesAvailable)`: オンラインノードが存在しない
    ///
    /// # 例
    ///
    /// ```ignore
    /// let manager = LoadManager::new(registry);
    /// let node = manager.select_node_by_metrics().await?;
    /// println!("Selected node: {}", node.machine_name);
    /// ```
    async fn select_node_by_metrics_from_candidates(
        &self,
        online_nodes: Vec<Node>,
    ) -> RouterResult<Node> {
        if online_nodes.is_empty() {
            return Err(RouterError::NoNodesAvailable);
        }

        let round_robin_cursor = self.round_robin.fetch_add(1, AtomicOrdering::SeqCst);
        let round_robin_start = round_robin_cursor % online_nodes.len();
        let round_robin_priority = compute_round_robin_priority(&online_nodes, round_robin_start);

        let state = self.state.read().await;
        let now = Utc::now();

        // メトリクスを持つノードの負荷スコアを計算
        let mut candidates: Vec<(Node, f64)> = Vec::new();

        for node in &online_nodes {
            if let Some(load_state) = state.get(&node.id) {
                if let Some(metrics) = &load_state.last_metrics {
                    if !load_state.is_stale(now) {
                        // 負荷スコア = cpu_usage + memory_usage + gpu_usage + gpu_memory_usage + (active_requests * 10)
                        let gpu_usage = metrics.gpu_usage.unwrap_or(0.0) as f64;
                        let gpu_memory_usage = metrics.gpu_memory_usage.unwrap_or(0.0) as f64;
                        let score = metrics.cpu_usage as f64
                            + metrics.memory_usage as f64
                            + gpu_usage
                            + gpu_memory_usage
                            + (load_state.combined_active() as f64 * 10.0);
                        candidates.push((node.clone(), score));
                    }
                }
            }
        }

        // すべてのノードがCPU > 80%かチェック
        let all_high_load = !candidates.is_empty()
            && candidates.iter().all(|(node, _)| {
                if let Some(load_state) = state.get(&node.id) {
                    if let Some(metrics) = &load_state.last_metrics {
                        return metrics.cpu_usage > 80.0;
                    }
                }
                false
            });

        if all_high_load || candidates.is_empty() {
            // フォールバック: ラウンドロビン
            return Ok(online_nodes[round_robin_start].clone());
        }

        // 最小スコアに属するノードを抽出し、ラウンドロビン順序で決定する
        let min_score = candidates
            .iter()
            .fold(f64::INFINITY, |acc, (_, score)| acc.min(*score));

        let mut best_nodes: Vec<Node> = candidates
            .iter()
            .filter(|(_, score)| (*score - min_score).abs() <= LOAD_SCORE_EPSILON)
            .map(|(node, _)| node.clone())
            .collect();

        if best_nodes.is_empty() {
            // 理論上起こらないが、安全のためフォールバック
            return Ok(online_nodes[round_robin_start].clone());
        }

        if best_nodes.len() == 1 {
            return Ok(best_nodes.pop().unwrap());
        }

        best_nodes.sort_by(|a, b| {
            compare_spec_by_state(a, b, &state).then_with(|| {
                let a_rank = round_robin_priority
                    .get(&a.id)
                    .copied()
                    .unwrap_or(usize::MAX);
                let b_rank = round_robin_priority
                    .get(&b.id)
                    .copied()
                    .unwrap_or(usize::MAX);
                a_rank.cmp(&b_rank)
            })
        });

        Ok(best_nodes[0].clone())
    }

    /// メトリクスベースでエンドポイントを選択（モデル指定なし）
    pub async fn select_endpoint_by_metrics(&self) -> RouterResult<Node> {
        let online_nodes = self.collect_online_nodes(None).await?;
        self.select_node_by_metrics_from_candidates(online_nodes)
            .await
    }

    /// select_endpoint_by_metrics のエイリアス（後方互換）
    #[deprecated(note = "Use select_endpoint_by_metrics instead")]
    pub async fn select_node_by_metrics(&self) -> RouterResult<Node> {
        self.select_endpoint_by_metrics().await
    }

    /// 指定モデルに対応するエンドポイントをメトリクスベースで選択
    pub async fn select_endpoint_by_metrics_for_model(&self, model_id: &str) -> RouterResult<Node> {
        let online_nodes = self.collect_online_nodes(Some(model_id)).await?;
        self.select_node_by_metrics_from_candidates(online_nodes)
            .await
    }

    /// select_endpoint_by_metrics_for_model のエイリアス（後方互換）
    #[deprecated(note = "Use select_endpoint_by_metrics_for_model instead")]
    pub async fn select_node_by_metrics_for_model(&self, model_id: &str) -> RouterResult<Node> {
        self.select_endpoint_by_metrics_for_model(model_id).await
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

fn compute_round_robin_priority(nodes: &[Node], start_index: usize) -> HashMap<Uuid, usize> {
    let len = nodes.len();
    let mut priority = HashMap::with_capacity(len);
    if len == 0 {
        return priority;
    }

    for offset in 0..len {
        let idx = (start_index + offset) % len;
        priority.insert(nodes[idx].id, offset);
    }

    priority
}

fn usage_snapshot(
    load_state: &EndpointLoadState,
) -> (Option<f32>, Option<f32>, Option<f32>, Option<f32>) {
    load_state
        .last_metrics
        .as_ref()
        .map(|metrics| {
            (
                Some(metrics.cpu_usage),
                Some(metrics.memory_usage),
                metrics.gpu_usage,
                metrics.gpu_memory_usage,
            )
        })
        .unwrap_or((None, None, None, None))
}

fn compare_usage_levels(a: &EndpointLoadState, b: &EndpointLoadState) -> Ordering {
    let (a_cpu, a_mem, a_gpu, a_gpu_mem) = usage_snapshot(a);
    let (b_cpu, b_mem, b_gpu, b_gpu_mem) = usage_snapshot(b);

    compare_option_f32(a_cpu, b_cpu)
        .then_with(|| compare_option_f32(a_mem, b_mem))
        .then_with(|| compare_option_f32(a_gpu, b_gpu))
        .then_with(|| compare_option_f32(a_gpu_mem, b_gpu_mem))
}

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
