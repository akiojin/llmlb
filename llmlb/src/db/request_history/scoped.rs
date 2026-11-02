//! SQL visibility boundary shared by every retained-history reader.

use super::*;
use crate::auth::scope::Scope;
use sqlx::{QueryBuilder, Sqlite};

impl RequestHistoryStorage {
    /// Known terminal outcomes are retained with the same daily usage bucket.
    pub(crate) async fn scoped_usage_outcomes(&self) -> RouterResult<(i64, i64, i64)> {
        let scope = self
            .scope
            .as_ref()
            .ok_or_else(|| LbError::Authorization("An explicit usage scope is required".into()))?;
        let mut query=QueryBuilder::<Sqlite>::new("SELECT COALESCE(SUM(successful_requests),0),COALESCE(SUM(failed_requests),0),COALESCE(SUM(request_count-pending_requests-successful_requests-failed_requests),0) FROM usage_daily WHERE ");
        scope.push_sql(&mut query, "organization_id", "account_id");
        query
            .build_query_as()
            .fetch_one(&self.pool)
            .await
            .map_err(|error| {
                LbError::Database(format!("Failed to read durable usage outcomes: {error}"))
            })
    }
    async fn rollup_periods(
        &self,
        period: &str,
        cutoff: String,
    ) -> RouterResult<Vec<(String, i64, i64, i64, i64)>> {
        let scope = self
            .scope
            .as_ref()
            .ok_or_else(|| LbError::Authorization("An explicit usage scope is required".into()))?;
        let mut query=QueryBuilder::<Sqlite>::new(format!("SELECT {period} period,SUM(input_tokens),SUM(output_tokens),SUM(total_tokens),SUM(request_count) FROM usage_daily WHERE "));
        scope.push_sql(&mut query, "organization_id", "account_id");
        query
            .push(" AND date >= ")
            .push_bind(cutoff)
            .push(format!(" GROUP BY {period} ORDER BY period DESC"));
        query
            .build_query_as()
            .fetch_all(&self.pool)
            .await
            .map_err(|error| LbError::Database(error.to_string()))
    }

    pub(super) async fn scoped_daily_tokens(
        &self,
        days: u32,
    ) -> RouterResult<Vec<crate::api::dashboard::DailyTokenStats>> {
        let cutoff = (Utc::now().date_naive() - chrono::Duration::days(i64::from(days.min(36500))))
            .to_string();
        Ok(self
            .rollup_periods("date", cutoff)
            .await?
            .into_iter()
            .map(
                |(date, input, output, total, requests)| crate::api::dashboard::DailyTokenStats {
                    date,
                    total_input_tokens: input as u64,
                    total_output_tokens: output as u64,
                    total_tokens: total as u64,
                    request_count: requests as u64,
                },
            )
            .collect())
    }

    pub(super) async fn scoped_monthly_tokens(
        &self,
        months: u32,
    ) -> RouterResult<Vec<crate::api::dashboard::MonthlyTokenStats>> {
        let cutoff = Utc::now()
            .date_naive()
            .checked_sub_months(chrono::Months::new(months.min(1200)))
            .ok_or_else(|| LbError::Database("Invalid monthly usage interval".into()))?
            .to_string();
        Ok(self
            .rollup_periods("substr(date,1,7)", cutoff)
            .await?
            .into_iter()
            .map(|(month, input, output, total, requests)| {
                crate::api::dashboard::MonthlyTokenStats {
                    month,
                    total_input_tokens: input as u64,
                    total_output_tokens: output as u64,
                    total_tokens: total as u64,
                    request_count: requests as u64,
                }
            })
            .collect())
    }
    /// Clone the handle with an explicit HTTP read scope, sharing terminal writers.
    pub fn scoped(&self, scope: &Scope) -> Self {
        Self {
            pool: self.pool.clone(),
            usage_writers: self.usage_writers.clone(),
            scope: Some(scope.clone()),
        }
    }

    /// Require valid membership before entering any private history handler.
    pub fn require_read_scope(scope: &Scope) -> Result<(), crate::api::error::AppError> {
        if matches!(scope, Scope::Unassigned(_)) {
            return Err(crate::api::error::AppError(LbError::Authorization(
                "An active organization membership is required".into(),
            )));
        }
        Ok(())
    }

