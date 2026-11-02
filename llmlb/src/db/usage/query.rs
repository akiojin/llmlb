//! Scoped reporting over retained daily usage, independent of raw retention.

use crate::{
    auth::scope::Scope,
    common::error::{CommonError, LbError},
};
use chrono::NaiveDate;
use serde::Serialize;
use sqlx::{QueryBuilder, Sqlite, SqliteConnection, SqlitePool};
use uuid::Uuid;

/// Fixed grouping dimensions; an API-key issuer is never a usage dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupBy {
    /// One row per accepted organization snapshot.
    Organization,
    /// One row per accepted organization/account pair.
    Account,
}

/// Validated UTC date interval and optional narrowing filters.
pub struct UsageFilter {
    /// Reporting dimension.
    pub group_by: GroupBy,
    /// Inclusive UTC receipt date.
    pub from: NaiveDate,
    /// Exclusive UTC receipt date.
    pub to_exclusive: NaiveDate,
    /// Optional organization snapshot filter.
    pub organization_id: Option<Uuid>,
    /// Optional usage account snapshot filter.
    pub account_id: Option<Uuid>,
    /// One-based page.
    pub page: i64,
    /// Page size, from 1 through 1000.
    pub per_page: i64,
}

/// Tokens and field-level coverage. Missing fields contribute no measured value.
#[derive(Debug, Clone, Default, Serialize, sqlx::FromRow)]
pub struct UsageTotals {
    /// Accepted logical HTTP requests.
    pub request_count: i64,
    /// Sum of available input observations.
    pub input_tokens: i64,
    /// Sum of available output observations.
    pub output_tokens: i64,
    /// Sum of available total observations, without deriving missing totals.
    pub total_tokens: i64,
    /// Requests with provider observations, including partial observations.
    pub measured_requests: i64,
    /// Requests with locally estimated observations.
    pub estimated_requests: i64,
    /// Finalized requests with no usable observation.
    pub missing_requests: i64,
    /// Accepted requests awaiting their terminal observation.
    pub pending_requests: i64,
    /// Final observations without an input count.
    pub input_missing_count: i64,
    /// Final observations without an output count.
    pub output_missing_count: i64,
    /// Final observations without a total count.
    pub total_missing_count: i64,
}

/// A retained snapshot bucket with current labels only for display.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct UsageRow {
    /// Original organization, or null for legacy unattributed usage.
    pub organization_id: Option<String>,
    /// Current organization name, if the organization still exists.
    pub organization_name: Option<String>,
    /// Archived or no longer available organization identity.
    pub organization_archived: bool,
    /// Original account, present only in account grouping.
    pub account_id: Option<String>,
    /// Current account name, independent of current membership.
    pub account_name: Option<String>,
    /// The original account has since been deleted.
    pub account_deleted: bool,
    /// Aggregates for this group.
    #[serde(flatten)]
    #[sqlx(flatten)]
    pub totals: UsageTotals,
}

/// Authorized totals cover all filtered groups, even when the page is empty.
#[derive(Debug, Serialize)]
pub struct UsageReport {
    /// Applied grouping.
    pub group_by: GroupBy,
    /// Inclusive UTC UI start date.
    pub from: String,
    /// Inclusive UTC UI end date.
    pub to: String,
    /// Requested page.
    pub page: i64,
    /// Applied page size.
    pub per_page: i64,
    /// Number of authorized groups before pagination.
    pub total: i64,
    /// Stable page of authorized groups.
    pub rows: Vec<UsageRow>,
    /// All authorized groups in the filtered interval.
    pub totals: UsageTotals,
    /// Observation deployment boundary; no historical usage is guessed.
    pub coverage_start: String,
}

// All columns here are source constants; request values are bound parameters.
const SUM_COLUMNS: &str = "
    COALESCE(SUM(d.request_count), 0) AS request_count,
    COALESCE(SUM(d.input_tokens), 0) AS input_tokens,
    COALESCE(SUM(d.output_tokens), 0) AS output_tokens,
    COALESCE(SUM(d.total_tokens), 0) AS total_tokens,
    COALESCE(SUM(d.provider_count), 0) AS measured_requests,
    COALESCE(SUM(d.estimated_count), 0) AS estimated_requests,
    COALESCE(SUM(d.missing_count), 0) AS missing_requests,
    COALESCE(SUM(d.pending_requests), 0) AS pending_requests,
    COALESCE(SUM(d.input_missing_count), 0) AS input_missing_count,
    COALESCE(SUM(d.output_missing_count), 0) AS output_missing_count,
    COALESCE(SUM(d.total_missing_count), 0) AS total_missing_count";

fn database_error(error: sqlx::Error) -> LbError {
    LbError::Database(format!("Failed to read scoped usage: {error}"))
}

fn forbidden_filter() -> LbError {
    LbError::Authorization("The requested usage filter is outside the authenticated scope".into())
}

