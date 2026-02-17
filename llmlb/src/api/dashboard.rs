//! ダッシュボードAPIハンドラー
//!
//! `/api/dashboard/*` 系のエンドポイントを提供し、ノードの状態および
//! システム統計を返却する。

use super::error::AppError;
use crate::common::{
    error::{CommonError, LbError},
    types::HealthMetrics,
};
use crate::db::request_history::{FilterStatus, RecordFilter};
use crate::{
    balancer::RequestHistoryPoint,
    types::endpoint::{EndpointStatus, EndpointType},
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
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::warn;
use uuid::Uuid;

/// エンドポイントのダッシュボード表示用サマリー
///
/// SPEC-66555000: llmlb主導エンドポイント登録システム
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DashboardEndpoint {
    /// エンドポイントID
    pub id: Uuid,
    /// 表示名
    pub name: String,
    /// ベースURL
    pub base_url: String,
    /// 現在の状態
    pub status: EndpointStatus,
    /// エンドポイントタイプ（xLLM/Ollama/vLLM 等）
    pub endpoint_type: EndpointType,
    /// ヘルスチェック間隔（秒）
    pub health_check_interval_secs: u32,
    /// 推論タイムアウト（秒）
    pub inference_timeout_secs: u32,
    /// レイテンシ（ミリ秒）
    pub latency_ms: Option<u32>,
    /// 最終確認時刻
    pub last_seen: Option<DateTime<Utc>>,
    /// 最後のエラーメッセージ
    pub last_error: Option<String>,
    /// 連続エラー回数
    pub error_count: u32,
    /// 登録日時
    pub registered_at: DateTime<Utc>,
    /// メモ
    pub notes: Option<String>,
    /// 利用可能なモデル数
    pub model_count: usize,
    /// 累計リクエスト数
    pub total_requests: i64,
    /// 成功リクエスト数
    pub successful_requests: i64,
    /// 失敗リクエスト数
    pub failed_requests: i64,
}

/// システム統計レスポンス
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DashboardStats {
    /// 登録ランタイム総数
    #[serde(rename = "total_runtimes", alias = "total_nodes")]
    pub total_nodes: usize,
    /// オンラインランタイム数
    #[serde(rename = "online_runtimes", alias = "online_nodes")]
    pub online_nodes: usize,
    /// 承認待ちランタイム数
    #[serde(rename = "pending_runtimes", alias = "pending_nodes")]
    pub pending_nodes: usize,
    /// 登録中ランタイム数
    #[serde(rename = "registering_runtimes", alias = "registering_nodes")]
    pub registering_nodes: usize,
    /// オフラインランタイム数
    #[serde(rename = "offline_runtimes", alias = "offline_nodes")]
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
    /// エンドポイント一覧（SPEC-66555000）
    pub endpoints: Vec<DashboardEndpoint>,
    /// システム統計
    pub stats: DashboardStats,
    /// リクエスト履歴
    pub history: Vec<RequestHistoryPoint>,
    /// レスポンス生成時刻
    pub generated_at: DateTime<Utc>,
    /// 集計に要した時間（ミリ秒）
    pub generation_time_ms: u64,
}

/// GET /api/dashboard/endpoints
///
/// SPEC-66555000: llmlb主導エンドポイント登録システム
pub async fn get_endpoints(State(state): State<AppState>) -> Json<Vec<DashboardEndpoint>> {
    Json(collect_endpoints(&state).await)
}

/// GET /api/dashboard/stats
pub async fn get_stats(State(state): State<AppState>) -> Json<DashboardStats> {
    Json(collect_stats(&state).await)
}

/// GET /api/dashboard/request-history
pub async fn get_request_history(State(state): State<AppState>) -> Json<Vec<RequestHistoryPoint>> {
    Json(collect_history(&state).await)
}

/// GET /api/dashboard/overview
pub async fn get_overview(State(state): State<AppState>) -> Json<DashboardOverview> {
    let started = Instant::now();
    let endpoints = collect_endpoints(&state).await;
    let stats = collect_stats(&state).await;
    let history = collect_history(&state).await;
    let generation_time_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    let generated_at = Utc::now();
    Json(DashboardOverview {
        endpoints,
        stats,
        history,
        generated_at,
        generation_time_ms,
    })
}

/// GET /api/dashboard/metrics/:runtime_id
pub async fn get_node_metrics(
    Path(node_id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Vec<HealthMetrics>>, AppError> {
    let history = state.load_manager.metrics_history(node_id).await?;
    Ok(Json(history))
}

/// GET /api/dashboard/stats/tokens - トークン統計取得
pub async fn get_token_stats(
    State(state): State<AppState>,
) -> Result<Json<crate::db::request_history::TokenStatistics>, AppError> {
    let stats = state
        .request_history
        .get_token_statistics()
        .await
        .map_err(AppError::from)?;
    Ok(Json(stats))
}

/// 日次トークン統計クエリパラメータ
#[derive(Debug, Clone, Deserialize)]
pub struct DailyTokenStatsQuery {
    /// 取得する日数（デフォルト: 30）
    #[serde(default = "default_days")]
    pub days: Option<u32>,
}

fn default_days() -> Option<u32> {
    Some(30)
}

/// 日次トークン統計レスポンス
#[derive(Debug, Clone, Serialize)]
pub struct DailyTokenStats {
    /// 日付（YYYY-MM-DD形式）
    pub date: String,
    /// 入力トークン合計
    pub total_input_tokens: u64,
    /// 出力トークン合計
    pub total_output_tokens: u64,
    /// 総トークン合計
    pub total_tokens: u64,
    /// リクエスト数
    pub request_count: u64,
}

/// GET /api/dashboard/stats/tokens/daily - 日次トークン統計取得
pub async fn get_daily_token_stats(
    State(state): State<AppState>,
    Query(query): Query<DailyTokenStatsQuery>,
) -> Result<Json<Vec<DailyTokenStats>>, AppError> {
    let days = query.days.unwrap_or(30);
    let stats = state
        .request_history
        .get_daily_token_statistics(days)
        .await
        .map_err(AppError::from)?;
    Ok(Json(stats))
}

/// 月次トークン統計クエリパラメータ
#[derive(Debug, Clone, Deserialize)]
pub struct MonthlyTokenStatsQuery {
    /// 取得する月数（デフォルト: 12）
    #[serde(default = "default_months")]
    pub months: Option<u32>,
}

fn default_months() -> Option<u32> {
    Some(12)
}

/// 月次トークン統計レスポンス
#[derive(Debug, Clone, Serialize)]
pub struct MonthlyTokenStats {
    /// 月（YYYY-MM形式）
    pub month: String,
    /// 入力トークン合計
    pub total_input_tokens: u64,
    /// 出力トークン合計
    pub total_output_tokens: u64,
    /// 総トークン合計
    pub total_tokens: u64,
    /// リクエスト数
    pub request_count: u64,
}

/// GET /api/dashboard/stats/tokens/monthly - 月次トークン統計取得
pub async fn get_monthly_token_stats(
    State(state): State<AppState>,
    Query(query): Query<MonthlyTokenStatsQuery>,
) -> Result<Json<Vec<MonthlyTokenStats>>, AppError> {
    let months = query.months.unwrap_or(12);
    let stats = state
        .request_history
        .get_monthly_token_statistics(months)
        .await
        .map_err(AppError::from)?;
    Ok(Json(stats))
}

/// エンドポイント一覧を収集
///
/// SPEC-66555000: llmlb主導エンドポイント登録システム
async fn collect_endpoints(state: &AppState) -> Vec<DashboardEndpoint> {
    let endpoint_registry = &state.endpoint_registry;
    let endpoints = endpoint_registry.list().await;

    let mut result = Vec::with_capacity(endpoints.len());
    for endpoint in endpoints {
        let model_count = endpoint_registry
            .list_models(endpoint.id)
            .await
            .map(|models| models.len())
            .unwrap_or(0);
        result.push(DashboardEndpoint {
            id: endpoint.id,
            name: endpoint.name,
            base_url: endpoint.base_url,
            status: endpoint.status,
            endpoint_type: endpoint.endpoint_type,
            health_check_interval_secs: endpoint.health_check_interval_secs,
            inference_timeout_secs: endpoint.inference_timeout_secs,
            latency_ms: endpoint.latency_ms,
            last_seen: endpoint.last_seen,
            last_error: endpoint.last_error,
            error_count: endpoint.error_count,
            registered_at: endpoint.registered_at,
            notes: endpoint.notes,
            model_count,
            total_requests: endpoint.total_requests,
            successful_requests: endpoint.successful_requests,
            failed_requests: endpoint.failed_requests,
        });
    }

    result
}

async fn collect_stats(state: &AppState) -> DashboardStats {
    let load_manager = state.load_manager.clone();

    let summary = load_manager.summary().await;
    let endpoints = state.endpoint_registry.list().await;

    let last_registered_at = endpoints.iter().map(|e| e.registered_at).max();
    let last_seen_at = endpoints.iter().filter_map(|e| e.last_seen).max();

    let openai_key_present = std::env::var("OPENAI_API_KEY").is_ok();
    let google_key_present = std::env::var("GOOGLE_API_KEY").is_ok();
    let anthropic_key_present = std::env::var("ANTHROPIC_API_KEY").is_ok();

    // Token totals must be consistent with the persisted request history.
    // The dashboard "Statistics" tab queries request_history directly, so prefer the same source
    // here to avoid "Total Tokens" mismatching after restarts / retention cleanup.
    let mut total_input_tokens = summary.total_input_tokens;
    let mut total_output_tokens = summary.total_output_tokens;
    let mut total_tokens = summary.total_tokens;
    match state.request_history.get_token_statistics().await {
        Ok(stats) => {
            total_input_tokens = stats.total_input_tokens;
            total_output_tokens = stats.total_output_tokens;
            total_tokens = stats.total_tokens;
        }
        Err(e) => {
            warn!(
                "Failed to query token statistics from request history: {}",
                e
            );
        }
    }

    let mut total_requests = summary.total_requests;
    let mut successful_requests = summary.successful_requests;
    let mut failed_requests = summary.failed_requests;
    let to_u64 = |value: i64| -> u64 {
        if value < 0 {
            0
        } else {
            value as u64
        }
    };
    match crate::db::endpoints::get_request_totals(&state.db_pool).await {
        Ok(request_totals) => {
            total_requests = to_u64(request_totals.total_requests);
            successful_requests = to_u64(request_totals.successful_requests);
            failed_requests = to_u64(request_totals.failed_requests);
        }
        Err(e) => {
            warn!("Failed to query persisted request totals: {}", e);
        }
    }

    DashboardStats {
        total_nodes: summary.total_nodes,
        online_nodes: summary.online_nodes,
        pending_nodes: summary.pending_nodes,
        registering_nodes: summary.registering_nodes,
        offline_nodes: summary.offline_nodes,
        total_requests,
        successful_requests,
        failed_requests,
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
        total_input_tokens,
        total_output_tokens,
        total_tokens,
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
    /// 1ページあたりの件数（互換: limit）
    #[serde(default)]
    pub limit: Option<usize>,
    /// オフセット（互換: offset）
    #[serde(default)]
    pub offset: Option<usize>,
    /// モデル名フィルタ（部分一致）
    pub model: Option<String>,
    /// ノードIDフィルタ
    #[serde(alias = "agent_id")]
    pub node_id: Option<Uuid>,
    /// ステータスフィルタ
    pub status: Option<FilterStatus>,
    /// 開始時刻フィルタ（RFC3339）
    pub start_time: Option<DateTime<Utc>>,
    /// 終了時刻フィルタ（RFC3339）
    pub end_time: Option<DateTime<Utc>>,
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

    fn to_record_filter(&self) -> Result<RecordFilter, AppError> {
        if let (Some(start), Some(end)) = (&self.start_time, &self.end_time) {
            if start > end {
                return Err(AppError(LbError::Common(CommonError::Validation(
                    "start_time must be <= end_time".to_string(),
                ))));
            }
        }

        Ok(RecordFilter {
            model: self.model.clone(),
            node_id: self.node_id,
            status: self.status,
            start_time: self.start_time,
            end_time: self.end_time,
        })
    }
}

/// エクスポート形式
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum RequestHistoryExportFormat {
    /// CSV形式
    #[default]
    Csv,
    /// JSON形式
    Json,
}

/// リクエスト履歴エクスポート用のクエリパラメータ
#[derive(Debug, Clone, Deserialize)]
pub struct RequestHistoryExportQuery {
    /// エクスポート形式（csv/json）
    #[serde(default)]
    pub format: RequestHistoryExportFormat,
    /// モデル名フィルタ（部分一致）
    pub model: Option<String>,
    /// ノードIDフィルタ
    #[serde(alias = "agent_id")]
    pub node_id: Option<Uuid>,
    /// ステータスフィルタ
    pub status: Option<FilterStatus>,
    /// 開始時刻フィルタ（RFC3339）
    pub start_time: Option<DateTime<Utc>>,
    /// 終了時刻フィルタ（RFC3339）
    pub end_time: Option<DateTime<Utc>>,
}

impl RequestHistoryExportQuery {
    fn to_record_filter(&self) -> Result<RecordFilter, AppError> {
        if let (Some(start), Some(end)) = (&self.start_time, &self.end_time) {
            if start > end {
                return Err(AppError(LbError::Common(CommonError::Validation(
                    "start_time must be <= end_time".to_string(),
                ))));
            }
        }

        Ok(RecordFilter {
            model: self.model.clone(),
            node_id: self.node_id,
            status: self.status,
            start_time: self.start_time,
            end_time: self.end_time,
        })
    }
}

/// T023: リクエスト履歴一覧API
pub async fn list_request_responses(
    State(state): State<AppState>,
    Query(query): Query<RequestHistoryQuery>,
) -> Result<Json<crate::db::request_history::FilteredRecords>, AppError> {
    let filter = query.to_record_filter()?;
    let mut page = if query.page == 0 { 1 } else { query.page };
    let mut per_page = query.normalized_per_page();

    if let Some(limit) = query.limit {
        per_page = if ALLOWED_PAGE_SIZES.contains(&limit) {
            limit
        } else {
            DEFAULT_PAGE_SIZE
        };
    }

    if let Some(offset) = query.offset {
        if per_page == 0 {
            per_page = DEFAULT_PAGE_SIZE;
        }
        page = offset / per_page + 1;
    }
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
) -> Result<Json<crate::common::protocol::RequestResponseRecord>, AppError> {
    let record = state
        .request_history
        .get_record_by_id(id)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| {
            crate::common::error::LbError::NotFound(format!("Record {} not found", id))
        })?;
    Ok(Json(record))
}

/// T025: エクスポートAPI
pub async fn export_request_responses(
    State(state): State<AppState>,
    Query(query): Query<RequestHistoryExportQuery>,
) -> Result<Response, AppError> {
    let filter = query.to_record_filter()?;
    const EXPORT_PAGE_SIZE: usize = 1000;

    let first_page = state
        .request_history
        .filter_and_paginate(&filter, 1, EXPORT_PAGE_SIZE)
        .await
        .map_err(AppError::from)?;

    match query.format {
        RequestHistoryExportFormat::Json => {
            let mut page = 1usize;
            let mut page_data = Some(first_page.clone());
            let mut body = Vec::new();
            body.push(b'[');
            let mut first = true;

            loop {
                let data = if let Some(data) = page_data.take() {
                    data
                } else {
                    state
                        .request_history
                        .filter_and_paginate(&filter, page, EXPORT_PAGE_SIZE)
                        .await
                        .map_err(|err| {
                            warn!("Failed to export request history page {}: {}", page, err);
                            AppError::from(err)
                        })?
                };

                if data.records.is_empty() {
                    break;
                }

                for record in data.records {
                    let json = serde_json::to_vec(&record).map_err(|err| {
                        warn!("Failed to serialize request history record: {}", err);
                        AppError::from(LbError::Internal(format!(
                            "Failed to serialize request history record: {}",
                            err
                        )))
                    })?;
                    if !first {
                        body.push(b',');
                    }
                    first = false;
                    body.extend_from_slice(&json);
                }

                if page * EXPORT_PAGE_SIZE >= data.total_count {
                    break;
                }
                page += 1;
            }

            body.push(b']');

            let response = Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header(
                    "Content-Disposition",
                    "attachment; filename=\"request_history.json\"",
                )
                .body(Body::from(body))
                .unwrap();
            Ok(response)
        }
        RequestHistoryExportFormat::Csv => {
            let mut page = 1usize;
            let mut page_data = Some(first_page.clone());
            let mut csv_writer = csv::Writer::from_writer(vec![]);
            csv_writer
                .write_record([
                    "id",
                    "timestamp",
                    "request_type",
                    "model",
                    "runtime_id",
                    "runtime_machine_name",
                    "runtime_ip",
                    "client_ip",
                    "duration_ms",
                    "status",
                    "completed_at",
                ])
                .map_err(|err| {
                    AppError::from(LbError::Internal(format!(
                        "Failed to write CSV header: {}",
                        err
                    )))
                })?;

            loop {
                let data = if let Some(data) = page_data.take() {
                    data
                } else {
                    state
                        .request_history
                        .filter_and_paginate(&filter, page, EXPORT_PAGE_SIZE)
                        .await
                        .map_err(|err| {
                            warn!("Failed to export request history page {}: {}", page, err);
                            AppError::from(err)
                        })?
                };

                if data.records.is_empty() {
                    break;
                }

                for record in data.records {
                    let status_str = match &record.status {
                        crate::common::protocol::RecordStatus::Success => "success".to_string(),
                        crate::common::protocol::RecordStatus::Error { message } => {
                            format!("error: {}", message)
                        }
                    };

                    csv_writer
                        .write_record(&[
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
                        .map_err(|err| {
                            AppError::from(LbError::Internal(format!(
                                "Failed to write CSV row: {}",
                                err
                            )))
                        })?;
                }

                if page * EXPORT_PAGE_SIZE >= data.total_count {
                    break;
                }
                page += 1;
            }

            let body = csv_writer.into_inner().map_err(|err| {
                AppError::from(LbError::Internal(format!(
                    "Failed to finalize CSV export: {}",
                    err
                )))
            })?;

            let response = Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "text/csv")
                .header(
                    "Content-Disposition",
                    "attachment; filename=\"request_history.csv\"",
                )
                .body(Body::from(body))
                .unwrap();

            Ok(response)
        }
    }
}

/// GET /api/endpoints/{id}/today-stats - 当日リクエスト統計
///
/// SPEC-76643000: エンドポイント単位リクエスト統計 (Phase 5)
pub async fn get_endpoint_today_stats(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<crate::db::endpoint_daily_stats::DailyStatEntry>, AppError> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let stats = crate::db::endpoint_daily_stats::get_today_stats(&state.db_pool, id, &today)
        .await
        .map_err(|e| AppError(crate::common::error::LbError::Database(e.to_string())))?;
    Ok(Json(stats))
}

/// GET /api/endpoints/{id}/daily-stats - 日次リクエスト統計
///
/// SPEC-76643000: エンドポイント単位リクエスト統計 (Phase 6)
pub async fn get_endpoint_daily_stats(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
    Query(query): Query<EndpointDailyStatsQuery>,
) -> Result<Json<Vec<crate::db::endpoint_daily_stats::DailyStatEntry>>, AppError> {
    let days = query.days.unwrap_or(7).min(365);
    let stats = crate::db::endpoint_daily_stats::get_daily_stats(&state.db_pool, id, days)
        .await
        .map_err(|e| AppError(crate::common::error::LbError::Database(e.to_string())))?;
    Ok(Json(stats))
}

/// エンドポイント日次統計クエリパラメータ
#[derive(Debug, Clone, Deserialize)]
pub struct EndpointDailyStatsQuery {
    /// 取得する日数（デフォルト: 7、最大: 365）
    #[serde(default)]
    pub days: Option<u32>,
}

/// GET /api/endpoints/{id}/model-stats - モデル別リクエスト統計
///
/// SPEC-76643000: エンドポイント単位リクエスト統計 (Phase 7)
pub async fn get_endpoint_model_stats(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Vec<crate::db::endpoint_daily_stats::ModelStatEntry>>, AppError> {
    let stats = crate::db::endpoint_daily_stats::get_model_stats(&state.db_pool, id)
        .await
        .map_err(|e| AppError(crate::common::error::LbError::Database(e.to_string())))?;
    Ok(Json(stats))
}

/// GET /api/dashboard/models - ダッシュボード向けモデル一覧
pub async fn get_models(State(state): State<AppState>) -> Result<Response, AppError> {
    crate::api::openai::list_models(State(state)).await
}

// NOTE: テストは NodeRegistry → EndpointRegistry 移行完了後に再実装
// 関連: SPEC-66555000