    /// Prefix SQL with a restricted CTE before COUNT, grouping, filtering or LIMIT.
    /// The only interpolated values are UUIDs from the verified server scope.
    /// Existing SQL parameters retain their order and existing recursive CTEs stay valid.
    pub(super) fn read_sql(&self, sql: &str) -> String {
        let predicate = match self.scope.as_ref() {
            None | Some(Scope::Global) => return sql.to_owned(),
            Some(Scope::Organization(id)) => format!("r.organization_id = '{id}'"),
            Some(Scope::SelfAccount {
                organization_id,
                account_id,
            }) => {
                format!("r.organization_id = '{organization_id}' AND r.account_id = '{account_id}'")
            }
            Some(Scope::Unassigned(_)) => "0 = 1".into(),
        };
        let cte = format!("request_history AS (SELECT h.* FROM main.request_history h JOIN usage_receipts r ON r.request_id = h.id WHERE {predicate})");
        let sql = sql.trim_start();
        if let Some(rest) = sql.strip_prefix("WITH RECURSIVE ") {
            format!("WITH RECURSIVE {cte}, {rest}")
        } else if let Some(rest) = sql.strip_prefix("WITH ") {
            format!("WITH {cte}, {rest}")
        } else {
            format!("WITH {cte} {sql}")
        }
    }

