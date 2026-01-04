//! ダッシュボードAPIハンドラー
//!
//! `/v0/dashboard/*` 系のエンドポイントを提供し、ノードの状態および
//! システム統計を返却する。

use super::nodes::AppError;
use crate::{
    balancer::{NodeLoadSnapshot, RequestHistoryPoint},
    AppState,
};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Response,
    Json,
};
use chrono::{DateTime, Utc};
use llm_router_common::types::{GpuDeviceInfo, HealthMetrics, NodeStatus, SyncProgress, SyncState};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Instant};
use uuid::Uuid;

/// ノードのダッシュボード表示用サマリー
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DashboardNode {
    /// ノードID
    pub id: Uuid,
    /// マシン名
    pub machine_name: String,
    /// IPアドレス（文字列化）
    pub ip_address: String,
    /// LLM runtime バージョン
    pub runtime_version: String,
    /// LLM runtime ポート
    pub runtime_port: u16,
    /// ステータス
    pub status: NodeStatus,
    /// 登録日時
    pub registered_at: DateTime<Utc>,
    /// 最終確認時刻
    pub last_seen: DateTime<Utc>,
    /// 稼働秒数
    pub uptime_seconds: i64,
    /// ロード済みモデル一覧
    #[serde(default)]
    pub loaded_models: Vec<String>,
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
    /// GPUデバイス一覧
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gpu_devices: Vec<GpuDeviceInfo>,
    /// GPU計算能力
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_compute_capability: Option<String>,
    /// GPU能力スコア
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_capability_score: Option<u32>,
    /// 処理中リクエスト数
    pub active_requests: u32,
    /// 累積リクエスト数
    pub total_requests: u64,
    /// 成功リクエスト数
    pub successful_requests: u64,
    /// 失敗リクエスト数
    pub failed_requests: u64,
    /// 平均レスポンスタイム
    pub average_response_time_ms: Option<f32>,
    /// メトリクス最終更新時刻
    pub metrics_last_updated_at: Option<DateTime<Utc>>,
    /// メトリクスが古いか
    pub metrics_stale: bool,
    /// GPU利用可能フラグ
    pub gpu_available: Option<bool>,
    /// GPUモデル名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_model: Option<String>,
    /// GPU個数
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_count: Option<u32>,
    /// モデル同期状態
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_state: Option<SyncState>,
    /// モデル同期の進捗
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_progress: Option<SyncProgress>,
    /// 同期状態の最終更新時刻
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_updated_at: Option<DateTime<Utc>>,
    /// 入力トークン累計
    pub total_input_tokens: u64,
    /// 出力トークン累計
    pub total_output_tokens: u64,
    /// 総トークン累計
    pub total_tokens: u64,
}

/// システム統計レスポンス
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DashboardStats {
    /// 登録ノード総数
    pub total_nodes: usize,
    /// オンラインノード数
    pub online_nodes: usize,
    /// 承認待ちノード数
    pub pending_nodes: usize,
    /// 登録中ノード数
    pub registering_nodes: usize,
    /// オフラインノード数
    pub offline_nodes: usize,
    /// 累積リクエスト数
    pub total_requests: u64,
    /// 成功リクエスト数
    pub successful_requests: u64,
    /// 失敗リクエスト数
    pub failed_requests: u64,
    /// 処理中リクエスト数
    pub total_active_requests: u32,
    /// 待機中リクエスト数
    pub queued_requests: usize,
    /// 平均レスポンスタイム
    pub average_response_time_ms: Option<f32>,
    /// 平均GPU使用率
    pub average_gpu_usage: Option<f32>,
    /// 平均GPUメモリ使用率
    pub average_gpu_memory_usage: Option<f32>,
    /// 最新メトリクス更新時刻
    pub last_metrics_updated_at: Option<DateTime<Utc>>,
    /// 最新登録日時
    pub last_registered_at: Option<DateTime<Utc>>,
    /// 最新ヘルスチェック時刻
    pub last_seen_at: Option<DateTime<Utc>>,
    /// OPENAI_API_KEY が設定されているか
    pub openai_key_present: bool,
    /// GOOGLE_API_KEY が設定されているか
    pub google_key_present: bool,
    /// ANTHROPIC_API_KEY が設定されているか
    pub anthropic_key_present: bool,
    /// 入力トークン累計
    pub total_input_tokens: u64,
    /// 出力トークン累計
    pub total_output_tokens: u64,
    /// 総トークン累計
    pub total_tokens: u64,
}

