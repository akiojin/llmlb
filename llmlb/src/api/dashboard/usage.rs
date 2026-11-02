//! UTC usage reporting for the current JWT authorization scope.

use crate::{
    api::{
        error::{AppError, HandlerError},
        organizations::scoped_headers,
    },
    auth::scope::Scope,
    common::error::{CommonError, LbError},
    db::usage::query::{self, GroupBy, UsageFilter, UsageReport},
    AppState,
};
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Extension, Json,
};
use chrono::{Datelike, Months, NaiveDate, Utc};
use serde::Deserialize;
use uuid::Uuid;

/// Client filters only narrow the authenticated server-selected scope.
#[derive(Default, Deserialize)]
pub struct UsageQuery {
    /// organization (default) or account.
    pub group_by: Option<String>,
    /// Inclusive UTC start date, YYYY-MM-DD.
    pub from: Option<String>,
    /// Inclusive UTC end date, YYYY-MM-DD.
    pub to: Option<String>,
    /// Original organization snapshot to narrow to.
    pub organization_id: Option<Uuid>,
    /// Original usage account snapshot to narrow to.
    pub account_id: Option<Uuid>,
    /// One-based page, default 1.
    pub page: Option<i64>,
    /// Page size, default 25 and maximum 1000.
    pub per_page: Option<i64>,
}

fn invalid(message: &str) -> AppError {
    AppError(LbError::Common(CommonError::Validation(message.into())))
}

fn parse_date(value: &str) -> Result<NaiveDate, AppError> {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| index != 4 && index != 7 && !byte.is_ascii_digit())
    {
        return Err(invalid("Usage dates must use YYYY-MM-DD"));
    }
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| invalid("Invalid UTC usage date"))?;
    if date.year() < 1 {
        return Err(invalid("Invalid UTC usage date"));
    }
    Ok(date)
}

/// GET /api/dashboard/usage. API keys never enter this JWT-only handler.
pub async fn get_usage(
    Extension(scope): Extension<Scope>,
    State(state): State<AppState>,
    Query(request): Query<UsageQuery>,
) -> Result<(HeaderMap, Json<UsageReport>), HandlerError> {
    scope.require_assigned()?;
    let group_by = match request.group_by.as_deref().unwrap_or("organization") {
        "organization" => GroupBy::Organization,
        "account" => GroupBy::Account,
        _ => return Err(invalid("group_by must be organization or account").into()),
    };
    let month_start = Utc::now()
        .date_naive()
        .with_day(1)
        .expect("first day of current UTC month");
    let next_month = month_start
        .checked_add_months(Months::new(1))
        .expect("current UTC month has an end");
    let from = request
        .from
        .as_deref()
        .map(parse_date)
        .transpose()?
        .unwrap_or(month_start);
    let to = request
        .to
        .as_deref()
        .map(parse_date)
        .transpose()?
        .unwrap_or_else(|| next_month.pred_opt().unwrap());
    let to_exclusive = to
        .succ_opt()
        .filter(|date| date.year() <= 9999)
        .ok_or_else(|| invalid("UTC usage end date is outside the supported range"))?;
    let page = request.page.unwrap_or(1);
    let per_page = request.per_page.unwrap_or(25);
    if from > to
        || page < 1
        || !(1..=1000).contains(&per_page)
        || (page - 1).checked_mul(per_page).is_none()
    {
        return Err(invalid("Invalid usage interval or pagination").into());
    }
    let filter = UsageFilter {
        group_by,
        from,
        to_exclusive,
        organization_id: request.organization_id,
        account_id: request.account_id,
        page,
        per_page,
    };
    let report = query::report(&state.db_pool, &scope, &filter)
        .await
        .map_err(AppError)?;
    Ok((scoped_headers(&scope), Json(report)))
}
