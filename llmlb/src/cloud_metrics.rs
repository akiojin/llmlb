use axum::{
    extract::{Query, State},
    http::header,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::{Duration, NaiveDate, Utc};
use once_cell::sync::Lazy;
use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry, TextEncoder,
};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::api::error::AppError;
use crate::common::error::{CommonError, LbError};
use crate::db::cloud_metrics::{self as store, CloudMetricsDailyRow, RETENTION_DAYS};
use crate::AppState;

static REGISTRY: Lazy<Registry> = Lazy::new(Registry::new);
static COUNTER: Lazy<IntCounterVec> = Lazy::new(|| {
    let opts = Opts::new("cloud_requests_total", "Cloud LLM requests");
    IntCounterVec::new(opts, &["provider", "status"]).expect("counter vec")
});
static HISTO: Lazy<HistogramVec> = Lazy::new(|| {
    let opts = HistogramOpts::new(
        "cloud_request_latency_seconds",
        "Cloud LLM request latency (seconds)",
    );
    HistogramVec::new(opts, &["provider"]).expect("histogram vec")
});

/// Register cloud metrics (idempotent).
pub fn init_metrics() {
    REGISTRY.register(Box::new(COUNTER.clone())).ok();
    REGISTRY.register(Box::new(HISTO.clone())).ok();
}

/// Record a cloud provider request with status and latency (ms).
pub fn record(provider: &str, status_code: u16, latency_ms: u128) {
    // ensure registry setup
    init_metrics();
    let status_str = status_code.to_string();
    COUNTER
        .with_label_values(&[provider, status_str.as_str()])
        .inc();
    let secs = latency_ms as f64 / 1000.0;
    HISTO.with_label_values(&[provider]).observe(secs);
}

/// Expose Prometheus text format for cloud metrics.
pub async fn export_metrics() -> impl IntoResponse {
    init_metrics();
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buf = Vec::new();
    let res = if let Err(e) = encoder.encode(&metric_families, &mut buf) {
        axum::response::Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(header::CONTENT_TYPE, "text/plain")
            .body(axum::body::Body::from(format!("encode error: {e}")))
            .unwrap()
    } else {
        let body = String::from_utf8(buf).unwrap_or_default();
        axum::response::Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/plain")
            .body(axum::body::Body::from(body))
            .unwrap()
    };
    res
}

/// retention prune の実行間隔
const RETENTION_PRUNE_INTERVAL_SECS: u64 = 3600;

/// CSV export のヘッダー（export row fields の順序を固定する）
const CSV_HEADER: &str =
    "date,provider,request_count,success_count,error_count,avg_latency_ms,p95_latency_ms";

/// クラウド要求を UTC 日次 rollup 用に永続化する（fire-and-forget）。
pub fn record_daily(pool: &SqlitePool, provider: &str, status_code: u16, latency_ms: u128) {
    if !store::is_supported_provider(provider) {
        return;
    }
    let pool = pool.clone();
    let provider = provider.to_string();
    let date = utc_date_string(Utc::now().date_naive());
    let latency_ms = u64::try_from(latency_ms).unwrap_or(u64::MAX);
    tokio::spawn(async move {
        if let Err(e) = store::insert_sample(&pool, &date, &provider, status_code, latency_ms).await
        {
            tracing::error!(provider = %provider, "Failed to record cloud metrics sample: {}", e);
        }
    });
}

/// 90 日 rolling retention を適用する（保持期間外の日付を削除）。
pub async fn prune_expired(pool: &SqlitePool, today: NaiveDate) -> Result<u64, sqlx::Error> {
    store::prune_before(pool, &window_start(today, RETENTION_DAYS)).await
}

/// 起動時と 1 時間ごとに retention prune を実行するバックグラウンドタスクを開始する。
pub fn start_retention_task(pool: SqlitePool) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(
            RETENTION_PRUNE_INTERVAL_SECS,
        ));
        loop {
            interval.tick().await;
            match prune_expired(&pool, Utc::now().date_naive()).await {
                Ok(0) => {}
                Ok(count) => tracing::info!(count, "Pruned expired cloud metrics samples"),
                Err(e) => tracing::error!("Cloud metrics retention prune failed: {}", e),
            }
        }
    });
}

/// `GET /api/metrics/cloud/export` のクエリ
#[derive(Debug, Deserialize)]
pub struct CloudMetricsExportQuery {
    /// `json`（既定）または `csv`
    pub format: Option<String>,
    /// 当日を含む UTC 日数（1..=90、既定 90）
    pub days: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportFormat {
    Json,
    Csv,
}

impl CloudMetricsExportQuery {
    fn validate(&self) -> Result<(ExportFormat, i64), String> {
        let format = match self.format.as_deref().unwrap_or("json") {
            "json" => ExportFormat::Json,
            "csv" => ExportFormat::Csv,
            other => return Err(format!("format must be json or csv (got {other})")),
        };
        let days = match self.days.as_deref() {
            None => RETENTION_DAYS,
            Some(raw) => raw
                .parse::<i64>()
                .ok()
                .filter(|d| (1..=RETENTION_DAYS).contains(d))
                .ok_or_else(|| {
                    format!("days must be an integer between 1 and {RETENTION_DAYS} (got {raw})")
                })?,
        };
        Ok((format, days))
    }
}

/// provider / UTC 日単位のクラウドメトリクスを JSON または CSV で export する。
pub async fn export_daily_metrics(
    State(state): State<AppState>,
    Query(query): Query<CloudMetricsExportQuery>,
) -> Result<Response, AppError> {
    let (format, days) = query
        .validate()
        .map_err(|msg| AppError::from(LbError::Common(CommonError::Validation(msg))))?;

    let today = Utc::now().date_naive();
    let db_error = |e: sqlx::Error| AppError::from(LbError::Database(e.to_string()));
    prune_expired(&state.db_pool, today)
        .await
        .map_err(db_error)?;
    let rows = store::daily_rollup(&state.db_pool, &window_start(today, days))
        .await
        .map_err(db_error)?;

    let (content_type, filename, body) = match format {
        ExportFormat::Json => (
            "application/json",
            "cloud_metrics.json",
            serde_json::to_string(&rows)
                .map_err(|e| AppError::from(LbError::Internal(e.to_string())))?,
        ),
        ExportFormat::Csv => (
            "text/csv; charset=utf-8",
            "cloud_metrics.csv",
            to_csv(&rows),
        ),
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .body(axum::body::Body::from(body))
        .expect("valid export response"))
}

/// export 行を CSV（ヘッダー付き）に変換する
fn to_csv(rows: &[CloudMetricsDailyRow]) -> String {
    let mut out = String::from(CSV_HEADER);
    out.push('\n');
    for row in rows {
        out.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            row.date,
            row.provider,
            row.request_count,
            row.success_count,
            row.error_count,
            row.avg_latency_ms,
            row.p95_latency_ms
        ));
    }
    out
}

