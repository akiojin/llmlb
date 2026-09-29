//! ダッシュボード概況の集約（エンドポイント・統計・運用指標・キャパシティ・対応事項）

use super::{
    DashboardActionItem, DashboardCapacity, DashboardEndpoint, DashboardOperations, DashboardStats,
    PersistedRequestTotals, PersistedTokenTotals, LAST_KNOWN_PERSISTED_TOTALS,
};
use crate::{
    balancer::RequestHistoryPoint,
    types::endpoint::{EndpointStatus, EndpointType},
    AppState,
};
use std::collections::HashSet;
use tracing::warn;

/// エンドポイント一覧を収集
///
/// SPEC-e8e9326e: llmlb主導エンドポイント登録システム
pub(super) async fn collect_endpoints(state: &AppState) -> Vec<DashboardEndpoint> {
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

pub(super) async fn collect_stats(state: &AppState) -> DashboardStats {
    let load_manager = state.load_manager.clone();

    let summary = load_manager.summary().await;
    let endpoints = state.endpoint_registry.list().await;

    let last_registered_at = endpoints.iter().map(|e| e.registered_at).max();
    let last_seen_at = endpoints.iter().filter_map(|e| e.last_seen).max();

    let openai_key_present = std::env::var("OPENAI_API_KEY").is_ok();
    let google_key_present = std::env::var("GOOGLE_API_KEY").is_ok();
    let anthropic_key_present = std::env::var("ANTHROPIC_API_KEY").is_ok();

    let to_u64 = |value: i64| -> u64 {
        if value < 0 {
            0
        } else {
            value as u64
        }
    };
    let cache_key = load_manager.cache_key();

    let request_totals_from_db =
        match crate::db::endpoints::get_request_totals(&state.db_pool).await {
            Ok(request_totals) => Some(PersistedRequestTotals {
                total_requests: to_u64(request_totals.total_requests),
                successful_requests: to_u64(request_totals.successful_requests),
                failed_requests: to_u64(request_totals.failed_requests),
            }),
            Err(e) => {
                warn!("Failed to query persisted request totals: {}", e);
                None
            }
        };

    // request_history 廃止完了まで、audit_log/request_history の双方を見て過小計上を避ける
    let token_totals_from_audit = match state.audit_log_storage.get_token_statistics().await {
        Ok(stats) => Some(PersistedTokenTotals {
            total_input_tokens: to_u64(stats.total_input_tokens),
            total_output_tokens: to_u64(stats.total_output_tokens),
            total_tokens: to_u64(stats.total_tokens),
        }),
        Err(e) => {
            warn!("Failed to query token statistics from audit log: {}", e);
            None
        }
    };
    let token_totals_from_history = match state.request_history.get_token_statistics().await {
        Ok(stats) => Some(PersistedTokenTotals {
            total_input_tokens: stats.total_input_tokens,
            total_output_tokens: stats.total_output_tokens,
            total_tokens: stats.total_tokens,
        }),
        Err(e) => {
            warn!(
                "Failed to query token statistics from request history: {}",
                e
            );
            None
        }
    };
    let token_totals_from_db = match (token_totals_from_audit, token_totals_from_history) {
        (Some(audit), Some(history)) => Some(PersistedTokenTotals {
            total_input_tokens: audit.total_input_tokens.max(history.total_input_tokens),
            total_output_tokens: audit.total_output_tokens.max(history.total_output_tokens),
            total_tokens: audit.total_tokens.max(history.total_tokens),
        }),
        (Some(audit), None) => Some(audit),
        (None, Some(history)) => Some(history),
        (None, None) => None,
    };

    let cached_totals = LAST_KNOWN_PERSISTED_TOTALS
        .read()
        .ok()
        .and_then(|guard| guard.get(&cache_key).copied());

    let request_totals = if let Some(request_totals) = request_totals_from_db {
        request_totals
    } else if let Some(cached) = cached_totals {
        warn!("Using last known persisted request totals after DB query failure");
        cached.request_totals
    } else {
        warn!("No cached persisted request totals available; returning zero values");
        PersistedRequestTotals::default()
    };

    let token_totals = if let Some(token_totals) = token_totals_from_db {
        token_totals
    } else if let Some(cached) = cached_totals {
        warn!("Using last known persisted token totals after token query failure");
        cached.token_totals
    } else {
        warn!("No cached persisted token totals available; returning zero values");
        PersistedTokenTotals::default()
    };

    if request_totals_from_db.is_some() || token_totals_from_db.is_some() {
        let mut updated_cache = cached_totals.unwrap_or_default();
        if let Some(request_totals) = request_totals_from_db {
            updated_cache.request_totals = request_totals;
        }
        if let Some(token_totals) = token_totals_from_db {
            updated_cache.token_totals = token_totals;
        }

        if let Ok(mut guard) = LAST_KNOWN_PERSISTED_TOTALS.write() {
            guard.insert(cache_key, updated_cache);
        } else {
            warn!("Failed to update persisted totals cache due to poisoned lock");
        }
    }

    // Bug 2: インメモリ average_response_time_ms が None の場合、
    // オンラインエンドポイントの latency_ms（DB永続化済み）から加重平均を計算
    let average_response_time_ms = summary.average_response_time_ms.or_else(|| {
        let online_endpoints: Vec<_> = endpoints
            .iter()
            .filter(|e| e.status == EndpointStatus::Online && e.latency_ms.is_some())
            .collect();
        if online_endpoints.is_empty() {
            return None;
        }
        let total: f64 = online_endpoints
            .iter()
            .map(|e| e.latency_ms.unwrap() as f64)
            .sum();
        Some((total / online_endpoints.len() as f64) as f32)
    });

    DashboardStats {
        total_nodes: summary.total_nodes,
        online_nodes: summary.online_nodes,
        pending_nodes: summary.pending_nodes,
        registering_nodes: summary.registering_nodes,
        offline_nodes: summary.offline_nodes,
        total_requests: request_totals.total_requests,
        successful_requests: request_totals.successful_requests,
        failed_requests: request_totals.failed_requests,
        total_active_requests: summary.total_active_requests,
        queued_requests: summary.queued_requests,
        average_response_time_ms,
        average_gpu_usage: summary.average_gpu_usage,
        average_gpu_memory_usage: summary.average_gpu_memory_usage,
        last_metrics_updated_at: summary.last_metrics_updated_at,
        last_registered_at,
        last_seen_at,
        openai_key_present,
        google_key_present,
        anthropic_key_present,
        total_input_tokens: token_totals.total_input_tokens,
        total_output_tokens: token_totals.total_output_tokens,
        total_tokens: token_totals.total_tokens,
    }
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
    let success_rate = if stats.total_requests > 0 {
        Some((stats.successful_requests as f64 / stats.total_requests as f64 * 100.0) as f32)
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

pub(super) async fn collect_operation_token_totals(state: &AppState) -> PersistedTokenTotals {
    match state.request_history.get_token_statistics().await {
        Ok(stats) => PersistedTokenTotals {
            total_input_tokens: stats.total_input_tokens,
            total_output_tokens: stats.total_output_tokens,
            total_tokens: stats.total_tokens,
        },
        Err(e) => {
            warn!(
                "Failed to query operation token totals from request history: {}",
                e
            );
            PersistedTokenTotals::default()
        }
    }
}

pub(super) async fn collect_capacity(
    state: &AppState,
    dashboard_endpoints: &[DashboardEndpoint],
) -> DashboardCapacity {
    let endpoints = state.endpoint_registry.list().await;
    let mut model_ids = Vec::new();
    for endpoint in dashboard_endpoints {
        match state.endpoint_registry.list_models(endpoint.id).await {
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

pub(super) async fn collect_history(state: &AppState) -> Vec<RequestHistoryPoint> {
    state.load_manager.request_history().await
}
