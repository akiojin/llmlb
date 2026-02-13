//! エンドポイント日次統計データベース操作
//!
//! SPEC-76643000: エンドポイント単位リクエスト統計
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
}

impl From<ModelStatRow> for ModelStatEntry {
    fn from(row: ModelStatRow) -> Self {
        ModelStatEntry {
            model_id: row.model_id,
            total_requests: row.total_requests,
            successful_requests: row.successful_requests,
            failed_requests: row.failed_requests,
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
) -> Result<(), sqlx::Error> {
    let success_increment: i64 = if success { 1 } else { 0 };
    let failure_increment: i64 = if success { 0 } else { 1 };

    sqlx::query(
        r#"
        INSERT INTO endpoint_daily_stats (endpoint_id, model_id, date, total_requests, successful_requests, failed_requests)
        VALUES (?, ?, ?, 1, ?, ?)
        ON CONFLICT(endpoint_id, model_id, date) DO UPDATE SET
            total_requests = total_requests + 1,
            successful_requests = successful_requests + excluded.successful_requests,
            failed_requests = failed_requests + excluded.failed_requests
        "#,
    )
    .bind(endpoint_id.to_string())
    .bind(model_id)
    .bind(date)
    .bind(success_increment)
    .bind(failure_increment)
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
            SUM(failed_requests) AS failed_requests
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

/// 日次統計バッチタスクを開始（SPEC-76643000）
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
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to create test database");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");
        pool
    }

    #[tokio::test]
    async fn test_upsert_daily_stats_new_record() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;

        let endpoint_id = Uuid::new_v4();
        let model_id = "llama3:8b";
        let date = "2025-01-15";

        // 成功リクエストを1件挿入
        upsert_daily_stats(&pool, endpoint_id, model_id, date, true)
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
        upsert_daily_stats(&pool, endpoint_id, model_id_2, date, false)
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
        upsert_daily_stats(&pool, endpoint_id, model_id, date, true)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_id, date, true)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_id, date, false)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_id, date, true)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_id, date, false)
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
        upsert_daily_stats(&pool, endpoint_id, model_a, "2025-01-13", true)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_a, "2025-01-14", true)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_b, "2025-01-14", false)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_a, "2025-01-15", true)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_a, "2025-01-15", true)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_b, "2025-01-15", false)
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
        upsert_daily_stats(&pool, endpoint_id, model_a, "2025-01-14", true)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_a, "2025-01-14", true)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_a, "2025-01-15", true)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_a, "2025-01-15", true)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_a, "2025-01-15", false)
            .await
            .unwrap();

        // model_b: 合計3件（成功1、失敗2）
        upsert_daily_stats(&pool, endpoint_id, model_b, "2025-01-14", false)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_b, "2025-01-15", true)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, model_b, "2025-01-15", false)
            .await
            .unwrap();

        // model_c: 合計1件（成功1）
        upsert_daily_stats(&pool, endpoint_id, model_c, "2025-01-15", true)
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
        upsert_daily_stats(&pool, endpoint_id, "llama3:8b", today, true)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, "llama3:8b", today, true)
            .await
            .unwrap();
        upsert_daily_stats(&pool, endpoint_id, "gpt-4", today, false)
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
}