/// ダッシュボード概要レスポンス
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DashboardOverview {
    /// ノード一覧
    pub nodes: Vec<DashboardNode>,
    /// システム統計
    pub stats: DashboardStats,
    /// リクエスト履歴
    pub history: Vec<RequestHistoryPoint>,
    /// レスポンス生成時刻
    pub generated_at: DateTime<Utc>,
    /// 集計に要した時間（ミリ秒）
    pub generation_time_ms: u64,
}

/// GET /v0/dashboard/nodes
pub async fn get_nodes(State(state): State<AppState>) -> Json<Vec<DashboardNode>> {
    Json(collect_nodes(&state).await)
}

/// GET /v0/dashboard/stats
pub async fn get_stats(State(state): State<AppState>) -> Json<DashboardStats> {
    Json(collect_stats(&state).await)
}

/// GET /v0/dashboard/request-history
pub async fn get_request_history(State(state): State<AppState>) -> Json<Vec<RequestHistoryPoint>> {
    Json(collect_history(&state).await)
}

/// GET /v0/dashboard/overview
pub async fn get_overview(State(state): State<AppState>) -> Json<DashboardOverview> {
    let started = Instant::now();
    let nodes = collect_nodes(&state).await;
    let stats = collect_stats(&state).await;
    let history = collect_history(&state).await;
    let generation_time_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    let generated_at = Utc::now();
    Json(DashboardOverview {
        nodes,
        stats,
        history,
        generated_at,
        generation_time_ms,
    })
}

