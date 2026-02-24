//! エンドポイント日次統計データベース操作
//!
//! SPEC-8c32349f: エンドポイント単位リクエスト統計
//! endpoint_daily_stats テーブルへのCRUD操作を提供する。
//! 日付はサーバーローカル時間 (chrono::Local::now().format("%Y-%m-%d").to_string()) を使用。

use sqlx::SqlitePool;
use uuid::Uuid;

/// 日次集計エントリ（日付ごとの集計結果）
#[derive(Debug, Clone, serde::Serialize)]
pub struct DailyStatEntry {
    /// 日付（YYYY-MM-DD形式）
    pub date: String,
    /// 合計リクエスト数
    pub total_requests: i64,
    /// 成功リクエスト数
    pub successful_requests: i64,
    /// 失敗リクエスト数
    pub failed_requests: i64,
}

/// モデル別集計エントリ（モデルごとの集計結果）
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelStatEntry {
    /// モデルID
    pub model_id: String,
    /// 合計リクエスト数
    pub total_requests: i64,
    /// 成功リクエスト数
    pub successful_requests: i64,
    /// 失敗リクエスト数
    pub failed_requests: i64,
    /// 出力トークン累計（SPEC-4bb5b55f）
    pub total_output_tokens: i64,
    /// 処理時間累計（ミリ秒、SPEC-4bb5b55f）
    pub total_duration_ms: i64,
}

// --- Internal Row Types ---

#[derive(sqlx::FromRow)]
struct DailyStatRow {
    date: String,
    total_requests: i64,
    successful_requests: i64,
    failed_requests: i64,
}

impl From<DailyStatRow> for DailyStatEntry {
    fn from(row: DailyStatRow) -> Self {
        DailyStatEntry {
            date: row.date,
            total_requests: row.total_requests,
            successful_requests: row.successful_requests,
            failed_requests: row.failed_requests,
        }
    }
}

#[derive(sqlx::FromRow)]
struct ModelStatRow {
    model_id: String,
    total_requests: i64,
    successful_requests: i64,
    failed_requests: i64,
    total_output_tokens: i64,
    total_duration_ms: i64,
}

impl From<ModelStatRow> for ModelStatEntry {
    fn from(row: ModelStatRow) -> Self {
        ModelStatEntry {
            model_id: row.model_id,
            total_requests: row.total_requests,
            successful_requests: row.successful_requests,
            failed_requests: row.failed_requests,
            total_output_tokens: row.total_output_tokens,
            total_duration_ms: row.total_duration_ms,
        }
    }
}

/// 日次統計をUPSERT（挿入または更新）
///
/// 指定のエンドポイント・モデル・日付の組み合わせでレコードが存在しない場合は新規挿入、
/// 存在する場合はカウンタをインクリメントする。
pub async fn upsert_daily_stats(
    pool: &SqlitePool,
    endpoint_id: Uuid,
    model_id: &str,
    date: &str,
    success: bool,
    output_tokens: u64,
    duration_ms: u64,
) -> Result<(), sqlx::Error> {
    upsert_daily_stats_with_api_kind(
        pool,
        endpoint_id,
        model_id,
        date,
        "chat_completions",
        success,
        output_tokens,
        duration_ms,
    )
    .await
}

/// api_kind指定付きの日次統計UPSERT
#[allow(clippy::too_many_arguments)]
pub async fn upsert_daily_stats_with_api_kind(
    pool: &SqlitePool,
    endpoint_id: Uuid,
    model_id: &str,
    date: &str,
    api_kind: &str,
    success: bool,
    output_tokens: u64,
    duration_ms: u64,
) -> Result<(), sqlx::Error> {
    let success_increment: i64 = if success { 1 } else { 0 };
    let failure_increment: i64 = if success { 0 } else { 1 };

    sqlx::query(
        r#"
        INSERT INTO endpoint_daily_stats (endpoint_id, model_id, date, api_kind, total_requests, successful_requests, failed_requests, total_output_tokens, total_duration_ms)
        VALUES (?, ?, ?, ?, 1, ?, ?, ?, ?)
        ON CONFLICT(endpoint_id, model_id, date, api_kind) DO UPDATE SET
            total_requests = total_requests + 1,
            successful_requests = successful_requests + excluded.successful_requests,
            failed_requests = failed_requests + excluded.failed_requests,
            total_output_tokens = total_output_tokens + excluded.total_output_tokens,
            total_duration_ms = total_duration_ms + excluded.total_duration_ms
        "#,
    )
    .bind(endpoint_id.to_string())
    .bind(model_id)
    .bind(date)
    .bind(api_kind)
    .bind(success_increment)
    .bind(failure_increment)
    .bind(output_tokens as i64)
    .bind(duration_ms as i64)
    .execute(pool)
    .await?;

    Ok(())
}

