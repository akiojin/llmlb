//! Durable organization/account usage, with SQL totals before page boundaries.

use super::{scope::database, AppError};
use crate::{
    auth::scope::UsageScope,
    common::error::{CommonError, LbError},
    db::request_history::scoped::scope_predicate,
    AppState,
};
use axum::{
    extract::{Query, State},
    Extension, Json,
};
use chrono::{Datelike, Days, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupBy {
    Organization,
    #[default]
    Account,
}

#[derive(Debug, Default, Deserialize)]
pub struct UsageQuery {
    #[serde(default)]
    pub group_by: GroupBy,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub organization_id: Option<Uuid>,
    pub account_id: Option<Uuid>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, sqlx::FromRow)]
pub struct UsageCounts {
    pub request_count: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub measured_requests: i64,
    pub estimated_requests: i64,
    pub missing_requests: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct UsageRow {
    pub organization_id: Option<String>,
    pub account_id: Option<String>,
    pub organization_name: Option<String>,
    pub account_name: Option<String>,
    pub organization_archived: bool,
    pub account_deleted: bool,
    #[serde(flatten)]
    #[sqlx(flatten)]
    pub counts: UsageCounts,
}

#[derive(Debug, Serialize)]
pub struct UsagePage {
    pub scope: &'static str,
    pub group_by: GroupBy,
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub to_exclusive: NaiveDate,
    pub rows: Vec<UsageRow>,
    pub totals: UsageCounts,
    pub total_count: i64,
    pub page: u32,
    pub per_page: u32,
    pub coverage_start: Option<String>,
}

fn validation(message: &str) -> AppError {
    AppError(LbError::Common(CommonError::Validation(message.into())))
}

const COUNTS: &str = "COALESCE(SUM(request_count),0) request_count,COALESCE(SUM(input_tokens),0) input_tokens,COALESCE(SUM(output_tokens),0) output_tokens,COALESCE(SUM(total_tokens),0) total_tokens,COALESCE(SUM(provider_count),0) measured_requests,COALESCE(SUM(estimated_count),0) estimated_requests,COALESCE(SUM(missing_count),0) missing_requests";

/// GET `/api/dashboard/usage`: durable totals and paginated authorized groups.
pub async fn get_usage(
    State(state): State<AppState>,
    Extension(scope): Extension<UsageScope>,
    Query(query): Query<UsageQuery>,
) -> Result<Json<UsagePage>, AppError> {
    let today = Utc::now().date_naive();
    let month_start = today.with_day(1).expect("valid month start");
    let next_month = if today.month() == 12 {
        NaiveDate::from_ymd_opt(today.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(today.year(), today.month() + 1, 1)
    }
    .ok_or_else(|| validation("Invalid month"))?;
    let from = query.from.unwrap_or(month_start);
    let to = query
        .to
        .unwrap_or(next_month.pred_opt().expect("valid previous day"));
    let to_exclusive = to
        .checked_add_days(Days::new(1))
        .ok_or_else(|| validation("Invalid end date"))?;
    if from > to {
        return Err(validation("from must be <= to"));
    }
    let page = query.page.unwrap_or(1).clamp(1, 1_000_000);
    let per_page = query.per_page.unwrap_or(10).clamp(1, 100);
    match &scope {
        UsageScope::Global => {}
        UsageScope::Organization(id) => {
            if query.organization_id.is_some_and(|filter| filter != *id) {
                return Err(AppError(LbError::Authorization(
                    "Requested organization is outside your scope".into(),
                )));
            }
        }
        UsageScope::Account {
            organization_id,
            account_id,
        } => {
            if query
                .organization_id
                .is_some_and(|filter| filter != *organization_id)
                || query.account_id.is_some_and(|filter| filter != *account_id)
            {
                return Err(AppError(LbError::Authorization(
                    "Requested account is outside your scope".into(),
                )));
            }
        }
    }
    let mut predicate = scope_predicate(&scope);
    if let Some(id) = query.organization_id {
        predicate.push_str(&format!(" AND organization_id = '{id}'"));
    }
    if let Some(id) = query.account_id {
        predicate.push_str(&format!(" AND account_id = '{id}'"));
    }
    let source = format!("FROM usage_daily WHERE ({predicate}) AND date >= ? AND date < ?");
    // Terminal usage writes may commit while this page is read. Keep totals,
    // group count, rows and coverage on the same read snapshot.
    let mut snapshot = state.db_pool.begin().await.map_err(database)?;
    let totals: UsageCounts = sqlx::query_as(&format!("SELECT {COUNTS} {source}"))
        .bind(from.to_string())
        .bind(to_exclusive.to_string())
        .fetch_one(&mut *snapshot)
        .await
        .map_err(database)?;
    let grouping = match query.group_by {
        GroupBy::Organization => "organization_id",
        GroupBy::Account => "organization_id,account_id",
    };
    let total_count: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM (SELECT 1 {source} GROUP BY {grouping})"
    ))
    .bind(from.to_string())
    .bind(to_exclusive.to_string())
    .fetch_one(&mut *snapshot)
    .await
    .map_err(database)?;
    let account = match query.group_by {
        GroupBy::Organization => "NULL",
        GroupBy::Account => "NULLIF(account_id,'')",
    };
    let grouped = format!("SELECT NULLIF(organization_id,'') organization_id,{account} account_id,{COUNTS} {source} GROUP BY {grouping}");
    // Historical usage authorizes the immutable account identifier, not private
    // profile changes made after that account moved to another organization.
    // Keep the identity join so a former account is not mislabeled as deleted.
    let account_name = if scope == UsageScope::Global {
        "u.username"
    } else {
        "CASE WHEN u.organization_id=g.organization_id THEN u.username ELSE NULL END"
    };
    let sql = format!("SELECT g.*,o.name organization_name,{account_name} account_name,(g.organization_id IS NOT NULL AND (o.id IS NULL OR o.archived_at IS NOT NULL)) organization_archived,(g.account_id IS NOT NULL AND u.id IS NULL) account_deleted FROM ({grouped}) g LEFT JOIN organizations o ON o.id=g.organization_id LEFT JOIN users u ON u.id=g.account_id ORDER BY COALESCE(g.organization_id,''),COALESCE(g.account_id,'') LIMIT ? OFFSET ?");
    let rows = sqlx::query_as::<_, UsageRow>(&sql)
        .bind(from.to_string())
        .bind(to_exclusive.to_string())
        .bind(i64::from(per_page))
        .bind(i64::from(page - 1) * i64::from(per_page))
        .fetch_all(&mut *snapshot)
        .await
        .map_err(database)?;
    let coverage_start = sqlx::query_scalar::<_, String>(
        "SELECT value FROM settings WHERE key='usage_observation_started_at'",
    )
    .fetch_optional(&mut *snapshot)
    .await
    .map_err(database)?;
    snapshot.commit().await.map_err(database)?;
    Ok(Json(UsagePage {
        scope: scope.label(),
        group_by: query.group_by,
        from,
        to,
        to_exclusive,
        rows,
        totals,
        total_count,
        page,
        per_page,
        coverage_start,
    }))
}