async fn validate_scope_filters(
    connection: &mut SqliteConnection,
    scope: &Scope,
    filter: &UsageFilter,
) -> Result<(), LbError> {
    match scope {
        Scope::Global => Ok(()),
        Scope::Unassigned(_) => Err(forbidden_filter()),
        Scope::SelfAccount {
            organization_id,
            account_id,
        } => {
            if filter
                .organization_id
                .is_some_and(|id| id != *organization_id)
                || filter.account_id.is_some_and(|id| id != *account_id)
            {
                Err(forbidden_filter())
            } else {
                Ok(())
            }
        }
        Scope::Organization(organization_id) => {
            if filter
                .organization_id
                .is_some_and(|id| id != *organization_id)
            {
                return Err(forbidden_filter());
            }
            if let Some(account_id) = filter.account_id {
                // Current members without usage and historical members with a
                // retained snapshot are valid narrowing targets. An unknown or
                // foreign ID gets the same refusal, without an existence oracle.
                let allowed: bool = sqlx::query_scalar(
                    "SELECT EXISTS(
                       SELECT 1 FROM users WHERE id = ? AND organization_id = ?
                       UNION ALL
                       SELECT 1 FROM usage_daily WHERE account_id = ? AND organization_id = ?
                     )",
                )
                .bind(account_id.to_string())
                .bind(organization_id.to_string())
                .bind(account_id.to_string())
                .bind(organization_id.to_string())
                .fetch_one(&mut *connection)
                .await
                .map_err(database_error)?;
                if !allowed {
                    return Err(forbidden_filter());
                }
            }
            Ok(())
        }
    }
}

fn push_predicate(query: &mut QueryBuilder<'_, Sqlite>, scope: &Scope, filter: &UsageFilter) {
    query.push(" WHERE (");
    scope.push_sql(query, "d.organization_id", "d.account_id");
    query
        .push(") AND d.date >= ")
        .push_bind(filter.from.to_string());
    query
        .push(" AND d.date < ")
        .push_bind(filter.to_exclusive.to_string());
    if let Some(organization_id) = filter.organization_id {
        query
            .push(" AND d.organization_id = ")
            .push_bind(organization_id.to_string());
    }
    if let Some(account_id) = filter.account_id {
        query
            .push(" AND d.account_id = ")
            .push_bind(account_id.to_string());
    }
}

fn push_grouping(query: &mut QueryBuilder<'_, Sqlite>, group_by: GroupBy) {
    query.push(" GROUP BY d.organization_id");
    if group_by == GroupBy::Account {
        query.push(", d.account_id");
    }
}

/// Read counts, all-group totals, labels and a page from one database snapshot.
///
/// Current identity tables provide labels only. Their membership columns never
/// reassign or filter historical daily buckets. Every aggregate applies the
/// authenticated snapshot predicate before grouping, count and pagination.
pub async fn report(
    pool: &SqlitePool,
    scope: &Scope,
    filter: &UsageFilter,
) -> Result<UsageReport, LbError> {
    if filter.page < 1
        || !(1..=1000).contains(&filter.per_page)
        || filter.from >= filter.to_exclusive
    {
        return Err(CommonError::Validation("Invalid usage interval or pagination".into()).into());
    }
    let offset = (filter.page - 1)
        .checked_mul(filter.per_page)
        .ok_or_else(|| {
            CommonError::Validation("Usage page is outside the supported range".into())
        })?;
    let mut transaction = pool.begin().await.map_err(database_error)?;
    validate_scope_filters(&mut transaction, scope, filter).await?;

    let mut count = QueryBuilder::<Sqlite>::new(
        "SELECT COUNT(*) FROM (SELECT d.organization_id FROM usage_daily d",
    );
    push_predicate(&mut count, scope, filter);
    push_grouping(&mut count, filter.group_by);
    count.push(") authorized_groups");
    let total: i64 = count
        .build_query_scalar()
        .fetch_one(&mut *transaction)
        .await
        .map_err(database_error)?;

    let mut totals = QueryBuilder::<Sqlite>::new("SELECT ");
    totals.push(SUM_COLUMNS).push(" FROM usage_daily d");
    push_predicate(&mut totals, scope, filter);
    let totals = totals
        .build_query_as::<UsageTotals>()
        .fetch_one(&mut *transaction)
        .await
        .map_err(database_error)?;

    let mut rows = QueryBuilder::<Sqlite>::new(
        "SELECT NULLIF(d.organization_id, '') AS organization_id,
         o.name AS organization_name,
         (d.organization_id != '' AND (o.id IS NULL OR o.archived_at IS NOT NULL)) AS organization_archived,",
    );
    match filter.group_by {
        GroupBy::Organization => {
            rows.push("NULL AS account_id, NULL AS account_name, 0 AS account_deleted,");
        }
        GroupBy::Account => {
            rows.push(
                "NULLIF(d.account_id, '') AS account_id, u.username AS account_name,
                       (d.account_id != '' AND u.id IS NULL) AS account_deleted,",
            );
        }
    }
    rows.push(SUM_COLUMNS).push(
        " FROM usage_daily d
          LEFT JOIN organizations o ON o.id = d.organization_id
          LEFT JOIN users u ON u.id = d.account_id",
    );
    push_predicate(&mut rows, scope, filter);
    push_grouping(&mut rows, filter.group_by);
    rows.push(" ORDER BY d.organization_id");
    if filter.group_by == GroupBy::Account {
        rows.push(", d.account_id");
    }
    rows.push(" LIMIT ")
        .push_bind(filter.per_page)
        .push(" OFFSET ")
        .push_bind(offset);
    let rows = rows
        .build_query_as::<UsageRow>()
        .fetch_all(&mut *transaction)
        .await
        .map_err(database_error)?;

    let coverage_start: String =
        sqlx::query_scalar("SELECT started_at FROM usage_observation WHERE id = 1")
            .fetch_one(&mut *transaction)
            .await
            .map_err(database_error)?;
    transaction.commit().await.map_err(database_error)?;
    Ok(UsageReport {
        group_by: filter.group_by,
        from: filter.from.to_string(),
        to: filter
            .to_exclusive
            .pred_opt()
            .expect("nonempty date interval")
            .to_string(),
        page: filter.page,
        per_page: filter.per_page,
        total,
        rows,
        totals,
        coverage_start,
    })
}
