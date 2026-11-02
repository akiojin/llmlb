//! ダッシュボード概況の集約（エンドポイント・統計・運用指標・キャパシティ・対応事項）

use super::{
    DashboardActionItem, DashboardCapacity, DashboardEndpoint, DashboardOperations, DashboardStats,
    PersistedTokenTotals,
};
use crate::{
    balancer::RequestHistoryPoint,
    types::endpoint::{EndpointStatus, EndpointType},
    AppState,
};
use std::collections::HashSet;
use tracing::warn;

pub(super) async fn collect_scoped_endpoints(
    state: &AppState,
    scope: &crate::auth::scope::Scope,
) -> Result<Vec<DashboardEndpoint>, crate::api::error::AppError> {
    let mut endpoints = collect_endpoints(state).await;
    let storage = state.balancer.request_history.scoped(scope);
    for endpoint in &mut endpoints {
        let (total, success, failure, _) = storage
            .scoped_request_counts(Some(endpoint.id))
            .await
            .map_err(crate::api::error::AppError)?;
        endpoint.total_requests = total;
        endpoint.successful_requests = success;
        endpoint.failed_requests = failure;
    }
    Ok(endpoints)
}

pub(super) async fn collect_scoped_stats(
    state: &AppState,
    scope: &crate::auth::scope::Scope,
) -> Result<DashboardStats, crate::api::error::AppError> {
    use crate::{api::error::AppError, auth::scope::Scope};
    let storage = state.balancer.request_history.scoped(scope);
    let totals = storage.scoped_usage_totals().await.map_err(AppError)?;
    let (_, _, _, average) = storage
        .scoped_request_counts(None)
        .await
        .map_err(AppError)?;
    let (success, failure, unknown) = storage.scoped_usage_outcomes().await.map_err(AppError)?;
    let summary = state.balancer.load_manager.summary().await;
    let endpoints = state.balancer.endpoint_registry.list().await;
    let global = matches!(scope, Scope::Global);
    Ok(DashboardStats {
        total_nodes: summary.total_nodes,
        online_nodes: summary.online_nodes,
        pending_nodes: summary.pending_nodes,
        registering_nodes: summary.registering_nodes,
        offline_nodes: summary.offline_nodes,
        total_requests: totals.request_count as u64,
        successful_requests: success as u64,
        failed_requests: failure as u64,
        unknown_outcome_requests: unknown as u64,
        total_active_requests: if global {
            summary.total_active_requests
        } else {
            u32::try_from(totals.pending_requests).unwrap_or(u32::MAX)
        },
        queued_requests: if global { summary.queued_requests } else { 0 },
        average_response_time_ms: average.map(|value| value as f32),
        average_gpu_usage: summary.average_gpu_usage,
        average_gpu_memory_usage: summary.average_gpu_memory_usage,
        last_metrics_updated_at: summary.last_metrics_updated_at,
        last_registered_at: endpoints
            .iter()
            .map(|endpoint| endpoint.registered_at)
            .max(),
        last_seen_at: endpoints
            .iter()
            .filter_map(|endpoint| endpoint.last_seen)
            .max(),
        openai_key_present: global && std::env::var("OPENAI_API_KEY").is_ok(),
        google_key_present: global && std::env::var("GOOGLE_API_KEY").is_ok(),
        anthropic_key_present: global && std::env::var("ANTHROPIC_API_KEY").is_ok(),
        total_input_tokens: totals.input_tokens as u64,
        total_output_tokens: totals.output_tokens as u64,
        total_tokens: totals.total_tokens as u64,
    })
}

pub(super) async fn collect_scoped_history(
    state: &AppState,
    scope: &crate::auth::scope::Scope,
) -> Result<Vec<RequestHistoryPoint>, crate::api::error::AppError> {
    state
        .balancer
        .request_history
        .scoped(scope)
        .get_recent_history_by_minute(60)
        .await
        .map_err(crate::api::error::AppError)?
        .into_iter()
        .map(|point| {
            let minute = point.minute.parse().map_err(|error| {
                crate::api::error::AppError(crate::common::error::LbError::Database(format!(
                    "Invalid stored history minute: {error}"
                )))
            })?;
            Ok(RequestHistoryPoint {
                minute,
                success: point.success_count as u64,
                error: point.error_count as u64,
            })
        })
        .collect()
}