/// 日次集計データを取得（期間指定）
///
/// 指定エンドポイントの直近N日分の日次データを日付昇順で返す。
/// 全モデルの合計値を日付ごとに集計する。
/// 日付はサーバーローカル時間で計算（書き込み時と一致）。
pub async fn get_daily_stats(
    pool: &SqlitePool,
    endpoint_id: Uuid,
    days: u32,
) -> Result<Vec<DailyStatEntry>, sqlx::Error> {
    let days = days.max(1);
    let start_date = (chrono::Local::now() - chrono::Duration::days((days - 1) as i64))
        .format("%Y-%m-%d")
        .to_string();

    let rows = sqlx::query_as::<_, DailyStatRow>(
        r#"
        SELECT
            date,
            SUM(total_requests) AS total_requests,
            SUM(successful_requests) AS successful_requests,
            SUM(failed_requests) AS failed_requests
        FROM endpoint_daily_stats
        WHERE endpoint_id = ?
          AND date >= ?
        GROUP BY date
        ORDER BY date ASC
        "#,
    )
    .bind(endpoint_id.to_string())
    .bind(&start_date)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

/// モデル別集計データを取得
///
/// 指定エンドポイントのモデル別累計統計を返す。
/// 全日付を通じた合計値をモデルごとに集計し、リクエスト数の降順で返す。
pub async fn get_model_stats(
    pool: &SqlitePool,
    endpoint_id: Uuid,
) -> Result<Vec<ModelStatEntry>, sqlx::Error> {
    let rows = sqlx::query_as::<_, ModelStatRow>(
        r#"
        SELECT
            model_id,
            SUM(total_requests) AS total_requests,
            SUM(successful_requests) AS successful_requests,
            SUM(failed_requests) AS failed_requests,
            SUM(total_output_tokens) AS total_output_tokens,
            SUM(total_duration_ms) AS total_duration_ms
        FROM endpoint_daily_stats
        WHERE endpoint_id = ?
        GROUP BY model_id
        ORDER BY total_requests DESC
        "#,
    )
    .bind(endpoint_id.to_string())
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

/// 全エンドポイント横断のモデル別集計データを取得
///
/// endpoint_daily_stats テーブルの全エンドポイントを通じた
/// モデル別累計統計を返す。リクエスト数の降順で返す。
pub async fn get_all_model_stats(pool: &SqlitePool) -> Result<Vec<ModelStatEntry>, sqlx::Error> {
    let rows = sqlx::query_as::<_, ModelStatRow>(
        r#"
        SELECT
            model_id,
            SUM(total_requests) AS total_requests,
            SUM(successful_requests) AS successful_requests,
            SUM(failed_requests) AS failed_requests,
            SUM(total_output_tokens) AS total_output_tokens,
            SUM(total_duration_ms) AS total_duration_ms
        FROM endpoint_daily_stats
        GROUP BY model_id
        ORDER BY total_requests DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

/// 当日の集計データを取得
///
/// 指定エンドポイントの指定日付のデータを返す。
/// 全モデルの合計値を日付で集計し、単一のDailyStatEntryとして返す。
/// データが存在しない場合はカウンタ0のエントリを返す。
pub async fn get_today_stats(
    pool: &SqlitePool,
    endpoint_id: Uuid,
    today: &str,
) -> Result<DailyStatEntry, sqlx::Error> {
    let row = sqlx::query_as::<_, DailyStatRow>(
        r#"
        SELECT
            date,
            SUM(total_requests) AS total_requests,
            SUM(successful_requests) AS successful_requests,
            SUM(failed_requests) AS failed_requests
        FROM endpoint_daily_stats
        WHERE endpoint_id = ?
          AND date = ?
        GROUP BY date
        "#,
    )
    .bind(endpoint_id.to_string())
    .bind(today)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into()).unwrap_or(DailyStatEntry {
        date: today.to_string(),
        total_requests: 0,
        successful_requests: 0,
        failed_requests: 0,
    }))
}

/// 当日の全エンドポイントのTPS関連データを取得（起動時seeding用）
pub async fn get_today_stats_all(
    pool: &SqlitePool,
    today: &str,
) -> Result<Vec<TpsSeedEntry>, sqlx::Error> {
    let rows = sqlx::query_as::<_, TpsSeedRow>(
        r#"
        SELECT
            endpoint_id,
            model_id,
            api_kind,
            total_output_tokens,
            total_duration_ms,
            successful_requests
        FROM endpoint_daily_stats
        WHERE date = ?
          AND total_output_tokens > 0
          AND total_duration_ms > 0
        "#,
    )
    .bind(today)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

/// TPS seeding用のエントリ
#[derive(Debug, Clone)]
pub struct TpsSeedEntry {
    /// エンドポイントID
    pub endpoint_id: Uuid,
    /// モデルID
    pub model_id: String,
    /// API種別（chat_completions/completions/responses）
    pub api_kind: String,
    /// 出力トークン累計
    pub total_output_tokens: i64,
    /// 処理時間累計（ミリ秒）
    pub total_duration_ms: i64,
    /// TPS対象リクエスト数（成功リクエストのみ）
    pub successful_requests: i64,
}

#[derive(sqlx::FromRow)]
struct TpsSeedRow {
    endpoint_id: String,
    model_id: String,
    api_kind: String,
    total_output_tokens: i64,
    total_duration_ms: i64,
    successful_requests: i64,
}

impl From<TpsSeedRow> for TpsSeedEntry {
    fn from(row: TpsSeedRow) -> Self {
        TpsSeedEntry {
            endpoint_id: Uuid::parse_str(&row.endpoint_id).unwrap_or_default(),
            model_id: row.model_id,
            api_kind: row.api_kind,
            total_output_tokens: row.total_output_tokens,
            total_duration_ms: row.total_duration_ms,
            successful_requests: row.successful_requests,
        }
    }
}

/// 日次統計バッチタスクを開始（SPEC-8c32349f）
///
/// サーバーローカル時間の0:00に前日分の統計をログ出力する。
/// リアルタイムUPSERTで統計は更新済みのため、
/// このタスクは日次マーカーとログ記録の役割を担う。
pub fn start_daily_stats_task(pool: SqlitePool) {
    tokio::spawn(async move {
        loop {
            // 次の0:00までスリープ
            let now = chrono::Local::now();
            let tomorrow = (now + chrono::Duration::days(1))
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .expect("valid midnight time");
            let tomorrow = tomorrow
                .and_local_timezone(chrono::Local)
                .single()
                .unwrap_or_else(|| {
                    (now + chrono::Duration::days(1))
                        .date_naive()
                        .and_hms_opt(0, 0, 1)
                        .expect("valid midnight+1s")
                        .and_local_timezone(chrono::Local)
                        .latest()
                        .expect("valid local time")
                });
            let sleep_duration = (tomorrow - now).to_std().unwrap_or_default();
            tokio::time::sleep(sleep_duration).await;

            let yesterday = (chrono::Local::now() - chrono::Duration::days(1))
                .format("%Y-%m-%d")
                .to_string();
            tracing::info!("Daily stats batch: finalizing {}", yesterday);

            // 前日分のレコード数をログ出力
            match sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM endpoint_daily_stats WHERE date = ?",
            )
            .bind(&yesterday)
            .fetch_one(&pool)
            .await
            {
                Ok(count) => {
                    tracing::info!(
                        "Daily stats batch complete: {} records for {}",
                        count,
                        yesterday
                    );
                }
                Err(e) => {
                    tracing::error!("Daily stats batch failed: {}", e);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_utils::TEST_LOCK;

    async fn setup_test_db() -> SqlitePool {
        crate::db::test_utils::test_db_pool().await
    }

    #[tokio::test]
    async fn test_upsert_daily_stats_new_record() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;

        let endpoint_id = Uuid::new_v4();
        let model_id = "llama3:8b";
        let date = "2025-01-15";

        // 成功リクエストを1件挿入
        upsert_daily_stats(&pool, endpoint_id, model_id, date, true, 0, 0)
            .await
            .unwrap();

        // 挿入されたレコードを確認
        let stats = get_today_stats(&pool, endpoint_id, date).await.unwrap();
        assert_eq!(stats.date, date);
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.successful_requests, 1);
        assert_eq!(stats.failed_requests, 0);

        // 失敗リクエストを別のモデルで1件挿入
        let model_id_2 = "gpt-4";
        upsert_daily_stats(&pool, endpoint_id, model_id_2, date, false, 0, 0)
            .await
            .unwrap();

        // 日付でグループ化して取得（2モデル合計）
        let stats = get_today_stats(&pool, endpoint_id, date).await.unwrap();
        assert_eq!(stats.total_requests, 2);
        assert_eq!(stats.successful_requests, 1);
        assert_eq!(stats.failed_requests, 1);
    }

    #[tokio::test]
    async fn test_upsert_daily_stats_increment() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;

        let endpoint_id = Uuid::new_v4();
        let model_id = "llama3:8b";
        let date = "2025-01-15";

        // 同一キーで複数回upsert
        upsert_daily_stats(&pool, endpoint_id, model_id, date, true, 0, 0)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_id, date, true, 0, 0)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_id, date, false, 0, 0)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_id, date, true, 0, 0)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_id, date, false, 0, 0)
            .await
            .unwrap();

        // カウンタが累積されていることを確認
        let stats = get_today_stats(&pool, endpoint_id, date).await.unwrap();
        assert_eq!(stats.total_requests, 5);
        assert_eq!(stats.successful_requests, 3);
        assert_eq!(stats.failed_requests, 2);
    }

    #[tokio::test]
    async fn test_get_daily_stats() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;

        let endpoint_id = Uuid::new_v4();
        let model_a = "llama3:8b";
        let model_b = "gpt-4";

        // 複数日にわたるデータを挿入
        upsert_daily_stats(&pool, endpoint_id, model_a, "2025-01-13", true, 0, 0)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_a, "2025-01-14", true, 0, 0)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_b, "2025-01-14", false, 0, 0)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_a, "2025-01-15", true, 0, 0)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_a, "2025-01-15", true, 0, 0)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_b, "2025-01-15", false, 0, 0)
            .await
            .unwrap();

        // 十分な日数で全データを取得（SQLiteのdate()関数は'now'基準のため、
        // テストデータが確実に範囲内に入るよう大きな値を指定）
        let stats = get_daily_stats(&pool, endpoint_id, 36500).await.unwrap();

        // 3日分のデータが日付昇順で返る
        assert_eq!(stats.len(), 3);

        // 2025-01-13: model_a成功1件
        assert_eq!(stats[0].date, "2025-01-13");
        assert_eq!(stats[0].total_requests, 1);
        assert_eq!(stats[0].successful_requests, 1);
        assert_eq!(stats[0].failed_requests, 0);

        // 2025-01-14: model_a成功1件 + model_b失敗1件
        assert_eq!(stats[1].date, "2025-01-14");
        assert_eq!(stats[1].total_requests, 2);
        assert_eq!(stats[1].successful_requests, 1);
        assert_eq!(stats[1].failed_requests, 1);

        // 2025-01-15: model_a成功2件 + model_b失敗1件
        assert_eq!(stats[2].date, "2025-01-15");
        assert_eq!(stats[2].total_requests, 3);
        assert_eq!(stats[2].successful_requests, 2);
        assert_eq!(stats[2].failed_requests, 1);

        // 別のエンドポイントでは空結果
        let other_id = Uuid::new_v4();
        let empty = get_daily_stats(&pool, other_id, 36500).await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn test_get_model_stats() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;

        let endpoint_id = Uuid::new_v4();
        let model_a = "llama3:8b";
        let model_b = "gpt-4";
        let model_c = "mistral:7b";

        // 複数モデル・複数日にわたるデータを挿入
        // model_a: 合計5件（成功4、失敗1）
        upsert_daily_stats(&pool, endpoint_id, model_a, "2025-01-14", true, 0, 0)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_a, "2025-01-14", true, 0, 0)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_a, "2025-01-15", true, 0, 0)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_a, "2025-01-15", true, 0, 0)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_a, "2025-01-15", false, 0, 0)
            .await
            .unwrap();

        // model_b: 合計3件（成功1、失敗2）
        upsert_daily_stats(&pool, endpoint_id, model_b, "2025-01-14", false, 0, 0)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_b, "2025-01-15", true, 0, 0)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_b, "2025-01-15", false, 0, 0)
            .await
            .unwrap();

        // model_c: 合計1件（成功1）
        upsert_daily_stats(&pool, endpoint_id, model_c, "2025-01-15", true, 0, 0)
            .await
            .unwrap();

        let stats = get_model_stats(&pool, endpoint_id).await.unwrap();

        // total_requests降順で3モデル
        assert_eq!(stats.len(), 3);

        // model_a が最多
        assert_eq!(stats[0].model_id, model_a);
        assert_eq!(stats[0].total_requests, 5);
        assert_eq!(stats[0].successful_requests, 4);
        assert_eq!(stats[0].failed_requests, 1);

        // model_b が次
        assert_eq!(stats[1].model_id, model_b);
        assert_eq!(stats[1].total_requests, 3);
        assert_eq!(stats[1].successful_requests, 1);
        assert_eq!(stats[1].failed_requests, 2);

        // model_c が最少
        assert_eq!(stats[2].model_id, model_c);
        assert_eq!(stats[2].total_requests, 1);
        assert_eq!(stats[2].successful_requests, 1);
        assert_eq!(stats[2].failed_requests, 0);

        // 別のエンドポイントでは空結果
        let other_id = Uuid::new_v4();
        let empty = get_model_stats(&pool, other_id).await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn test_get_today_stats() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;

        let endpoint_id = Uuid::new_v4();
        let today = "2025-01-15";

        // データがない場合はカウンタ0のエントリが返る
        let stats = get_today_stats(&pool, endpoint_id, today).await.unwrap();
        assert_eq!(stats.date, today);
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.successful_requests, 0);
        assert_eq!(stats.failed_requests, 0);

        // データを挿入
        upsert_daily_stats(&pool, endpoint_id, "llama3:8b", today, true, 0, 0)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, "llama3:8b", today, true, 0, 0)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, "gpt-4", today, false, 0, 0)
            .await
            .unwrap();

        // 当日の集計を取得（全モデル合計）
        let stats = get_today_stats(&pool, endpoint_id, today).await.unwrap();
        assert_eq!(stats.date, today);
        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.successful_requests, 2);
        assert_eq!(stats.failed_requests, 1);

        // 別の日付ではカウンタ0
        let other_date = "2025-01-16";
        let stats = get_today_stats(&pool, endpoint_id, other_date)
            .await
            .unwrap();
        assert_eq!(stats.date, other_date);
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.successful_requests, 0);
        assert_eq!(stats.failed_requests, 0);
    }

    // SPEC-4bb5b55f T003: upsert_daily_statsにoutput_tokens/duration_msが累積加算されるテスト

    #[tokio::test]
    async fn test_upsert_daily_stats_tps_accumulation() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;

        let endpoint_id = Uuid::new_v4();
        let model_id = "llama3:8b";
        let date = "2025-01-20";

        // output_tokens=100, duration_ms=2000 で1件目
        upsert_daily_stats(&pool, endpoint_id, model_id, date, true, 100, 2000)
            .await
            .unwrap();

        // output_tokens=200, duration_ms=3000 で2件目（累積加算される）
        upsert_daily_stats(&pool, endpoint_id, model_id, date, true, 200, 3000)
            .await
            .unwrap();

        // DBから直接クエリして累積値を検証
        let row = sqlx::query_as::<_, (i64, i64)>(
            "SELECT total_output_tokens, total_duration_ms FROM endpoint_daily_stats WHERE endpoint_id = ? AND model_id = ? AND date = ?"
        )
        .bind(endpoint_id.to_string())
        .bind(model_id)
        .bind(date)
        .fetch_one(&pool)
        .await
        .expect("TPS columns should exist after migration 016");

        // 累積加算: 100+200=300, 2000+3000=5000
        assert_eq!(row.0, 300, "total_output_tokens should be 300");
        assert_eq!(row.1, 5000, "total_duration_ms should be 5000");

        // 既存のリクエストカウント動作に影響しないことを確認
        let stats = get_today_stats(&pool, endpoint_id, date).await.unwrap();
        assert_eq!(stats.total_requests, 2);
        assert_eq!(stats.successful_requests, 2);
        assert_eq!(stats.failed_requests, 0);
    }

    #[tokio::test]
    async fn test_upsert_daily_stats_tps_zero_values_no_impact() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;

        let endpoint_id = Uuid::new_v4();
        let model_id = "gpt-4";
        let date = "2025-01-20";

        // output_tokens=0, duration_ms=0 で挿入（既存の呼び出しパターン）
        upsert_daily_stats(&pool, endpoint_id, model_id, date, true, 0, 0)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_id, date, false, 0, 0)
            .await
            .unwrap();

        // DBから直接クエリして値が0のまま
        let row = sqlx::query_as::<_, (i64, i64)>(
            "SELECT total_output_tokens, total_duration_ms FROM endpoint_daily_stats WHERE endpoint_id = ? AND model_id = ? AND date = ?"
        )
        .bind(endpoint_id.to_string())
        .bind(model_id)
        .bind(date)
        .fetch_one(&pool)
        .await
        .expect("TPS columns should exist after migration 016");

        assert_eq!(row.0, 0, "total_output_tokens should be 0");
        assert_eq!(row.1, 0, "total_duration_ms should be 0");

        // 通常のカウントは正常
        let stats = get_today_stats(&pool, endpoint_id, date).await.unwrap();
        assert_eq!(stats.total_requests, 2);
    }

    // SPEC-4bb5b55f T008: get_model_statsにTPS情報が含まれることを検証

    #[tokio::test]
    async fn test_get_model_stats_includes_tps_data() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;

        let endpoint_id = Uuid::new_v4();
        let model_a = "llama3:8b";
        let model_b = "gpt-4";

        // model_a: 2件、tokens=300, duration=5000ms
        upsert_daily_stats(&pool, endpoint_id, model_a, "2025-01-20", true, 100, 2000)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_a, "2025-01-20", true, 200, 3000)
            .await
            .unwrap();

        // model_b: 1件、tokens=50, duration=1000ms
        upsert_daily_stats(&pool, endpoint_id, model_b, "2025-01-20", true, 50, 1000)
            .await
            .unwrap();

        let stats = get_model_stats(&pool, endpoint_id).await.unwrap();
        assert_eq!(stats.len(), 2);

        // model_a: total_output_tokens=300, total_duration_ms=5000
        let a = &stats[0];
        assert_eq!(a.model_id, model_a);
        assert_eq!(a.total_requests, 2);
        assert_eq!(
            a.total_output_tokens, 300,
            "ModelStatEntry should include total_output_tokens"
        );
        assert_eq!(
            a.total_duration_ms, 5000,
            "ModelStatEntry should include total_duration_ms"
        );

        // model_b: total_output_tokens=50, total_duration_ms=1000
        let b = &stats[1];
        assert_eq!(b.model_id, model_b);
        assert_eq!(
            b.total_output_tokens, 50,
            "ModelStatEntry should include total_output_tokens"
        );
        assert_eq!(
            b.total_duration_ms, 1000,
            "ModelStatEntry should include total_duration_ms"
        );

        // 日次平均TPSが計算可能であることを確認
        // model_a: 300 / (5000/1000) = 60 tok/s
        let tps_a = a.total_output_tokens as f64 / (a.total_duration_ms as f64 / 1000.0);
        assert!(
            (tps_a - 60.0).abs() < 0.01,
            "日次TPS計算: expected 60.0, got {tps_a}"
        );
    }

    /// T017 [US4]: get_today_stats_all が当日のTPS関連データを正しく返すことを検証
    #[tokio::test]
    async fn test_get_today_stats_all() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;

        let endpoint_id = Uuid::new_v4();
        let date = "2026-02-24";

        // (1) endpoint_daily_stats にデータ挿入
        // total_output_tokens=100, total_duration_ms=2000 になるようにupsert
        upsert_daily_stats(&pool, endpoint_id, "test-model", date, true, 100, 2000)
            .await
            .unwrap();

        // (2) get_today_stats_all 呼び出し
        let entries = get_today_stats_all(&pool, date).await.unwrap();

        // (3) 返却された TpsSeedEntry が正しいことを確認
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].endpoint_id, endpoint_id);
        assert_eq!(entries[0].model_id, "test-model");
        assert_eq!(entries[0].api_kind, "chat_completions");
        assert_eq!(entries[0].total_output_tokens, 100);
        assert_eq!(entries[0].total_duration_ms, 2000);
        assert_eq!(entries[0].successful_requests, 1);
    }
}
