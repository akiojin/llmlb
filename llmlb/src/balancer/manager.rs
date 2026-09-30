//! ロードマネージャー本体（TPS 優先の負荷分散と統計集約）

use super::lease::RequestLease;
use super::types::{
    EndpointLoadSnapshot, EndpointLoadState, EndpointTpsSummary, MetricsUpdate, ModelTpsInfo,
    QueueWaiterGuard, RequestHistoryPoint, RequestOutcome, SystemSummary, TpsTrackerMap,
    WaitResult, REQUEST_HISTORY_WINDOW_MINUTES,
};

use crate::common::error::{LbError, RouterResult};
use crate::common::protocol::{TpsApiKind, TpsSource};
use crate::registry::endpoints::EndpointRegistry;
use crate::types::HealthMetrics;
use chrono::{DateTime, Duration as ChronoDuration, Timelike, Utc};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering as AtomicOrdering},
        Arc,
    },
    time::Duration as StdDuration,
};
use tokio::sync::{Notify, RwLock};
use uuid::Uuid;

/// LoadManagerインスタンスIDの採番カウンタ
static NEXT_LOAD_MANAGER_ID: AtomicU64 = AtomicU64::new(1);

#[cfg(test)]
mod tests;

/// ロードマネージャー
///
/// # EndpointRegistry統合
///
/// EndpointRegistryを使用してエンドポイント情報を管理します。
#[derive(Clone)]
pub struct LoadManager {
    /// インスタンス固有ID（キャッシュキー用途）
    instance_id: u64,
    /// エンドポイントレジストリ
    endpoint_registry: Arc<EndpointRegistry>,
    state: Arc<RwLock<HashMap<Uuid, EndpointLoadState>>>,
    round_robin: Arc<AtomicUsize>,
    history: Arc<RwLock<VecDeque<RequestHistoryPoint>>>,
    /// ready通知
    ready_notify: Arc<Notify>,
    /// リクエストキュー待機中の通知
    queue_notify: Arc<Notify>,
    /// リクエストキュー待機数
    queue_waiters: Arc<AtomicUsize>,
    /// エンドポイント×モデル単位のTPS状態（SPEC-4bb5b55f）
    tps_tracker: Arc<RwLock<TpsTrackerMap>>,
}

