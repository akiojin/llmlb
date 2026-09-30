//! クラウドメトリクス日次集計データベース操作
//!
//! SPEC #582 US-005: openai / google / anthropic へのクラウドプロキシ要求を
//! `cloud_request_metrics` テーブルに 1 件 1 行で保存し、UTC 日次 × provider で rollup する。
//! 日付は UTC (`YYYY-MM-DD`)。保持期間は rolling 90 days。

use sqlx::SqlitePool;

/// 日次集計の対象 provider（SPEC で固定）
pub const SUPPORTED_PROVIDERS: [&str; 3] = ["openai", "google", "anthropic"];

/// 保持期間（当日を含む UTC 日数）
pub const RETENTION_DAYS: i64 = 90;

/// provider / UTC 日単位の集計行（export の 1 行）
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CloudMetricsDailyRow {
    /// UTC 日付（YYYY-MM-DD）
    pub date: String,
    /// provider 名（openai / google / anthropic）
    pub provider: String,
    /// 合計リクエスト数
    pub request_count: u64,
    /// 成功（2xx）リクエスト数
    pub success_count: u64,
    /// 失敗（2xx 以外）リクエスト数
    pub error_count: u64,
    /// 平均レイテンシ（ミリ秒、小数第2位で丸め）
    pub avg_latency_ms: f64,
    /// p95 レイテンシ（ミリ秒、nearest-rank 法）
    pub p95_latency_ms: u64,
}

/// SPEC 対象の provider かどうか
pub fn is_supported_provider(provider: &str) -> bool {
    SUPPORTED_PROVIDERS.contains(&provider)
}

/// クラウド要求 1 件を記録する。対象外 provider は記録しない。
pub async fn insert_sample(
    pool: &SqlitePool,
    date: &str,
    provider: &str,
    status_code: u16,
    latency_ms: u64,
) -> Result<(), sqlx::Error> {
    if !is_supported_provider(provider) {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO cloud_request_metrics (date, provider, status_code, latency_ms) \
         VALUES (?, ?, ?, ?)",
    )
    .bind(date)
    .bind(provider)
    .bind(i64::from(status_code))
    .bind(i64::try_from(latency_ms).unwrap_or(i64::MAX))
    .execute(pool)
    .await?;
    Ok(())
}

/// `oldest_kept_date` より前の日付のサンプルを削除し、削除件数を返す
pub async fn prune_before(pool: &SqlitePool, oldest_kept_date: &str) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM cloud_request_metrics WHERE date < ?")
        .bind(oldest_kept_date)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

/// `from_date` 以降のサンプルを provider / 日単位に集計する（date 昇順 → provider 昇順）
pub async fn daily_rollup(
    pool: &SqlitePool,
    from_date: &str,
) -> Result<Vec<CloudMetricsDailyRow>, sqlx::Error> {
    let samples: Vec<(String, String, i64, i64)> = sqlx::query_as(
        "SELECT date, provider, status_code, latency_ms FROM cloud_request_metrics \
         WHERE date >= ? ORDER BY date, provider, latency_ms",
    )
    .bind(from_date)
    .fetch_all(pool)
    .await?;

    let mut rows = Vec::new();
    let mut group: Vec<(i64, i64)> = Vec::new();
    let mut key: Option<(String, String)> = None;
    for (date, provider, status_code, latency_ms) in samples {
        if key.as_ref() != Some(&(date.clone(), provider.clone())) {
            if let Some((date, provider)) = key.take() {
                rows.push(summarize(date, provider, &group));
            }
            group.clear();
            key = Some((date, provider));
        }
        group.push((status_code, latency_ms));
    }
    if let Some((date, provider)) = key {
        rows.push(summarize(date, provider, &group));
    }
    Ok(rows)
}