/// Compatibility token charts use the same durable rollup as the usage screen.
pub(super) async fn token_totals(
    state: &AppState,
    scope: &UsageScope,
) -> Result<crate::db::request_history::TokenStatistics, AppError> {
    let totals = super::scope::totals(state, scope).await?;
    Ok(crate::db::request_history::TokenStatistics {
        total_input_tokens: totals.input_tokens as u64,
        total_output_tokens: totals.output_tokens as u64,
        total_tokens: totals.total_tokens as u64,
    })
}

pub(super) async fn daily_tokens(
    state: &AppState,
    scope: &UsageScope,
    days: u32,
) -> Result<Vec<super::DailyTokenStats>, AppError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        date: String,
        input: i64,
        output: i64,
        total: i64,
        requests: i64,
    }
    let sql = format!("SELECT date,SUM(input_tokens) input,SUM(output_tokens) output,SUM(total_tokens) total,SUM(request_count) requests FROM usage_daily WHERE {} AND date >= DATE('now','-' || ? || ' days') GROUP BY date ORDER BY date DESC",scope_predicate(scope));
    let rows: Vec<Row> = sqlx::query_as(&sql)
        .bind(i64::from(days.min(3660)))
        .fetch_all(&state.db_pool)
        .await
        .map_err(database)?;
    Ok(rows
        .into_iter()
        .map(|row| super::DailyTokenStats {
            date: row.date,
            total_input_tokens: row.input as u64,
            total_output_tokens: row.output as u64,
            total_tokens: row.total as u64,
            request_count: row.requests as u64,
        })
        .collect())
}

pub(super) async fn monthly_tokens(
    state: &AppState,
    scope: &UsageScope,
    months: u32,
) -> Result<Vec<super::MonthlyTokenStats>, AppError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        month: String,
        input: i64,
        output: i64,
        total: i64,
        requests: i64,
    }
    let sql = format!("SELECT substr(date,1,7) month,SUM(input_tokens) input,SUM(output_tokens) output,SUM(total_tokens) total,SUM(request_count) requests FROM usage_daily WHERE {} AND date >= DATE('now','start of month','-' || ? || ' months') GROUP BY substr(date,1,7) ORDER BY month DESC",scope_predicate(scope));
    let rows: Vec<Row> = sqlx::query_as(&sql)
        .bind(i64::from(months.min(1200).saturating_sub(1)))
        .fetch_all(&state.db_pool)
        .await
        .map_err(database)?;
    Ok(rows
        .into_iter()
        .map(|row| super::MonthlyTokenStats {
            month: row.month,
            total_input_tokens: row.input as u64,
            total_output_tokens: row.output as u64,
            total_tokens: row.total as u64,
            request_count: row.requests as u64,
        })
        .collect())
}