impl LoadManager {
    /// 新しいロードマネージャーを作成
    pub fn new(endpoint_registry: Arc<EndpointRegistry>) -> Self {
        Self {
            instance_id: NEXT_LOAD_MANAGER_ID.fetch_add(1, AtomicOrdering::Relaxed),
            endpoint_registry,
            state: Arc::new(RwLock::new(HashMap::new())),
            round_robin: Arc::new(AtomicUsize::new(0)),
            history: Arc::new(RwLock::new(VecDeque::new())),
            ready_notify: Arc::new(Notify::new()),
            queue_notify: Arc::new(Notify::new()),
            queue_waiters: Arc::new(AtomicUsize::new(0)),
            tps_tracker: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// インスタンス単位のキャッシュキーを返す。
    pub fn cache_key(&self) -> u64 {
        self.instance_id
    }

    /// TPS計測値を更新（SPEC-4bb5b55f）
    pub async fn update_tps(
        &self,
        endpoint_id: Uuid,
        model_id: String,
        api_kind: TpsApiKind,
        output_tokens: u64,
        duration_ms: u64,
    ) {
        if duration_ms == 0 || output_tokens == 0 {
            return;
        }
        let mut tracker = self.tps_tracker.write().await;
        // オフライン/エラー遷移で clear_tps_for_endpoint された後に、in-flight リクエスト
        // 完了の遅延 update_tps が TPS エントリを再生成するのを防ぐ。
        // tps_tracker の書き込みロックを保持したまま現在のステータスを確認することで、
        // clear_tps_for_endpoint（同じロックを取得）との競合を直列化し、
        // ステータス確認と再生成の間の TOCTOU 競合を排除する。
        // レジストリ未登録（None）の場合は従来どおり更新する（テスト互換・production では
        // 削除直後の一時的な遅延更新のみが該当し、forget_endpoint が別途状態を掃除する）。
        if let Some(endpoint) = self.endpoint_registry.get(endpoint_id).await {
            if matches!(
                endpoint.status,
                crate::types::endpoint::EndpointStatus::Offline
                    | crate::types::endpoint::EndpointStatus::Error
            ) {
                return;
            }
        }
        let state = tracker
            .entry((endpoint_id, model_id, api_kind))
            .or_default();
        state.update_tps(output_tokens, duration_ms);
    }

    /// 指定エンドポイントのTPS状態をクリアする。
    ///
    /// エンドポイントがoffline/errorへ遷移した際に、復帰後は再計測から始める。
    pub async fn clear_tps_for_endpoint(&self, endpoint_id: Uuid) {
        let mut tracker = self.tps_tracker.write().await;
        tracker.retain(|(eid, _, _), _| *eid != endpoint_id);
    }

    /// エンドポイント削除時に LoadManager が保持する状態を破棄する。
    ///
    /// `EndpointRegistry::remove` はレジストリ・DB・モデルマッピングのみを掃除するため、
    /// LoadManager 側の `state`（負荷状態）と `tps_tracker`（TPS 状態）は残存しリークする。
    /// これらは `begin_request` / `update_tps` の `entry().or_default()` で挿入されるが
    /// 削除契機がないため、本メソッドを削除ハンドラから呼び出して完全に除去する。
    pub async fn forget_endpoint(&self, endpoint_id: Uuid) {
        {
            let mut state = self.state.write().await;
            state.remove(&endpoint_id);
        }
        {
            let mut tracker = self.tps_tracker.write().await;
            tracker.retain(|(eid, _, _), _| *eid != endpoint_id);
        }
    }

    /// エンドポイントのモデル別TPS情報を取得（SPEC-4bb5b55f）
    pub async fn get_model_tps(&self, endpoint_id: Uuid) -> Vec<ModelTpsInfo> {
        let tracker = self.tps_tracker.read().await;
        tracker
            .iter()
            .filter(|((eid, _, _), _)| *eid == endpoint_id)
            .map(|((_, model_id, api_kind), state)| ModelTpsInfo {
                model_id: model_id.clone(),
                api_kind: *api_kind,
                source: TpsSource::Production,
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

    /// 全エンドポイントのTPS概要を返す（SPEC-4bb5b55f T023）
    pub async fn get_all_endpoint_tps(&self) -> Vec<EndpointTpsSummary> {
        let tracker = self.tps_tracker.read().await;
        let mut map: HashMap<Uuid, EndpointTpsSummary> = HashMap::new();
        let mut model_sets: HashMap<Uuid, std::collections::HashSet<&str>> = HashMap::new();

        for ((endpoint_id, model_id, _), state) in tracker.iter() {
            let entry = map
                .entry(*endpoint_id)
                .or_insert_with(|| EndpointTpsSummary {
                    endpoint_id: *endpoint_id,
                    model_count: 0,
                    aggregate_tps: None,
                    total_output_tokens: 0,
                    total_requests: 0,
                });
            model_sets
                .entry(*endpoint_id)
                .or_default()
                .insert(model_id.as_str());
            entry.total_output_tokens += state.total_output_tokens;
            entry.total_requests += state.request_count;
        }

        for (endpoint_id, model_set) in model_sets {
            if let Some(entry) = map.get_mut(&endpoint_id) {
                entry.model_count = model_set.len();
            }
        }

        for ((endpoint_id, _, _), state) in tracker.iter() {
            if let Some(entry) = map.get_mut(endpoint_id) {
                if entry.aggregate_tps.is_none() {
                    let total_tokens: u64 = tracker
                        .iter()
                        .filter(|((eid, _, _), _)| eid == endpoint_id)
                        .map(|(_, s)| s.total_output_tokens)
                        .sum();
                    let total_duration: u64 = tracker
                        .iter()
                        .filter(|((eid, _, _), _)| eid == endpoint_id)
                        .map(|(_, s)| s.total_duration_ms)
                        .sum();
                    if total_duration > 0 {
                        entry.aggregate_tps =
                            Some(total_tokens as f64 / (total_duration as f64 / 1000.0));
                    }
                }
            }
            let _ = state;
        }

        map.into_values().collect()
    }

    async fn compute_endpoint_tps_scores(
        &self,
        endpoints: &[crate::types::endpoint::Endpoint],
        model_id: Option<&str>,
        api_kind: Option<TpsApiKind>,
    ) -> HashMap<Uuid, f64> {
        let tracker = self.tps_tracker.read().await;
        let mut scores = HashMap::with_capacity(endpoints.len());

        for endpoint in endpoints {
            let score = if let Some(model_id) = model_id {
                let Some(api_kind) = api_kind else {
                    scores.insert(endpoint.id, 0.0);
                    continue;
                };

                tracker
                    .iter()
                    .filter(|((eid, mid, kind), _)| {
                        *eid == endpoint.id && mid == model_id && *kind == api_kind
                    })
                    .filter_map(|(_, state)| state.tps_ema)
                    .fold(0.0, f64::max)
            } else {
                let (total_tokens, total_duration_ms) = tracker
                    .iter()
                    .filter(|((eid, _, kind), _)| {
                        *eid == endpoint.id && api_kind.is_none_or(|expected| *kind == expected)
                    })
                    .fold((0u64, 0u64), |(tokens, duration), (_, state)| {
                        (
                            tokens.saturating_add(state.total_output_tokens),
                            duration.saturating_add(state.total_duration_ms),
                        )
                    });

                if total_duration_ms > 0 {
                    total_tokens as f64 / (total_duration_ms as f64 / 1000.0)
                } else {
                    0.0
                }
            };

            scores.insert(endpoint.id, score);
        }

        scores
    }

    async fn select_endpoint_by_tps_from_endpoints(
        &self,
        endpoints: Vec<crate::types::endpoint::Endpoint>,
        model_id: Option<&str>,
        api_kind: Option<TpsApiKind>,
    ) -> RouterResult<crate::types::endpoint::Endpoint> {
        if endpoints.is_empty() {
            return Err(match model_id {
                Some(model_id) => LbError::NoCapableEndpoints(model_id.to_string()),
                None => LbError::NoEndpointsAvailable,
            });
        }

        let candidates: Vec<_> = {
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

        if candidates.is_empty() {
            return Err(LbError::NoEndpointsAvailable);
        }

        let scores = self
            .compute_endpoint_tps_scores(&candidates, model_id, api_kind)
            .await;
        let round_robin_cursor = self.round_robin.fetch_add(1, AtomicOrdering::SeqCst);
        let round_robin_start = round_robin_cursor % candidates.len().max(1);
        let round_robin_priority =
            compute_round_robin_priority_for_endpoints(&candidates, round_robin_start);

        let mut ordered = candidates;
        ordered.sort_by(|a, b| {
            let a_score = scores.get(&a.id).copied().unwrap_or(0.0);
            let b_score = scores.get(&b.id).copied().unwrap_or(0.0);

            b_score
                .partial_cmp(&a_score)
                .unwrap_or(std::cmp::Ordering::Equal)
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

        Ok(ordered
            .into_iter()
            .next()
            .expect("candidates checked as non-empty"))
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
            endpoint_id,
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

        if self.endpoint_registry.get(endpoint_id).await.is_none() {
            return Err(LbError::EndpointNotFound(endpoint_id));
        }

        let _ = self
            .endpoint_registry
            .update_gpu_info(
                endpoint_id,
                None,
                gpu_memory_total_mb.map(|mb| mb * 1024 * 1024),
                gpu_memory_used_mb.map(|mb| mb * 1024 * 1024),
                gpu_capability_score.map(|s| s as f32),
                Some(active_requests),
            )
            .await;

        let mut state = self.state.write().await;
        let entry = state.entry(endpoint_id).or_default();
        let was_active = entry.combined_active() > 0;
        let was_initializing = entry.initializing;

        let derived_average = average_response_time_ms.or_else(|| entry.average_latency_ms());
        let timestamp = Utc::now();
        let metrics = HealthMetrics {
            endpoint_id,
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

    /// エンドポイント登録時に初期状態を同期
    pub async fn upsert_initial_state(
        &self,
        endpoint_id: Uuid,
        initializing: bool,
        ready_models: Option<(u8, u8)>,
    ) {
        let mut state = self.state.write().await;
        let entry = state.entry(endpoint_id).or_default();
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

    /// リクエストキュー待機数を取得
    pub fn queue_waiters(&self) -> usize {
        self.queue_waiters.load(AtomicOrdering::Relaxed)
    }

    async fn has_idle_nodes(&self) -> bool {
        let endpoints = self.endpoint_registry.list_online().await;
        if endpoints.is_empty() {
            return false;
        }

        let state = self.state.read().await;
        endpoints.iter().any(|endpoint| {
            let load = state.get(&endpoint.id);
            let is_not_initializing = load.map(|l| !l.initializing).unwrap_or(true);
            let is_idle = load.map(|l| l.combined_active() == 0).unwrap_or(true);
            is_not_initializing && is_idle
        })
    }

    async fn has_idle_nodes_for_model(&self, model_id: &str) -> bool {
        let endpoints = self.endpoint_registry.find_by_model(model_id).await;
        if endpoints.is_empty() {
            return false;
        }

        let state = self.state.read().await;
        endpoints.iter().any(|endpoint| {
            let load = state.get(&endpoint.id);
            let is_not_initializing = load.map(|l| !l.initializing).unwrap_or(true);
            let is_idle = load.map(|l| l.combined_active() == 0).unwrap_or(true);
            is_not_initializing && is_idle
        })
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

    /// リクエスト開始を記録
    ///
    /// 選択(find_by_model は Online のみ返す)から本メソッド呼び出しまでの間に
    /// エンドポイントが Offline/Error へ遷移する TOCTOU を塞ぐため、dead 状態
    /// (Offline/Error)への割当を拒否する。Pending は従来どおり許可する。
    pub async fn begin_request(&self, endpoint_id: Uuid) -> RouterResult<RequestLease> {
        let Some(endpoint) = self.endpoint_registry.get(endpoint_id).await else {
            return Err(LbError::EndpointNotFound(endpoint_id));
        };
        if endpoint.status == crate::types::endpoint::EndpointStatus::Offline
            || endpoint.status == crate::types::endpoint::EndpointStatus::Error
        {
            return Err(LbError::EndpointOffline(endpoint_id));
        }

        let mut state = self.state.write().await;
        let entry = state.entry(endpoint_id).or_default();
        entry.assigned_active = entry.assigned_active.saturating_add(1);
        entry.total_assigned = entry.total_assigned.saturating_add(1);

        Ok(RequestLease::new(self.clone(), endpoint_id))
    }

    /// リクエスト完了を記録
    pub async fn finish_request(
        &self,
        endpoint_id: Uuid,
        outcome: RequestOutcome,
        duration: StdDuration,
    ) -> RouterResult<()> {
        if self.endpoint_registry.get(endpoint_id).await.is_none() {
            return Err(LbError::EndpointNotFound(endpoint_id));
        }

        let mut state = self.state.write().await;
        let entry = state.entry(endpoint_id).or_default();

        if let RequestOutcome::Queued = outcome {
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
        endpoint_id: Uuid,
        outcome: RequestOutcome,
        duration: StdDuration,
        token_usage: Option<crate::token::TokenUsage>,
    ) -> RouterResult<()> {
        if self.endpoint_registry.get(endpoint_id).await.is_none() {
            return Err(LbError::EndpointNotFound(endpoint_id));
        }

        let mut state = self.state.write().await;
        let entry = state.entry(endpoint_id).or_default();

        if let RequestOutcome::Queued = outcome {
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

            if let Some(ref usage) = token_usage {
                if let Some(input) = usage.input_tokens {
                    entry.total_input_tokens =
                        entry.total_input_tokens.saturating_add(input as u64);
                }
                if let Some(output) = usage.output_tokens {
                    entry.total_output_tokens =
                        entry.total_output_tokens.saturating_add(output as u64);
                }
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

    /// 指定されたエンドポイントのロードスナップショットを取得
    pub async fn snapshot(&self, endpoint_id: Uuid) -> RouterResult<EndpointLoadSnapshot> {
        let endpoint = self
            .endpoint_registry
            .get(endpoint_id)
            .await
            .ok_or(LbError::EndpointNotFound(endpoint_id))?;
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
    pub async fn metrics_history(&self, endpoint_id: Uuid) -> RouterResult<Vec<HealthMetrics>> {
        if self.endpoint_registry.get(endpoint_id).await.is_none() {
            return Err(LbError::EndpointNotFound(endpoint_id));
        }
        let state = self.state.read().await;
        let history = state
            .get(&endpoint_id)
            .map(|load_state| load_state.metrics_history.iter().cloned().collect())
            .unwrap_or_else(Vec::new);
        Ok(history)
    }

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
            registering_nodes: 0,
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

    /// 起動時にDBからリクエスト履歴をseedする
    pub async fn seed_history_from_db(
        &self,
        points: Vec<crate::db::request_history::MinuteHistoryPoint>,
    ) {
        let mut history = self.history.write().await;
        for point in points {
            if let Ok(minute) = chrono::DateTime::parse_from_rfc3339(&point.minute) {
                let minute = minute.with_timezone(&Utc);
                history.push_back(RequestHistoryPoint {
                    minute,
                    success: point.success_count as u64,
                    error: point.error_count as u64,
                });
            }
        }
        // 古いエントリをプルーニング
        let now = align_to_minute(Utc::now());
        prune_history(&mut history, now);
    }

    /// 起動時にDBからTPS状態をseedする
    pub async fn seed_tps_from_db(
        &self,
        entries: Vec<crate::db::endpoint_daily_stats::TpsSeedEntry>,
    ) {
        let mut tracker = self.tps_tracker.write().await;
        for entry in entries {
            if entry.total_duration_ms <= 0 || entry.total_output_tokens <= 0 {
                continue;
            }
            let tps = entry.total_output_tokens as f64 / (entry.total_duration_ms as f64 / 1000.0);
            let api_kind = match entry.api_kind.as_str() {
                "completions" => TpsApiKind::Completions,
                "responses" => TpsApiKind::Responses,
                _ => TpsApiKind::ChatCompletions,
            };
            let key = (entry.endpoint_id, entry.model_id, api_kind);
            let state = tracker.entry(key).or_default();
            state.tps_ema = Some(tps);
            state.request_count = entry.successful_requests as u64;
            state.total_output_tokens = entry.total_output_tokens as u64;
            state.total_duration_ms = entry.total_duration_ms as u64;
        }
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

    async fn collect_online_endpoints(
        &self,
        model_id: Option<&str>,
    ) -> RouterResult<Vec<crate::types::endpoint::Endpoint>> {
        if let Some(model_id) = model_id {
            let endpoints = self.endpoint_registry.find_by_model(model_id).await;
            if endpoints.is_empty() {
                return Err(LbError::NoCapableEndpoints(model_id.to_string()));
            }
            return Ok(endpoints);
        }

        let endpoints = self.endpoint_registry.list_online().await;
        if endpoints.is_empty() {
            return Err(LbError::NoEndpointsAvailable);
        }

        Ok(endpoints)
    }

    /// エンドポイントを直接選択（ラウンドロビン）
    pub async fn select_endpoint_direct(&self) -> RouterResult<crate::types::endpoint::Endpoint> {
        let endpoints = self.collect_online_endpoints(None).await?;
        self.select_endpoint_round_robin_from_endpoints(endpoints)
    }

    /// エンドポイントをTPS優先で直接選択する。
    ///
    /// `api_kind` を指定した場合、そのAPI種別の集計TPSを優先度に用いる。
    /// 未計測エンドポイントはTPS=0.0として最低優先になる。
    pub async fn select_endpoint_by_tps_direct(
        &self,
        api_kind: Option<TpsApiKind>,
    ) -> RouterResult<crate::types::endpoint::Endpoint> {
        let endpoints = self.collect_online_endpoints(None).await?;
        self.select_endpoint_by_tps_from_endpoints(endpoints, None, api_kind)
            .await
    }

    /// 指定モデルに対応するエンドポイントを直接選択（ラウンドロビン）
    pub async fn select_endpoint_direct_for_model(
        &self,
        model_id: &str,
    ) -> RouterResult<crate::types::endpoint::Endpoint> {
        let endpoints = self.collect_online_endpoints(Some(model_id)).await?;
        self.select_endpoint_round_robin_from_endpoints(endpoints)
    }

    /// 指定モデルに対応する初期化完了エンドポイントをラウンドロビンで選択
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

    /// 指定モデルに対応する初期化完了エンドポイントをTPS優先で選択する。
    ///
    /// 実装上は初期化中除外を共通処理で行うため、TPS優先選択の標準経路として使う。
    pub async fn select_endpoint_by_tps_ready_for_model(
        &self,
        model_id: &str,
        api_kind: Option<TpsApiKind>,
    ) -> RouterResult<crate::types::endpoint::Endpoint> {
        let endpoints = self.collect_online_endpoints(Some(model_id)).await?;
        self.select_endpoint_by_tps_from_endpoints(endpoints, Some(model_id), api_kind)
            .await
    }

    /// 指定済み候補から、指定モデルに対応する初期化完了エンドポイントをTPS優先で選択する。
    pub async fn select_endpoint_by_tps_ready_from_candidates(
        &self,
        endpoints: Vec<crate::types::endpoint::Endpoint>,
        model_id: &str,
        api_kind: Option<TpsApiKind>,
    ) -> RouterResult<crate::types::endpoint::Endpoint> {
        self.select_endpoint_by_tps_from_endpoints(endpoints, Some(model_id), api_kind)
            .await
    }

    fn select_endpoint_round_robin_from_endpoints(
        &self,
        endpoints: Vec<crate::types::endpoint::Endpoint>,
    ) -> RouterResult<crate::types::endpoint::Endpoint> {
        if endpoints.is_empty() {
            return Err(LbError::NoEndpointsAvailable);
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
        RequestOutcome::Queued => {}
    }
}

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
