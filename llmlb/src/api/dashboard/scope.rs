//! Dashboard read authorization and scoped operational summaries.

use super::{collect_endpoints, collect_stats, AppError, DashboardEndpoint, DashboardStats};
use crate::db::request_history::scoped::scope_predicate;
use crate::{
    auth::scope::{resolve_scope, UsageScope},
    common::{auth::Claims, error::LbError},
    AppState,
};
use axum::{
    extract::{Request, State},
    http::{header, HeaderValue},
    middleware::Next,
    response::Response,
};
use chrono::{Timelike, Utc};
use std::collections::HashMap;
use uuid::Uuid;

/// Resolve the current database membership before serving a dashboard read.
pub async fn authorize_scope(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let claims = request
        .extensions()
        .get::<Claims>()
        .ok_or_else(|| AppError(LbError::Authentication("JWT session required".into())))?;
    let scope = resolve_scope(&state.db_pool, claims).await?;
    let identity = match &scope {
        UsageScope::Global => "global".to_string(),
        UsageScope::Organization(organization_id) => format!("organization:{organization_id}"),
        UsageScope::Account {
            organization_id,
            account_id,
        } => format!("account:{organization_id}:{account_id}"),
    };
    request.extensions_mut().insert(scope);
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        "x-llmlb-usage-scope",
        HeaderValue::from_str(&identity)
            .map_err(|_| AppError(LbError::Internal("Invalid usage scope identity".into())))?,
    );
    // Cookies authenticate these scoped GETs; intermediary caches must not reuse
    // a previous account's body when the shared URL stays the same.
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    Ok(response)
}

pub(super) fn global_only(scope: &UsageScope) -> Result<(), AppError> {
    if *scope == UsageScope::Global {
        Ok(())
    } else {
        Err(AppError(LbError::Authorization(
            "Global administrator access required".into(),
        )))
    }
}

#[derive(Default, sqlx::FromRow)]
pub(super) struct Totals {
    pub request_count: i64,
    pub successful_requests: i64,
    pub failed_requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
}

pub(super) async fn totals(state: &AppState, scope: &UsageScope) -> Result<Totals, AppError> {
    let sql = format!("SELECT COALESCE(SUM(request_count),0) request_count, COALESCE(SUM(successful_requests),0) successful_requests, COALESCE(SUM(failed_requests),0) failed_requests, COALESCE(SUM(input_tokens),0) input_tokens, COALESCE(SUM(output_tokens),0) output_tokens, COALESCE(SUM(total_tokens),0) total_tokens FROM usage_daily WHERE {}", scope_predicate(scope));
    sqlx::query_as(&sql)
        .fetch_one(&state.db_pool)
        .await
        .map_err(database)
}

pub(super) fn database(error: sqlx::Error) -> AppError {
    AppError(LbError::Database(format!(
        "Scoped usage read failed: {error}"
    )))
}

pub(super) async fn endpoints(
    state: &AppState,
    scope: &UsageScope,
) -> Result<Vec<DashboardEndpoint>, AppError> {
    let mut endpoints = collect_endpoints(state).await;
    #[derive(sqlx::FromRow)]
    struct Row {
        endpoint_id: String,
        total: i64,
        success: i64,
        failure: i64,
    }
    let sql = format!("SELECT endpoint_id, SUM(request_count) total, SUM(successful_requests) success, SUM(failed_requests) failure FROM usage_daily WHERE {} GROUP BY endpoint_id", scope_predicate(scope));
    let rows: Vec<Row> = sqlx::query_as(&sql)
        .fetch_all(&state.db_pool)
        .await
        .map_err(database)?;
    let counts: HashMap<_, _> = rows
        .into_iter()
        .map(|row| (row.endpoint_id.clone(), row))
        .collect();
    for endpoint in &mut endpoints {
        let row = counts.get(&endpoint.id.to_string());
        endpoint.total_requests = row.map_or(0, |row| row.total);
        endpoint.successful_requests = row.map_or(0, |row| row.success);
        endpoint.failed_requests = row.map_or(0, |row| row.failure);
    }
    Ok(endpoints)
}

