use super::{
    collect_action_items, count_unique_model_ids, parse_ip_alert_threshold, DashboardOperations,
};
use crate::types::endpoint::{Endpoint, EndpointStatus, EndpointType};
// 子モジュールの関数内 `use super::X` を、分割前と同じ名前で解決させる
use super::*;

mod history_query;
mod ip_alert_threshold;
mod overview_stats;
mod stats_query;

/// フォールバック計算: avg_response_time_ms が None の場合に
/// オンラインエンドポイントの latency_ms から平均値を計算するロジック
fn fallback_avg_response_time(summary_avg: Option<f32>, endpoints: &[Endpoint]) -> Option<f32> {
    summary_avg.or_else(|| {
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
    })
}
