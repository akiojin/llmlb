//! ダッシュボードのトークン使用量統計（累計・日次・月次）

use crate::api::error::AppError;
use crate::auth::scope::UsageScope;
use crate::AppState;
use axum::{
    extract::{Query, State},
    Extension, Json,
};
use serde::{Deserialize, Serialize};

/// GET /api/dashboard/stats/tokens - トークン統計取得
///
/// 導入後の永続 usage_daily を全scopeで共有し、raw retentionに依存しない。
pub async fn get_token_stats(
    State(state): State<AppState>,
    Extension(scope): Extension<UsageScope>,
) -> Result<Json<crate::db::request_history::TokenStatistics>, AppError> {
    Ok(Json(super::usage::token_totals(&state, &scope).await?))
}

/// 日次トークン統計クエリパラメータ
#[derive(Debug, Clone, Deserialize)]
pub struct DailyTokenStatsQuery {
    /// 取得する日数（デフォルト: 30）
    #[serde(default = "default_days")]
    pub days: Option<u32>,
}

pub(super) fn default_days() -> Option<u32> {
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
///
/// 導入後の永続 usage_daily を全scopeで共有し、raw retentionに依存しない。
pub async fn get_daily_token_stats(
    State(state): State<AppState>,
    Query(query): Query<DailyTokenStatsQuery>,
    Extension(scope): Extension<UsageScope>,
) -> Result<Json<Vec<DailyTokenStats>>, AppError> {
    Ok(Json(
        super::usage::daily_tokens(&state, &scope, query.days.unwrap_or(30)).await?,
    ))
}

/// 月次トークン統計クエリパラメータ
#[derive(Debug, Clone, Deserialize)]
pub struct MonthlyTokenStatsQuery {
    /// 取得する月数（デフォルト: 12）
    #[serde(default = "default_months")]
    pub months: Option<u32>,
}

pub(super) fn default_months() -> Option<u32> {
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
///
/// 導入後の永続 usage_daily を全scopeで共有し、raw retentionに依存しない。
pub async fn get_monthly_token_stats(
    State(state): State<AppState>,
    Query(query): Query<MonthlyTokenStatsQuery>,
    Extension(scope): Extension<UsageScope>,
) -> Result<Json<Vec<MonthlyTokenStats>>, AppError> {
    Ok(Json(
        super::usage::monthly_tokens(&state, &scope, query.months.unwrap_or(12)).await?,
    ))
}