/// エンドポイント一覧を収集
///
/// SPEC-e8e9326e: llmlb主導エンドポイント登録システム
pub(super) async fn collect_endpoints(state: &AppState) -> Vec<DashboardEndpoint> {
    let endpoint_registry = &state.balancer.endpoint_registry;
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

pub(super) fn collect_operations(
    stats: &DashboardStats,
    endpoints: &[DashboardEndpoint],
    token_totals: PersistedTokenTotals,
    endpoint_tps: &[crate::balancer::EndpointTpsSummary],
) -> DashboardOperations {
    let error_endpoints = endpoints
        .iter()
        .filter(|endpoint| endpoint.status == EndpointStatus::Error)
        .count();
    let offline_endpoints = endpoints
        .iter()
        .filter(|endpoint| endpoint.status == EndpointStatus::Offline)
        .count();
    let known_outcomes = stats
        .successful_requests
        .saturating_add(stats.failed_requests);
    let success_rate = if known_outcomes > 0 {
        Some((stats.successful_requests as f64 / known_outcomes as f64 * 100.0) as f32)
    } else {
        None
    };
    let health = if stats.total_nodes == 0 {
        "empty"
    } else if error_endpoints > 0
        || offline_endpoints > 0
        || stats.failed_requests > 0
        || stats.queued_requests > 0
    {
        "attention"
    } else {
        "healthy"
    };

    DashboardOperations {
        health: health.to_string(),
        total_endpoints: stats.total_nodes,
        online_endpoints: stats.online_nodes,
        pending_endpoints: stats.pending_nodes,
        registering_endpoints: stats.registering_nodes,
        offline_endpoints,
        error_endpoints,
        total_requests: stats.total_requests,
        successful_requests: stats.successful_requests,
        failed_requests: stats.failed_requests,
        unknown_outcome_requests: stats.unknown_outcome_requests,
        success_rate,
        active_requests: stats.total_active_requests,
        queued_requests: stats.queued_requests,
        average_response_time_ms: stats.average_response_time_ms,
        output_tps: calculate_output_tps(endpoint_tps),
        total_input_tokens: token_totals.total_input_tokens,
        total_output_tokens: token_totals.total_output_tokens,
        total_tokens: token_totals.total_tokens,
        last_registered_at: stats.last_registered_at,
        last_seen_at: stats.last_seen_at,
    }
}

pub(super) fn calculate_output_tps(
    endpoint_tps: &[crate::balancer::EndpointTpsSummary],
) -> Option<f64> {
    let mut total_output_tokens = 0.0;
    let mut total_duration_seconds = 0.0;

    for summary in endpoint_tps {
        let Some(tps) = summary.aggregate_tps else {
            continue;
        };
        if !tps.is_finite() || tps <= 0.0 || summary.total_output_tokens == 0 {
            continue;
        }

        let output_tokens = summary.total_output_tokens as f64;
        total_output_tokens += output_tokens;
        total_duration_seconds += output_tokens / tps;
    }

    if total_output_tokens > 0.0 && total_duration_seconds > 0.0 {
        Some(total_output_tokens / total_duration_seconds)
    } else {
        None
    }
}

pub(super) async fn collect_capacity(
    state: &AppState,
    dashboard_endpoints: &[DashboardEndpoint],
) -> DashboardCapacity {
    let endpoints = state.balancer.endpoint_registry.list().await;
    let mut model_ids = Vec::new();
    for endpoint in dashboard_endpoints {
        match state
            .balancer
            .endpoint_registry
            .list_models(endpoint.id)
            .await
        {
            Ok(models) => model_ids.extend(models.into_iter().map(|model| model.model_id)),
            Err(e) => warn!(
                endpoint_id = %endpoint.id,
                "Failed to list endpoint models for dashboard capacity: {}",
                e
            ),
        }
    }
    let total_models = count_unique_model_ids(model_ids);
    let mut gpu_capable_endpoints = 0usize;
    let mut gpu_telemetry_endpoints = 0usize;
    let mut total_gpu_memory_bytes = 0u64;
    let mut used_gpu_memory_bytes = 0u64;

    for endpoint in endpoints {
        let has_gpu_info = endpoint.gpu_device_count.unwrap_or(0) > 0
            || endpoint.gpu_total_memory_bytes.unwrap_or(0) > 0
            || endpoint.gpu_used_memory_bytes.unwrap_or(0) > 0;
        let can_report_gpu = matches!(
            endpoint.endpoint_type,
            EndpointType::Xllm | EndpointType::Vllm | EndpointType::Llamacpp
        );
        if has_gpu_info || can_report_gpu {
            gpu_capable_endpoints = gpu_capable_endpoints.saturating_add(1);
        }
        if let Some(total) = endpoint.gpu_total_memory_bytes {
            if total > 0 {
                gpu_telemetry_endpoints = gpu_telemetry_endpoints.saturating_add(1);
                total_gpu_memory_bytes = total_gpu_memory_bytes.saturating_add(total);
                used_gpu_memory_bytes = used_gpu_memory_bytes
                    .saturating_add(endpoint.gpu_used_memory_bytes.unwrap_or(0));
            }
        }
    }

    let gpu_memory_usage_percent = if total_gpu_memory_bytes > 0 {
        Some((used_gpu_memory_bytes as f64 / total_gpu_memory_bytes as f64 * 100.0) as f32)
    } else {
        None
    };
    let telemetry_status = if gpu_telemetry_endpoints == 0 {
        "unavailable"
    } else if gpu_telemetry_endpoints < gpu_capable_endpoints {
        "partial"
    } else {
        "available"
    };

    DashboardCapacity {
        total_models,
        gpu_capable_endpoints,
        gpu_telemetry_endpoints,
        total_gpu_memory_bytes: (total_gpu_memory_bytes > 0).then_some(total_gpu_memory_bytes),
        used_gpu_memory_bytes: (total_gpu_memory_bytes > 0).then_some(used_gpu_memory_bytes),
        gpu_memory_usage_percent,
        telemetry_status: telemetry_status.to_string(),
    }
}

pub(super) fn count_unique_model_ids(model_ids: impl IntoIterator<Item = String>) -> usize {
    let mut unique_model_ids = HashSet::new();
    for model_id in model_ids {
        unique_model_ids.insert(model_id);
    }
    unique_model_ids.len()
}

pub(super) fn collect_action_items(operations: &DashboardOperations) -> Vec<DashboardActionItem> {
    let mut items = Vec::new();

    if operations.error_endpoints > 0 {
        items.push(DashboardActionItem {
            severity: "critical".to_string(),
            title: "Endpoint errors".to_string(),
            detail: "One or more endpoints are reporting errors.".to_string(),
            count: operations.error_endpoints,
        });
    }
    if operations.offline_endpoints > 0 {
        items.push(DashboardActionItem {
            severity: "warning".to_string(),
            title: "Offline endpoints".to_string(),
            detail: "Registered endpoints are offline and cannot serve traffic.".to_string(),
            count: operations.offline_endpoints,
        });
    }
    if operations.queued_requests > 0 {
        items.push(DashboardActionItem {
            severity: "warning".to_string(),
            title: "Queue pressure".to_string(),
            detail: "Requests are waiting for an available endpoint.".to_string(),
            count: operations.queued_requests,
        });
    }
    if operations.failed_requests > 0 {
        items.push(DashboardActionItem {
            severity: "warning".to_string(),
            title: "Failed requests".to_string(),
            detail: "Recent or persisted request failures need review.".to_string(),
            count: operations.failed_requests.min(usize::MAX as u64) as usize,
        });
    }

    if items.is_empty() {
        items.push(DashboardActionItem {
            severity: "info".to_string(),
            title: "No action required".to_string(),
            detail: "All operational signals are nominal.".to_string(),
            count: 0,
        });
    }

    items
}