/// GET /v0/dashboard/metrics/:node_id
pub async fn get_node_metrics(
    Path(node_id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Vec<HealthMetrics>>, AppError> {
    let history = state.load_manager.metrics_history(node_id).await?;
    Ok(Json(history))
}

async fn collect_nodes(state: &AppState) -> Vec<DashboardNode> {
    let registry = state.registry.clone();
    let load_manager = state.load_manager.clone();

    let nodes = registry.list().await;
    let snapshots = load_manager.snapshots().await;
    let snapshot_map = snapshots
        .into_iter()
        .map(|snapshot| (snapshot.node_id, snapshot))
        .collect::<HashMap<Uuid, NodeLoadSnapshot>>();

    let now = Utc::now();

    nodes
        .into_iter()
        .map(|node| {
            let uptime_seconds = if let Some(online_since) = node.online_since {
                let end = if node.status == NodeStatus::Online {
                    now
                } else {
                    node.last_seen
                };
                (end - online_since).num_seconds().max(0)
            } else {
                0
            };

            let snapshot = snapshot_map.get(&node.id);
            let (
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
                total_requests,
                successful_requests,
                failed_requests,
                average_response_time_ms,
                metrics_last_updated_at,
                metrics_stale,
                total_input_tokens,
                total_output_tokens,
                total_tokens,
            ) = if let Some(snapshot) = snapshot {
                (
                    snapshot.cpu_usage,
                    snapshot.memory_usage,
                    snapshot.gpu_usage,
                    snapshot.gpu_memory_usage,
                    snapshot.gpu_memory_total_mb,
                    snapshot.gpu_memory_used_mb,
                    snapshot.gpu_temperature,
                    snapshot.gpu_model_name.clone(),
                    snapshot.gpu_compute_capability.clone(),
                    snapshot.gpu_capability_score,
                    snapshot.active_requests,
                    snapshot.total_requests,
                    snapshot.successful_requests,
                    snapshot.failed_requests,
                    snapshot.average_response_time_ms,
                    snapshot.last_updated,
                    snapshot.is_stale,
                    snapshot.total_input_tokens,
                    snapshot.total_output_tokens,
                    snapshot.total_tokens,
                )
            } else {
                (
                    None, None, None, None, None, None, None, None, None, None, 0, 0, 0, 0, None,
                    None, true, 0, 0, 0,
                )
            };

            DashboardNode {
                id: node.id,
                machine_name: node.machine_name,
                ip_address: node.ip_address.to_string(),
                runtime_version: node.runtime_version,
                runtime_port: node.runtime_port,
                status: node.status,
                registered_at: node.registered_at,
                last_seen: node.last_seen,
                uptime_seconds,
                loaded_models: node.loaded_models.clone(),
                cpu_usage,
                memory_usage,
                gpu_usage,
                gpu_memory_usage,
                gpu_memory_total_mb,
                gpu_memory_used_mb,
                gpu_temperature,
                gpu_model_name,
                gpu_devices: node.gpu_devices.clone(),
                gpu_compute_capability,
                gpu_capability_score,
                active_requests,
                total_requests,
                successful_requests,
                failed_requests,
                average_response_time_ms,
                metrics_last_updated_at,
                metrics_stale,
                gpu_available: Some(node.gpu_available),
                gpu_model: node.gpu_model.clone(),
                gpu_count: node.gpu_count,
                sync_state: node.sync_state,
                sync_progress: node.sync_progress.clone(),
                sync_updated_at: node.sync_updated_at,
                total_input_tokens,
                total_output_tokens,
                total_tokens,
            }
        })
        .collect::<Vec<DashboardNode>>()
}

async fn collect_stats(state: &AppState) -> DashboardStats {
    let load_manager = state.load_manager.clone();
    let registry = state.registry.clone();

    let summary = load_manager.summary().await;
    let nodes = registry.list().await;

    let last_registered_at = nodes.iter().map(|node| node.registered_at).max();
    let last_seen_at = nodes.iter().map(|node| node.last_seen).max();

    let openai_key_present = std::env::var("OPENAI_API_KEY").is_ok();
    let google_key_present = std::env::var("GOOGLE_API_KEY").is_ok();
    let anthropic_key_present = std::env::var("ANTHROPIC_API_KEY").is_ok();

    DashboardStats {
        total_nodes: summary.total_nodes,
        online_nodes: summary.online_nodes,
        pending_nodes: summary.pending_nodes,
        registering_nodes: summary.registering_nodes,
        offline_nodes: summary.offline_nodes,
        total_requests: summary.total_requests,
        successful_requests: summary.successful_requests,
        failed_requests: summary.failed_requests,
        total_active_requests: summary.total_active_requests,
        queued_requests: summary.queued_requests,
        average_response_time_ms: summary.average_response_time_ms,
        average_gpu_usage: summary.average_gpu_usage,
        average_gpu_memory_usage: summary.average_gpu_memory_usage,
        last_metrics_updated_at: summary.last_metrics_updated_at,
        last_registered_at,
        last_seen_at,
        openai_key_present,
        google_key_present,
        anthropic_key_present,
        total_input_tokens: summary.total_input_tokens,
        total_output_tokens: summary.total_output_tokens,
        total_tokens: summary.total_tokens,
    }
}

async fn collect_history(state: &AppState) -> Vec<RequestHistoryPoint> {
    state.load_manager.request_history().await
}

/// 許可されたページサイズ
pub const ALLOWED_PAGE_SIZES: &[usize] = &[10, 25, 50, 100];

/// デフォルトのページサイズ
pub const DEFAULT_PAGE_SIZE: usize = 10;

/// リクエスト履歴一覧のクエリパラメータ
#[derive(Debug, Clone, Deserialize)]
pub struct RequestHistoryQuery {
    /// ページ番号（1始まり）
    #[serde(default = "default_page")]
    pub page: usize,
    /// 1ページあたりの件数（10, 25, 50, 100のいずれか）
    #[serde(default = "default_per_page")]
    pub per_page: usize,
}

fn default_page() -> usize {
    1
}

fn default_per_page() -> usize {
    DEFAULT_PAGE_SIZE
}

impl RequestHistoryQuery {
    /// ページサイズを正規化（許可された値のいずれかに制限）
    pub fn normalized_per_page(&self) -> usize {
        if ALLOWED_PAGE_SIZES.contains(&self.per_page) {
            self.per_page
        } else {
            DEFAULT_PAGE_SIZE
        }
    }
}

/// T023: リクエスト履歴一覧API
pub async fn list_request_responses(
    State(state): State<AppState>,
    Query(query): Query<RequestHistoryQuery>,
) -> Result<Json<crate::db::request_history::FilteredRecords>, AppError> {
    let filter = crate::db::request_history::RecordFilter::default();
    let page = if query.page == 0 { 1 } else { query.page };
    let per_page = query.normalized_per_page();
    let result = state
        .request_history
        .filter_and_paginate(&filter, page, per_page)
        .await
        .map_err(AppError::from)?;
    Ok(Json(result))
}

/// T024: リクエスト履歴詳細API
pub async fn get_request_response_detail(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<llm_router_common::protocol::RequestResponseRecord>, AppError> {
    let records = state
        .request_history
        .load_records()
        .await
        .map_err(AppError::from)?;
    let record = records.into_iter().find(|r| r.id == id).ok_or_else(|| {
        llm_router_common::error::RouterError::Database(format!("Record {} not found", id))
    })?;
    Ok(Json(record))
}

/// T025: エクスポートAPI
pub async fn export_request_responses(State(state): State<AppState>) -> Result<Response, AppError> {
    let records = state
        .request_history
        .load_records()
        .await
        .map_err(AppError::from)?;

    // CSV形式でエクスポート
    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record([
        "id",
        "timestamp",
        "request_type",
        "model",
        "node_id",
        "node_machine_name",
        "node_ip",
        "client_ip",
        "duration_ms",
        "status",
        "completed_at",
    ])
    .map_err(|e| {
        llm_router_common::error::RouterError::Internal(format!("CSV header error: {}", e))
    })?;

    for record in records {
        let status_str = match &record.status {
            llm_router_common::protocol::RecordStatus::Success => "success".to_string(),
            llm_router_common::protocol::RecordStatus::Error { message } => {
                format!("error: {}", message)
            }
        };

        wtr.write_record(&[
            record.id.to_string(),
            record.timestamp.to_rfc3339(),
            format!("{:?}", record.request_type),
            record.model,
            record.node_id.to_string(),
            record.node_machine_name,
            record.node_ip.to_string(),
            record
                .client_ip
                .map(|ip| ip.to_string())
                .unwrap_or_default(),
            record.duration_ms.to_string(),
            status_str,
            record.completed_at.to_rfc3339(),
        ])
        .map_err(|e| {
            llm_router_common::error::RouterError::Internal(format!("CSV write error: {}", e))
        })?;
    }

    let csv_data = wtr.into_inner().map_err(|e| {
        llm_router_common::error::RouterError::Internal(format!("CSV finalize error: {}", e))
    })?;

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/csv")
        .header(
            "Content-Disposition",
            "attachment; filename=\"request_history.csv\"",
        )
        .body(Body::from(csv_data))
        .unwrap();

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        balancer::{LoadManager, MetricsUpdate, RequestOutcome},
        registry::NodeRegistry,
    };
    use llm_router_common::{protocol::RegisterRequest, types::GpuDeviceInfo};
    use std::net::{IpAddr, Ipv4Addr};
    use tokio::time::Duration;

    async fn create_state() -> AppState {
        let registry = NodeRegistry::new();
        let load_manager = LoadManager::new(registry.clone());
        let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to create test database");
        sqlx::migrate!("./migrations")
            .run(&db_pool)
            .await
            .expect("Failed to run migrations");
        let request_history = std::sync::Arc::new(
            crate::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
        );
        let jwt_secret = "test-secret".to_string();
        AppState {
            registry,
            load_manager,
            request_history,
            db_pool,
            jwt_secret,
            http_client: reqwest::Client::new(),
            queue_config: crate::config::QueueConfig::from_env(),
            event_bus: crate::events::create_shared_event_bus(),
        }
    }

    fn sample_gpu_devices() -> Vec<GpuDeviceInfo> {
        vec![GpuDeviceInfo {
            model: "Test GPU".to_string(),
            count: 1,
            memory: None,
        }]
    }

    #[tokio::test]
    async fn test_get_nodes_returns_joined_state() {
        let state = create_state().await;

        // ノードを登録
        let register_req = RegisterRequest {
            machine_name: "node-01".into(),
            ip_address: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            runtime_version: "0.1.0".into(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        };
        let node_id = state.registry.register(register_req).await.unwrap().node_id;
        state.registry.approve(node_id).await.unwrap();

        // メトリクスを記録（ready_models を渡すと Registering → Online に遷移）
        state
            .load_manager
            .record_metrics(MetricsUpdate {
                node_id,
                cpu_usage: 32.5,
                memory_usage: 48.0,
                gpu_usage: Some(72.0),
                gpu_memory_usage: Some(68.0),
                gpu_memory_total_mb: None,
                gpu_memory_used_mb: None,
                gpu_temperature: None,
                gpu_model_name: None,
                gpu_compute_capability: None,
                gpu_capability_score: None,
                active_requests: 2,
                average_response_time_ms: Some(110.0),
                initializing: false,
                ready_models: Some((0, 0)),
            })
            .await
            .unwrap();
        state.load_manager.begin_request(node_id).await.unwrap();
        state
            .load_manager
            .finish_request(node_id, RequestOutcome::Success, Duration::from_millis(120))
            .await
            .unwrap();

        let response = get_nodes(State(state.clone())).await;
        let body = response.0;

        assert_eq!(body.len(), 1);
        let node = &body[0];
        assert_eq!(node.machine_name, "node-01");
        assert_eq!(node.status, NodeStatus::Online);
        assert_eq!(node.runtime_port, 32768);
        assert_eq!(node.total_requests, 1);
        assert_eq!(node.successful_requests, 1);
        assert_eq!(node.failed_requests, 0);
        assert_eq!(node.average_response_time_ms, Some(120.0));
        assert!(node.cpu_usage.is_some());
        assert!(node.memory_usage.is_some());
        assert_eq!(node.gpu_usage, Some(72.0));
        assert_eq!(node.gpu_memory_usage, Some(68.0));
    }

    #[tokio::test]
    async fn test_get_stats_summarises_registry_and_metrics() {
        let state = create_state().await;

        let first_node = state
            .registry
            .register(RegisterRequest {
                machine_name: "node-01".into(),
                ip_address: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
                runtime_version: "0.1.0".into(),
                runtime_port: 32768,
                gpu_available: true,
                gpu_devices: sample_gpu_devices(),
                gpu_count: Some(1),
                gpu_model: Some("Test GPU".to_string()),
                supported_runtimes: Vec::new(),
            })
            .await
            .unwrap()
            .node_id;
        state.registry.approve(first_node).await.unwrap();

        let second_node = state
            .registry
            .register(RegisterRequest {
                machine_name: "node-02".into(),
                ip_address: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
                runtime_version: "0.1.0".into(),
                runtime_port: 32768,
                gpu_available: true,
                gpu_devices: sample_gpu_devices(),
                gpu_count: Some(1),
                gpu_model: Some("Test GPU".to_string()),
                supported_runtimes: Vec::new(),
            })
            .await
            .unwrap()
            .node_id;
        state.registry.approve(second_node).await.unwrap();

        // 両ノードをOnline状態にするため、ready_modelsでメトリクスを記録
        state
            .load_manager
            .record_metrics(MetricsUpdate {
                node_id: first_node,
                cpu_usage: 40.0,
                memory_usage: 65.0,
                gpu_usage: None,
                gpu_memory_usage: None,
                gpu_memory_total_mb: None,
                gpu_memory_used_mb: None,
                gpu_temperature: None,
                gpu_model_name: None,
                gpu_compute_capability: None,
                gpu_capability_score: None,
                active_requests: 3,
                average_response_time_ms: Some(95.0),
                initializing: false,
                ready_models: Some((0, 0)),
            })
            .await
            .unwrap();
        state
            .load_manager
            .record_metrics(MetricsUpdate {
                node_id: second_node,
                cpu_usage: 30.0,
                memory_usage: 50.0,
                gpu_usage: None,
                gpu_memory_usage: None,
                gpu_memory_total_mb: None,
                gpu_memory_used_mb: None,
                gpu_temperature: None,
                gpu_model_name: None,
                gpu_compute_capability: None,
                gpu_capability_score: None,
                active_requests: 0,
                average_response_time_ms: None,
                initializing: false,
                ready_models: Some((0, 0)),
            })
            .await
            .unwrap();
        state.load_manager.begin_request(first_node).await.unwrap();
        state
            .load_manager
            .finish_request(
                first_node,
                RequestOutcome::Error,
                Duration::from_millis(150),
            )
            .await
            .unwrap();

        let stats = get_stats(State(state)).await.0;
        assert_eq!(stats.total_nodes, 2);
        assert_eq!(stats.online_nodes, 2);
        assert_eq!(stats.pending_nodes, 0);
        assert_eq!(stats.registering_nodes, 0);
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.failed_requests, 1);
        assert_eq!(stats.successful_requests, 0);
        assert!(stats.last_registered_at.is_some());
        assert!(stats.last_seen_at.is_some());
        assert!(stats.average_gpu_usage.is_none());
        assert!(stats.average_gpu_memory_usage.is_none());
    }

    #[tokio::test]
    async fn test_get_request_history_returns_series() {
        let state = create_state().await;

        let node_id = state
            .registry
            .register(RegisterRequest {
                machine_name: "node-history".into(),
                ip_address: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 11)),
                runtime_version: "0.1.0".into(),
                runtime_port: 32768,
                gpu_available: true,
                gpu_devices: sample_gpu_devices(),
                gpu_count: Some(1),
                gpu_model: Some("Test GPU".to_string()),
                supported_runtimes: Vec::new(),
            })
            .await
            .unwrap()
            .node_id;
        state.registry.approve(node_id).await.unwrap();

        state.load_manager.begin_request(node_id).await.unwrap();
        state
            .load_manager
            .finish_request(node_id, RequestOutcome::Success, Duration::from_millis(150))
            .await
            .unwrap();

        state.load_manager.begin_request(node_id).await.unwrap();
        state
            .load_manager
            .finish_request(node_id, RequestOutcome::Error, Duration::from_millis(200))
            .await
            .unwrap();

        let history = get_request_history(State(state.clone())).await.0;
        assert_eq!(history.len() as i64, 60);
        let latest = history.last().unwrap();
        assert!(latest.success >= 1);
        assert!(latest.error >= 1);
    }

    #[tokio::test]
    async fn test_get_overview_combines_all_sections() {
        let state = create_state().await;

        let node_id = state
            .registry
            .register(RegisterRequest {
                machine_name: "overview".into(),
                ip_address: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 21)),
                runtime_version: "0.1.0".into(),
                runtime_port: 32768,
                gpu_available: true,
                gpu_devices: sample_gpu_devices(),
                gpu_count: Some(1),
                gpu_model: Some("Test GPU".to_string()),
                supported_runtimes: Vec::new(),
            })
            .await
            .unwrap()
            .node_id;
        state.registry.approve(node_id).await.unwrap();

        state.load_manager.begin_request(node_id).await.unwrap();
        state
            .load_manager
            .finish_request(node_id, RequestOutcome::Success, Duration::from_millis(180))
            .await
            .unwrap();

        let overview = get_overview(State(state)).await.0;
        assert_eq!(overview.nodes.len(), 1);
        assert_eq!(overview.stats.total_nodes, 1);
        assert_eq!(overview.history.len(), 60);
    }

    #[tokio::test]
    async fn test_get_node_metrics_returns_history() {
        let state = create_state().await;

        let response = state
            .registry
            .register(RegisterRequest {
                machine_name: "metrics-node".into(),
                ip_address: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 31)),
                runtime_version: "0.1.0".into(),
                runtime_port: 32768,
                gpu_available: true,
                gpu_devices: sample_gpu_devices(),
                gpu_count: Some(1),
                gpu_model: Some("Test GPU".to_string()),
                supported_runtimes: Vec::new(),
            })
            .await
            .unwrap();

        let node_id = response.node_id;
        state.registry.approve(node_id).await.unwrap();

        state
            .load_manager
            .record_metrics(MetricsUpdate {
                node_id,
                cpu_usage: 24.0,
                memory_usage: 45.0,
                gpu_usage: Some(35.0),
                gpu_memory_usage: Some(40.0),
                gpu_memory_total_mb: None,
                gpu_memory_used_mb: None,
                gpu_temperature: None,
                gpu_model_name: None,
                gpu_compute_capability: None,
                gpu_capability_score: None,
                active_requests: 1,
                average_response_time_ms: Some(110.0),
                initializing: false,
                ready_models: Some((0, 0)),
            })
            .await
            .unwrap();
        state
            .load_manager
            .record_metrics(MetricsUpdate {
                node_id,
                cpu_usage: 32.0,
                memory_usage: 40.0,
                gpu_usage: Some(28.0),
                gpu_memory_usage: Some(30.0),
                gpu_memory_total_mb: None,
                gpu_memory_used_mb: None,
                gpu_temperature: None,
                gpu_model_name: None,
                gpu_compute_capability: None,
                gpu_capability_score: None,
                active_requests: 0,
                average_response_time_ms: Some(95.0),
                initializing: false,
                ready_models: Some((0, 0)),
            })
            .await
            .unwrap();

        let metrics = get_node_metrics(Path(node_id), State(state))
            .await
            .unwrap()
            .0;
        assert_eq!(metrics.len(), 2);
        assert_eq!(metrics[0].node_id, node_id);
        assert!(metrics[1].timestamp >= metrics[0].timestamp);
        assert_eq!(metrics[0].gpu_usage, Some(35.0));
        assert_eq!(metrics[1].gpu_memory_usage, Some(30.0));
    }

    #[test]
    fn test_request_history_query_default_values() {
        let query: RequestHistoryQuery = serde_urlencoded::from_str("").unwrap();
        assert_eq!(query.page, 1);
        assert_eq!(query.per_page, DEFAULT_PAGE_SIZE);
        assert_eq!(query.per_page, 10);
    }

    #[test]
    fn test_request_history_query_allowed_page_sizes() {
        // 許可されたページサイズ: 10, 25, 50, 100
        assert_eq!(ALLOWED_PAGE_SIZES, &[10, 25, 50, 100]);
    }

    #[test]
    fn test_request_history_query_normalized_per_page_valid() {
        for &size in ALLOWED_PAGE_SIZES {
            let query = RequestHistoryQuery {
                page: 1,
                per_page: size,
            };
            assert_eq!(query.normalized_per_page(), size);
        }
    }

    #[test]
    fn test_request_history_query_normalized_per_page_invalid() {
        // 無効な値はデフォルト(10)に正規化される
        let invalid_sizes = [0, 5, 15, 30, 99, 101, 200];
        for size in invalid_sizes {
            let query = RequestHistoryQuery {
                page: 1,
                per_page: size,
            };
            assert_eq!(query.normalized_per_page(), DEFAULT_PAGE_SIZE);
        }
    }

    #[test]
    fn test_request_history_query_parse_from_url() {
        // ページサイズ25を指定
        let query: RequestHistoryQuery = serde_urlencoded::from_str("page=2&per_page=25").unwrap();
        assert_eq!(query.page, 2);
        assert_eq!(query.per_page, 25);
        assert_eq!(query.normalized_per_page(), 25);

        // ページサイズ50を指定
        let query: RequestHistoryQuery = serde_urlencoded::from_str("per_page=50").unwrap();
        assert_eq!(query.page, 1);
        assert_eq!(query.per_page, 50);

        // ページサイズ100を指定
        let query: RequestHistoryQuery = serde_urlencoded::from_str("per_page=100").unwrap();
        assert_eq!(query.per_page, 100);
    }

    // T-8: DashboardNodeトークンフィールド応答テスト
    #[tokio::test]
    async fn test_dashboard_node_has_token_statistics_fields() {
        let state = create_state().await;

        // ノードを登録
        let register_req = RegisterRequest {
            machine_name: "token-node".into(),
            ip_address: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 50)),
            runtime_version: "0.1.0".into(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        };
        let node_id = state.registry.register(register_req).await.unwrap().node_id;
        state.registry.approve(node_id).await.unwrap();

        // メトリクスを記録してOnline状態にする
        state
            .load_manager
            .record_metrics(MetricsUpdate {
                node_id,
                cpu_usage: 30.0,
                memory_usage: 40.0,
                gpu_usage: Some(50.0),
                gpu_memory_usage: Some(60.0),
                gpu_memory_total_mb: None,
                gpu_memory_used_mb: None,
                gpu_temperature: None,
                gpu_model_name: None,
                gpu_compute_capability: None,
                gpu_capability_score: None,
                active_requests: 0,
                average_response_time_ms: Some(100.0),
                initializing: false,
                ready_models: Some((0, 0)),
            })
            .await
            .unwrap();

        // トークン使用量を記録するリクエストを完了
        use crate::token::TokenUsage;
        let token_usage = TokenUsage::new(Some(100), Some(50), Some(150));
        state.load_manager.begin_request(node_id).await.unwrap();
        state
            .load_manager
            .finish_request_with_tokens(
                node_id,
                RequestOutcome::Success,
                Duration::from_millis(100),
                Some(token_usage),
            )
            .await
            .unwrap();

        // DashboardNodeを取得
        let response = get_nodes(State(state.clone())).await;
        let nodes = response.0;
        assert_eq!(nodes.len(), 1);

        let node = &nodes[0];
        // T-8: トークン統計フィールドが存在し、正しい値を持つことを確認
        assert_eq!(node.total_input_tokens, 100);
        assert_eq!(node.total_output_tokens, 50);
        assert_eq!(node.total_tokens, 150);
    }

    // T-9: DashboardStatsトークンフィールド応答テスト
    #[tokio::test]
    async fn test_dashboard_stats_has_token_statistics_fields() {
        let state = create_state().await;

        // 2つのノードを登録
        let node1_id = state
            .registry
            .register(RegisterRequest {
                machine_name: "stats-node-1".into(),
                ip_address: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 51)),
                runtime_version: "0.1.0".into(),
                runtime_port: 32768,
                gpu_available: true,
                gpu_devices: sample_gpu_devices(),
                gpu_count: Some(1),
                gpu_model: Some("Test GPU".to_string()),
                supported_runtimes: Vec::new(),
            })
            .await
            .unwrap()
            .node_id;
        state.registry.approve(node1_id).await.unwrap();

        let node2_id = state
            .registry
            .register(RegisterRequest {
                machine_name: "stats-node-2".into(),
                ip_address: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 52)),
                runtime_version: "0.1.0".into(),
                runtime_port: 32768,
                gpu_available: true,
                gpu_devices: sample_gpu_devices(),
                gpu_count: Some(1),
                gpu_model: Some("Test GPU".to_string()),
                supported_runtimes: Vec::new(),
            })
            .await
            .unwrap()
            .node_id;
        state.registry.approve(node2_id).await.unwrap();

        // 両ノードをOnline状態にする
        for node_id in [node1_id, node2_id] {
            state
                .load_manager
                .record_metrics(MetricsUpdate {
                    node_id,
                    cpu_usage: 30.0,
                    memory_usage: 40.0,
                    gpu_usage: None,
                    gpu_memory_usage: None,
                    gpu_memory_total_mb: None,
                    gpu_memory_used_mb: None,
                    gpu_temperature: None,
                    gpu_model_name: None,
                    gpu_compute_capability: None,
                    gpu_capability_score: None,
                    active_requests: 0,
                    average_response_time_ms: None,
                    initializing: false,
                    ready_models: Some((0, 0)),
                })
                .await
                .unwrap();
        }

        // ノード1: 100入力 + 50出力
        use crate::token::TokenUsage;
        state.load_manager.begin_request(node1_id).await.unwrap();
        state
            .load_manager
            .finish_request_with_tokens(
                node1_id,
                RequestOutcome::Success,
                Duration::from_millis(100),
                Some(TokenUsage::new(Some(100), Some(50), Some(150))),
            )
            .await
            .unwrap();

        // ノード2: 200入力 + 100出力
        state.load_manager.begin_request(node2_id).await.unwrap();
        state
            .load_manager
            .finish_request_with_tokens(
                node2_id,
                RequestOutcome::Success,
                Duration::from_millis(100),
                Some(TokenUsage::new(Some(200), Some(100), Some(300))),
            )
            .await
            .unwrap();

        // DashboardStatsを取得
        let stats = get_stats(State(state)).await.0;

        // T-9: トークン統計フィールドが存在し、集計値が正しいことを確認
        assert_eq!(stats.total_input_tokens, 300); // 100 + 200
        assert_eq!(stats.total_output_tokens, 150); // 50 + 100
        assert_eq!(stats.total_tokens, 450); // 150 + 300
    }
}
