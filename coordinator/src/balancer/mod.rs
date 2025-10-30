//! ロードバランサーモジュール
//!
//! エージェントに関する最新メトリクスとリクエスト統計を集約し、
//! 高度なロードバランシング戦略を提供する。

use crate::registry::AgentRegistry;
use chrono::{DateTime, Utc};
use ollama_coordinator_common::{
    error::{CoordinatorError, CoordinatorResult},
    types::{Agent, AgentStatus, HealthMetrics},
};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::RwLock;
use uuid::Uuid;

/// リクエスト結果
#[derive(Debug, Clone, Copy)]
pub enum RequestOutcome {
    /// 正常終了
    Success,
    /// エラー終了
    Error,
}

/// エージェントの最新ロード状態
#[derive(Debug, Clone, Default)]
struct AgentLoadState {
    last_metrics: Option<HealthMetrics>,
    assigned_active: u32,
    total_assigned: u64,
    success_count: u64,
    error_count: u64,
    total_latency_ms: u128,
}

impl AgentLoadState {
    fn combined_active(&self) -> u32 {
        let heartbeat_active = self
            .last_metrics
            .as_ref()
            .map(|m| m.active_requests)
            .unwrap_or(0);
        heartbeat_active.saturating_add(self.assigned_active)
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
}

/// エージェントのロードスナップショット
#[derive(Debug, Clone, Serialize)]
pub struct AgentLoadSnapshot {
    /// エージェントID
    pub agent_id: Uuid,
    /// マシン名
    pub machine_name: String,
    /// エージェント状態
    pub status: AgentStatus,
    /// CPU使用率
    pub cpu_usage: Option<f32>,
    /// メモリ使用率
    pub memory_usage: Option<f32>,
    /// 処理中リクエスト数（Coordinator観点+エージェント自己申告）
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
}

/// ロードマネージャー
#[derive(Clone)]
pub struct LoadManager {
    registry: AgentRegistry,
    state: Arc<RwLock<HashMap<Uuid, AgentLoadState>>>,
    round_robin: Arc<AtomicUsize>,
}

impl LoadManager {
    /// 新しいロードマネージャーを作成
    pub fn new(registry: AgentRegistry) -> Self {
        Self {
            registry,
            state: Arc::new(RwLock::new(HashMap::new())),
            round_robin: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// ヘルスメトリクスを記録
    pub async fn record_metrics(
        &self,
        agent_id: Uuid,
        cpu_usage: f32,
        memory_usage: f32,
        active_requests: u32,
    ) -> CoordinatorResult<()> {
        // エージェントが存在することを確認
        self.registry.get(agent_id).await?;

        let mut state = self.state.write().await;
        let entry = state.entry(agent_id).or_default();

        entry.last_metrics = Some(HealthMetrics {
            agent_id,
            cpu_usage,
            memory_usage,
            active_requests,
            total_requests: entry.total_assigned,
            timestamp: Utc::now(),
        });

        Ok(())
    }

    /// リクエスト開始を記録
    pub async fn begin_request(&self, agent_id: Uuid) -> CoordinatorResult<()> {
        self.registry.get(agent_id).await?;

        let mut state = self.state.write().await;
        let entry = state.entry(agent_id).or_default();
        entry.assigned_active = entry.assigned_active.saturating_add(1);
        entry.total_assigned = entry.total_assigned.saturating_add(1);

        // 最新メトリクスがある場合はトータルリクエスト数も同期
        if let Some(metrics) = entry.last_metrics.as_mut() {
            metrics.total_requests = entry.total_assigned;
        }

        Ok(())
    }

    /// リクエスト完了を記録
    pub async fn finish_request(
        &self,
        agent_id: Uuid,
        outcome: RequestOutcome,
        duration: Duration,
    ) -> CoordinatorResult<()> {
        self.registry.get(agent_id).await?;

        let mut state = self.state.write().await;
        let entry = state.entry(agent_id).or_default();

        if entry.assigned_active > 0 {
            entry.assigned_active -= 1;
        }

        match outcome {
            RequestOutcome::Success => entry.success_count = entry.success_count.saturating_add(1),
            RequestOutcome::Error => entry.error_count = entry.error_count.saturating_add(1),
        }

        entry.total_latency_ms = entry
            .total_latency_ms
            .saturating_add(duration.as_millis() as u128);

        Ok(())
    }

    /// 適切なエージェントを選択
    pub async fn select_agent(&self) -> CoordinatorResult<Agent> {
        let agents = self.registry.list().await;

        let online_agents: Vec<_> = agents
            .into_iter()
            .filter(|agent| agent.status == AgentStatus::Online)
            .collect();

        if online_agents.is_empty() {
            return Err(CoordinatorError::NoAgentsAvailable);
        }

        let state = self.state.read().await;

        let mut load_based_candidates: Vec<(Agent, AgentLoadState)> = Vec::new();
        for agent in &online_agents {
            if let Some(load_state) = state.get(&agent.id) {
                if let Some(metrics) = &load_state.last_metrics {
                    if metrics.cpu_usage <= 80.0 {
                        load_based_candidates.push((agent.clone(), load_state.clone()));
                    }
                }
            }
        }

        if !load_based_candidates.is_empty() {
            load_based_candidates.sort_by(|a, b| {
                let a_active = a.1.combined_active();
                let b_active = b.1.combined_active();
                a_active
                    .cmp(&b_active)
                    .then_with(|| a.1.total_assigned.cmp(&b.1.total_assigned))
                    .then_with(|| a.0.machine_name.cmp(&b.0.machine_name))
            });

            return Ok(load_based_candidates[0].0.clone());
        }

        // すべてのエージェントが高負荷またはメトリクスなし → ラウンドロビン
        let next_index = self
            .round_robin
            .fetch_add(1, Ordering::SeqCst)
            .rem_euclid(online_agents.len());
        Ok(online_agents[next_index].clone())
    }

    /// 指定されたエージェントのロードスナップショットを取得
    pub async fn snapshot(&self, agent_id: Uuid) -> CoordinatorResult<AgentLoadSnapshot> {
        let agent = self.registry.get(agent_id).await?;
        let state = self.state.read().await;
        let load_state = state.get(&agent_id).cloned().unwrap_or_default();

        Ok(self.build_snapshot(agent, load_state))
    }

    /// すべてのエージェントのロードスナップショットを取得
    pub async fn snapshots(&self) -> Vec<AgentLoadSnapshot> {
        let agents = self.registry.list().await;
        let state = self.state.read().await;

        agents
            .into_iter()
            .map(|agent| {
                let load_state = state.get(&agent.id).cloned().unwrap_or_default();
                self.build_snapshot(agent, load_state)
            })
            .collect()
    }

    fn build_snapshot(&self, agent: Agent, load_state: AgentLoadState) -> AgentLoadSnapshot {
        let cpu_usage = load_state
            .last_metrics
            .as_ref()
            .map(|metrics| metrics.cpu_usage);
        let memory_usage = load_state
            .last_metrics
            .as_ref()
            .map(|metrics| metrics.memory_usage);
        let active_requests = load_state.combined_active();

        AgentLoadSnapshot {
            agent_id: agent.id,
            machine_name: agent.machine_name,
            status: agent.status,
            cpu_usage,
            memory_usage,
            active_requests,
            total_requests: load_state.total_assigned,
            successful_requests: load_state.success_count,
            failed_requests: load_state.error_count,
            average_response_time_ms: load_state.average_latency_ms(),
            last_updated: load_state.last_updated(),
        }
    }
}