    /// Durable deployment-onward totals for an explicit HTTP scope.
    pub async fn scoped_usage_totals(&self) -> RouterResult<crate::db::usage::query::UsageTotals> {
        let scope = self
            .scope
            .as_ref()
            .ok_or_else(|| LbError::Authorization("An explicit usage scope is required".into()))?;
        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT COALESCE(SUM(request_count),0) request_count, COALESCE(SUM(input_tokens),0) input_tokens,
             COALESCE(SUM(output_tokens),0) output_tokens, COALESCE(SUM(total_tokens),0) total_tokens,
             COALESCE(SUM(provider_count),0) measured_requests, COALESCE(SUM(estimated_count),0) estimated_requests,
             COALESCE(SUM(missing_count),0) missing_requests, COALESCE(SUM(pending_requests),0) pending_requests,
             COALESCE(SUM(input_missing_count),0) input_missing_count, COALESCE(SUM(output_missing_count),0) output_missing_count,
             COALESCE(SUM(total_missing_count),0) total_missing_count FROM usage_daily WHERE ",
        );
        scope.push_sql(&mut query, "organization_id", "account_id");
        query
            .build_query_as()
            .fetch_one(&self.pool)
            .await
            .map_err(|error| {
                LbError::Database(format!("Failed to read durable scoped totals: {error}"))
            })
    }

    pub(crate) async fn scoped_request_counts(
        &self,
        endpoint: Option<Uuid>,
    ) -> RouterResult<(i64, i64, i64, Option<f64>)> {
        let sql = self.read_sql(
            "SELECT COUNT(*),
            COALESCE(SUM(r.outcome='success'),0),
            COALESCE(SUM(r.outcome='error'),0),
            AVG(CASE WHEN r.outcome IS NOT NULL THEN h.duration_ms END)
            FROM request_history h JOIN usage_receipts r ON r.request_id=h.id
            WHERE (? IS NULL OR h.endpoint_id = ?)",
        );
        let endpoint = endpoint.map(|id| id.to_string());
        sqlx::query_as(&sql)
            .bind(&endpoint)
            .bind(&endpoint)
            .fetch_one(&self.pool)
            .await
            .map_err(|error| {
                LbError::Database(format!("Failed to read scoped request counts: {error}"))
            })
    }

    pub(crate) async fn scoped_endpoint_days(
        &self,
        endpoint: Uuid,
        days: u32,
    ) -> RouterResult<Vec<crate::db::endpoint_daily_stats::DailyStatEntry>> {
        let sql = self.read_sql(
            "SELECT DATE(h.timestamp) date, COUNT(*) total_requests,
            COALESCE(SUM(r.outcome='success'),0) successful_requests,
            COALESCE(SUM(r.outcome='error'),0) failed_requests
            FROM request_history h JOIN usage_receipts r ON r.request_id=h.id
            WHERE h.endpoint_id=? AND DATE(h.timestamp)>=DATE('now','-' || ? || ' days')
            GROUP BY DATE(h.timestamp) ORDER BY date ASC",
        );
        let rows: Vec<(String, i64, i64, i64)> = sqlx::query_as(&sql)
            .bind(endpoint.to_string())
            .bind(i64::from(days))
            .fetch_all(&self.pool)
            .await
            .map_err(|error| LbError::Database(error.to_string()))?;
        Ok(rows
            .into_iter()
            .map(
                |(date, total_requests, successful_requests, failed_requests)| {
                    crate::db::endpoint_daily_stats::DailyStatEntry {
                        date,
                        total_requests,
                        successful_requests,
                        failed_requests,
                    }
                },
            )
            .collect())
    }

    pub(crate) async fn scoped_model_stats(
        &self,
        endpoint: Option<Uuid>,
    ) -> RouterResult<Vec<crate::db::endpoint_daily_stats::ModelStatEntry>> {
        let sql = self.read_sql(
            "SELECT h.model,COUNT(*),
            COALESCE(SUM(r.outcome='success'),0),
            COALESCE(SUM(r.outcome='error'),0),
            COALESCE(SUM(h.output_tokens),0),
            COALESCE(SUM(CASE WHEN r.outcome IS NOT NULL THEN h.duration_ms END),0)
            FROM request_history h JOIN usage_receipts r ON r.request_id=h.id
            WHERE (? IS NULL OR h.endpoint_id=?) GROUP BY h.model ORDER BY h.model",
        );
        let endpoint = endpoint.map(|id| id.to_string());
        let rows: Vec<(String, i64, i64, i64, i64, i64)> = sqlx::query_as(&sql)
            .bind(&endpoint)
            .bind(&endpoint)
            .fetch_all(&self.pool)
            .await
            .map_err(|error| LbError::Database(error.to_string()))?;
        Ok(rows
            .into_iter()
            .map(
                |(
                    model_id,
                    total_requests,
                    successful_requests,
                    failed_requests,
                    total_output_tokens,
                    total_duration_ms,
                )| crate::db::endpoint_daily_stats::ModelStatEntry {
                    model_id,
                    total_requests,
                    successful_requests,
                    failed_requests,
                    total_output_tokens,
                    total_duration_ms,
                },
            )
            .collect())
    }

    pub(crate) async fn scoped_activity(
        &self,
    ) -> RouterResult<crate::balancer::RequestActivitySnapshot> {
        use crate::balancer::{RequestActivitySnapshot, RequestActivityTotals};
        let scope = self
            .scope
            .as_ref()
            .ok_or_else(|| LbError::Authorization("An explicit usage scope is required".into()))?;
        let now = chrono::DateTime::<Utc>::from_timestamp(Utc::now().timestamp(), 0)
            .expect("current UTC second is valid");
        let mut query=QueryBuilder::<Sqlite>::new("SELECT CAST(strftime('%s',received_at) AS INTEGER) second,COUNT(*) FROM usage_receipts WHERE ");
        scope.push_sql(&mut query, "organization_id", "account_id");
        query
            .push(" AND received_at >= ")
            .push_bind((now - chrono::Duration::seconds(59)).to_rfc3339());
        query.push(" GROUP BY second");
        let rows: Vec<(i64, i64)> = query
            .build_query_as()
            .fetch_all(&self.pool)
            .await
            .map_err(|error| LbError::Database(error.to_string()))?;
        let mut series = vec![0; 60];
        for (second, count) in rows {
            let offset = second - (now.timestamp() - 59);
            if (0..60).contains(&offset) {
                series[offset as usize] = count as u64;
            }
        }
        let totals = self.scoped_usage_totals().await?;
        Ok(RequestActivitySnapshot {
            timestamp: now,
            bucket_seconds: 1,
            window_seconds: 60,
            total: RequestActivityTotals {
                active_requests: u32::try_from(totals.pending_requests).unwrap_or(u32::MAX),
                queued_requests: 0,
                requests_per_second: series,
            },
            endpoints: vec![],
        })
    }
}