pub(super) async fn stats(
    state: &AppState,
    scope: &UsageScope,
) -> Result<DashboardStats, AppError> {
    if *scope == UsageScope::Global {
        return Ok(collect_stats(state).await);
    }
    let endpoints = endpoints(state, scope).await?;
    let totals = totals(state, scope).await?;
    let activity = activity(state, scope).await?;
    use crate::types::endpoint::EndpointStatus;
    Ok(DashboardStats {
        total_nodes: endpoints.len(),
        online_nodes: endpoints
            .iter()
            .filter(|e| e.status == EndpointStatus::Online)
            .count(),
        pending_nodes: 0,
        registering_nodes: 0,
        offline_nodes: endpoints
            .iter()
            .filter(|e| e.status == EndpointStatus::Offline)
            .count(),
        total_requests: totals.request_count as u64,
        successful_requests: totals.successful_requests as u64,
        failed_requests: totals.failed_requests as u64,
        total_active_requests: activity.total.active_requests,
        queued_requests: activity.total.queued_requests,
        average_response_time_ms: None,
        average_gpu_usage: None,
        average_gpu_memory_usage: None,
        last_metrics_updated_at: None,
        last_registered_at: endpoints.iter().map(|e| e.registered_at).max(),
        last_seen_at: endpoints.iter().filter_map(|e| e.last_seen).max(),
        openai_key_present: false,
        google_key_present: false,
        anthropic_key_present: false,
        total_input_tokens: totals.input_tokens as u64,
        total_output_tokens: totals.output_tokens as u64,
        total_tokens: totals.total_tokens as u64,
    })
}

pub(super) async fn history(
    state: &AppState,
    scope: &UsageScope,
) -> Result<Vec<crate::balancer::RequestHistoryPoint>, AppError> {
    if *scope == UsageScope::Global {
        return Ok(super::collect_history(state).await);
    }
    state
        .balancer
        .request_history
        .scoped(scope.clone())
        .get_recent_history_by_minute(60)
        .await?
        .into_iter()
        .map(|row| {
            Ok(crate::balancer::RequestHistoryPoint {
                minute: row
                    .minute
                    .parse()
                    .map_err(|_| AppError(LbError::Database("Invalid history timestamp".into())))?,
                success: row.success_count as u64,
                error: row.error_count as u64,
            })
        })
        .collect()
}

pub(super) async fn activity(
    state: &AppState,
    scope: &UsageScope,
) -> Result<crate::balancer::RequestActivitySnapshot, AppError> {
    use crate::balancer::{
        EndpointRequestActivity, RequestActivitySnapshot, RequestActivityTotals,
    };
    if *scope == UsageScope::Global {
        return Ok(state.balancer.load_manager.request_activity().await);
    }
    let now = Utc::now().with_nanosecond(0).expect("valid UTC second");
    #[derive(sqlx::FromRow)]
    struct Row {
        endpoint_id: String,
        received_at: String,
        finalized: bool,
    }
    let sql = format!("SELECT endpoint_id,received_at,finalized FROM usage_receipts WHERE ({}) AND (received_at >= ? OR finalized = 0)", scope_predicate(scope));
    let rows: Vec<Row> = sqlx::query_as(&sql)
        .bind((now - chrono::Duration::seconds(59)).to_rfc3339())
        .fetch_all(&state.db_pool)
        .await
        .map_err(database)?;
    let mut total = RequestActivityTotals {
        active_requests: 0,
        queued_requests: 0,
        requests_per_second: vec![0; 60],
    };
    let mut endpoint_rows: HashMap<Uuid, EndpointRequestActivity> = HashMap::new();
    for row in rows {
        let timestamp: chrono::DateTime<Utc> = row
            .received_at
            .parse()
            .map_err(|_| AppError(LbError::Database("Invalid receipt timestamp".into())))?;
        let age = now.timestamp() - timestamp.timestamp();
        let slot = (0..60).contains(&age).then_some((59 - age) as usize);
        if let Some(slot) = slot {
            total.requests_per_second[slot] += 1;
        }
        if let Ok(id) = row.endpoint_id.parse::<Uuid>() {
            let endpoint = endpoint_rows
                .entry(id)
                .or_insert_with(|| EndpointRequestActivity {
                    endpoint_id: id,
                    active_requests: 0,
                    queued_requests: 0,
                    requests_per_second: vec![0; 60],
                });
            if let Some(slot) = slot {
                endpoint.requests_per_second[slot] += 1;
            }
            if !row.finalized {
                total.active_requests = total.active_requests.saturating_add(1);
                endpoint.active_requests = endpoint.active_requests.saturating_add(1);
            }
        } else if !row.finalized {
            total.queued_requests += 1;
        }
    }
    let mut endpoints: Vec<_> = endpoint_rows.into_values().collect();
    endpoints.sort_by_key(|row| row.endpoint_id);
    Ok(RequestActivitySnapshot {
        timestamp: now,
        bucket_seconds: 1,
        window_seconds: 60,
        total,
        endpoints,
    })
}
