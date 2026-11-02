//! エンドポイント単位のリクエスト統計とモデル別 TPS

use crate::api::error::AppError;
use crate::auth::scope::Scope;
use crate::db::request_history::RequestHistoryStorage;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// GET /api/endpoints/{id}/today-stats - 当日リクエスト統計
///
/// SPEC-8c32349f: エンドポイント単位リクエスト統計 (Phase 5)
pub async fn get_endpoint_today_stats(
    Extension(scope): Extension<Scope>,
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<crate::db::endpoint_daily_stats::DailyStatEntry>, AppError> {
    RequestHistoryStorage::require_read_scope(&scope)?;
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let stats = state
        .balancer
        .request_history
        .scoped(&scope)
        .scoped_endpoint_days(id, 0)
        .await
        .map_err(AppError)?
        .into_iter()
        .find(|row| row.date == today)
        .unwrap_or(crate::db::endpoint_daily_stats::DailyStatEntry {
            date: today,
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
        });
    Ok(Json(stats))
}

/// GET /api/endpoints/{id}/daily-stats - 日次リクエスト統計
///
/// SPEC-8c32349f: エンドポイント単位リクエスト統計 (Phase 6)
pub async fn get_endpoint_daily_stats(
    Extension(scope): Extension<Scope>,
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
    Query(query): Query<EndpointDailyStatsQuery>,
) -> Result<Json<Vec<crate::db::endpoint_daily_stats::DailyStatEntry>>, AppError> {
    let days = query.days.unwrap_or(7).min(365);
    RequestHistoryStorage::require_read_scope(&scope)?;
    let stats = state
        .balancer
        .request_history
        .scoped(&scope)
        .scoped_endpoint_days(id, days)
        .await
        .map_err(AppError)?;
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
/// SPEC-8c32349f: エンドポイント単位リクエスト統計 (Phase 7)
pub async fn get_endpoint_model_stats(
    Extension(scope): Extension<Scope>,
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Vec<crate::db::endpoint_daily_stats::ModelStatEntry>>, AppError> {
    RequestHistoryStorage::require_read_scope(&scope)?;
    let stats = state
        .balancer
        .request_history
        .scoped(&scope)
        .scoped_model_stats(Some(id))
        .await
        .map_err(AppError)?;
    Ok(Json(stats))
}

/// GET /api/dashboard/all-model-stats - 全エンドポイント横断のモデル別統計
///
/// SPEC-8c32349f: ダッシュボード向けモデル別集計
pub async fn get_all_model_stats(
    Extension(scope): Extension<Scope>,
    State(state): State<AppState>,
) -> Result<Json<Vec<crate::db::endpoint_daily_stats::ModelStatEntry>>, AppError> {
    RequestHistoryStorage::require_read_scope(&scope)?;
    let stats = state
        .balancer
        .request_history
        .scoped(&scope)
        .scoped_model_stats(None)
        .await
        .map_err(AppError)?;
    Ok(Json(stats))
}

/// エンドポイント×モデル単位のTPS情報（SPEC-4bb5b55f）
#[derive(Debug, Clone, Serialize)]
pub struct ModelTpsEntry {
    /// モデルID
    pub model_id: String,
    /// API種別（chat/completions/responses）
    pub api_kind: crate::common::protocol::TpsApiKind,
    /// 計測元（production / benchmark）
    pub source: crate::common::protocol::TpsSource,
    /// EMA平滑化されたTPS値（None=未計測）
    pub tps: Option<f64>,
    /// リクエスト完了数
    pub request_count: u64,
    /// 出力トークン累計
    pub total_output_tokens: u64,
    /// 平均処理時間（ミリ秒、None=未計測）
    pub average_duration_ms: Option<f64>,
}

/// GET /api/endpoints/{id}/model-tps - エンドポイント×モデル単位のTPS情報
///
/// SPEC-4bb5b55f: エンドポイント×モデル単位TPS可視化 (Phase 3)
pub async fn get_endpoint_model_tps(
    Extension(scope): Extension<Scope>,
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Vec<ModelTpsEntry>>, AppError> {
    RequestHistoryStorage::require_read_scope(&scope)?;
    if !matches!(scope, Scope::Global) {
        return Ok(Json(vec![]));
    }
    let tps_list = state.balancer.load_manager.get_model_tps(id).await;
    Ok(Json(
        tps_list
            .into_iter()
            .map(|info| ModelTpsEntry {
                model_id: info.model_id,
                api_kind: info.api_kind,
                source: info.source,
                tps: info.tps,
                request_count: info.request_count,
                total_output_tokens: info.total_output_tokens,
                average_duration_ms: info.average_duration_ms,
            })
            .collect(),
    ))
}