/// 1 グループ（latency 昇順ソート済み）を集計行にまとめる
fn summarize(date: String, provider: String, samples: &[(i64, i64)]) -> CloudMetricsDailyRow {
    let request_count = samples.len() as u64;
    let success_count = samples
        .iter()
        .filter(|(status, _)| (200..300).contains(status))
        .count() as u64;
    let total_latency: f64 = samples.iter().map(|(_, latency)| *latency as f64).sum();
    let avg_latency_ms = (total_latency / request_count as f64 * 100.0).round() / 100.0;
    // nearest-rank: ceil(0.95 * n) 番目（1-origin）
    let rank = (samples.len() * 95).div_ceil(100).max(1);
    let p95_latency_ms = samples[rank - 1].1.max(0) as u64;

    CloudMetricsDailyRow {
        date,
        provider,
        request_count,
        success_count,
        error_count: request_count - success_count,
        avg_latency_ms,
        p95_latency_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_utils::test_db_pool;

    #[tokio::test]
    async fn insert_sample_ignores_unsupported_provider() {
        let pool = test_db_pool().await;
        insert_sample(&pool, "2026-01-01", "local", 200, 10)
            .await
            .unwrap();
        insert_sample(&pool, "2026-01-01", "openai", 200, 10)
            .await
            .unwrap();

        let rows = daily_rollup(&pool, "2026-01-01").await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].provider, "openai");
    }

    #[tokio::test]
    async fn daily_rollup_groups_by_date_and_provider() {
        let pool = test_db_pool().await;
        for (date, provider, status, latency) in [
            ("2026-01-02", "openai", 200, 30),
            ("2026-01-01", "openai", 200, 10),
            ("2026-01-01", "openai", 503, 20),
            ("2026-01-01", "google", 200, 5),
        ] {
            insert_sample(&pool, date, provider, status, latency)
                .await
                .unwrap();
        }

        let rows = daily_rollup(&pool, "2026-01-01").await.unwrap();
        let keys: Vec<(&str, &str, u64, u64)> = rows
            .iter()
            .map(|r| {
                (
                    r.date.as_str(),
                    r.provider.as_str(),
                    r.request_count,
                    r.error_count,
                )
            })
            .collect();
        assert_eq!(
            keys,
            vec![
                ("2026-01-01", "google", 1, 0),
                ("2026-01-01", "openai", 2, 1),
                ("2026-01-02", "openai", 1, 0),
            ]
        );
        assert_eq!(rows[1].avg_latency_ms, 15.0);
        assert_eq!(rows[1].p95_latency_ms, 20);
    }

    #[tokio::test]
    async fn daily_rollup_excludes_dates_before_from_date() {
        let pool = test_db_pool().await;
        insert_sample(&pool, "2026-01-01", "openai", 200, 10)
            .await
            .unwrap();
        insert_sample(&pool, "2026-01-03", "openai", 200, 10)
            .await
            .unwrap();

        let rows = daily_rollup(&pool, "2026-01-02").await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].date, "2026-01-03");
    }

    #[tokio::test]
    async fn prune_before_deletes_only_older_dates() {
        let pool = test_db_pool().await;
        for date in ["2026-01-01", "2026-01-02", "2026-01-03"] {
            insert_sample(&pool, date, "anthropic", 200, 10)
                .await
                .unwrap();
        }

        let deleted = prune_before(&pool, "2026-01-02").await.unwrap();

        assert_eq!(deleted, 1);
        let rows = daily_rollup(&pool, "2000-01-01").await.unwrap();
        let dates: Vec<&str> = rows.iter().map(|r| r.date.as_str()).collect();
        assert_eq!(dates, vec!["2026-01-02", "2026-01-03"]);
    }

    #[test]
    fn summarize_p95_uses_nearest_rank() {
        let samples: Vec<(i64, i64)> = (1..=100).map(|l| (200, l)).collect();
        let row = summarize("d".into(), "openai".into(), &samples);
        assert_eq!(row.p95_latency_ms, 95);
        assert_eq!(row.avg_latency_ms, 50.5);

        let row = summarize("d".into(), "openai".into(), &[(200, 7)]);
        assert_eq!(row.p95_latency_ms, 7);
    }

    #[test]
    fn summarize_counts_non_2xx_as_errors() {
        let row = summarize(
            "d".into(),
            "google".into(),
            &[(200, 1), (204, 1), (301, 1), (429, 1), (500, 1)],
        );
        assert_eq!(row.success_count, 2);
        assert_eq!(row.error_count, 3);
    }
}