/// 当日を含む `days` 日間の開始日（UTC）
fn window_start(today: NaiveDate, days: i64) -> String {
    utc_date_string(today - Duration::days(days - 1))
}

fn utc_date_string(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(format: Option<&str>, days: Option<&str>) -> CloudMetricsExportQuery {
        CloudMetricsExportQuery {
            format: format.map(str::to_string),
            days: days.map(str::to_string),
        }
    }

    #[test]
    fn export_query_defaults_to_json_and_retention_window() {
        assert_eq!(
            query(None, None).validate(),
            Ok((ExportFormat::Json, RETENTION_DAYS))
        );
        assert_eq!(
            query(Some("csv"), Some("1")).validate(),
            Ok((ExportFormat::Csv, 1))
        );
    }

    #[test]
    fn export_query_rejects_out_of_range_or_invalid_values() {
        for (format, days) in [
            (None, Some("0")),
            (None, Some("91")),
            (None, Some("-3")),
            (None, Some("7.5")),
            (Some("xml"), Some("7")),
        ] {
            assert!(
                query(format, days).validate().is_err(),
                "{format:?} {days:?}"
            );
        }
    }

    #[test]
    fn window_start_includes_today() {
        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        assert_eq!(window_start(today, 1), "2026-03-10");
        assert_eq!(window_start(today, 90), "2025-12-11");
    }

    #[tokio::test]
    async fn prune_expired_keeps_exactly_90_days() {
        let pool = crate::db::test_utils::test_db_pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        for days_ago in [0, 89, 90] {
            let date = utc_date_string(today - Duration::days(days_ago));
            store::insert_sample(&pool, &date, "openai", 200, 1)
                .await
                .unwrap();
        }

        assert_eq!(prune_expired(&pool, today).await.unwrap(), 1);
        let rows = store::daily_rollup(&pool, "2000-01-01").await.unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn to_csv_writes_header_and_rows_in_field_order() {
        let csv = to_csv(&[CloudMetricsDailyRow {
            date: "2026-03-10".into(),
            provider: "google".into(),
            request_count: 3,
            success_count: 2,
            error_count: 1,
            avg_latency_ms: 12.25,
            p95_latency_ms: 20,
        }]);
        assert_eq!(
            csv,
            format!("{CSV_HEADER}\n2026-03-10,google,3,2,1,12.25,20\n")
        );
    }

    #[test]
    fn record_and_export() {
        record("openai", StatusCode::OK.as_u16(), 123);
        let encoder = TextEncoder::new();
        let metric_families = REGISTRY.gather();
        let mut buf = Vec::new();
        encoder.encode(&metric_families, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("cloud_requests_total"));
        assert!(out.contains("cloud_request_latency_seconds"));
    }

    #[test]
    fn record_multiple_providers() {
        record("openai", 200, 100);
        record("google", 200, 150);
        record("anthropic", 200, 200);
        let encoder = TextEncoder::new();
        let metric_families = REGISTRY.gather();
        let mut buf = Vec::new();
        encoder.encode(&metric_families, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("openai"));
        assert!(out.contains("google"));
        assert!(out.contains("anthropic"));
    }

    #[test]
    fn record_various_status_codes() {
        record("openai", 200, 50);
        record("openai", 400, 25);
        record("openai", 500, 30);
        let encoder = TextEncoder::new();
        let metric_families = REGISTRY.gather();
        let mut buf = Vec::new();
        encoder.encode(&metric_families, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("200"));
        assert!(out.contains("400"));
        assert!(out.contains("500"));
    }

    #[test]
    fn init_metrics_is_idempotent() {
        init_metrics();
        init_metrics();
        init_metrics();
        // No panic means success
    }

    #[tokio::test]
    async fn export_metrics_returns_text_plain() {
        record("test", 200, 10);
        let response = export_metrics().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response.headers().get(header::CONTENT_TYPE);
        assert!(content_type.is_some());
        assert!(content_type
            .unwrap()
            .to_str()
            .unwrap()
            .contains("text/plain"));
    }

    #[test]
    fn latency_conversion_ms_to_seconds() {
        record("latency_test", 200, 1500);
        let encoder = TextEncoder::new();
        let metric_families = REGISTRY.gather();
        let mut buf = Vec::new();
        encoder.encode(&metric_families, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        // 1500ms = 1.5s, bucket should contain this value
        assert!(out.contains("cloud_request_latency_seconds"));
    }
}
