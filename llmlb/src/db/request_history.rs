//! リクエスト/レスポンス履歴のストレージ層
//!
//! SQLiteベースでリクエスト履歴を永続化（load balancer.dbと統合）

use crate::common::{
    error::{LbError, RouterResult},
    protocol::{RecordStatus, RequestResponseRecord, RequestType},
};
use crate::config::get_env_with_fallback_parse;
use chrono::{DateTime, Duration, Utc};
use sqlx::SqlitePool;
use std::env;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

mod clients;
mod statistics;
#[cfg(test)]
mod tests;

pub use clients::{
    ClientApiKeyUsage, ClientDetail, ClientIpRanking, ClientIpRankingResult, ClientRecentRequest,
    HeatmapCell, HourlyPattern, ModelDistribution, UniqueIpTimelinePoint,
};
pub use statistics::{EndpointTokenStatistics, ModelTokenStatistics, TokenStatistics};

const LEGACY_DATA_DIR_ENV: &str = "LLMLB_DATA_DIR";
const DEFAULT_DATA_DIR: &str = ".llmlb";
const LEGACY_REQUEST_HISTORY_FILE: &str = "request_history.json";
const REQUEST_HISTORY_RETENTION_DAYS_ENV: &str = "LLMLB_REQUEST_HISTORY_RETENTION_DAYS";
const LEGACY_REQUEST_HISTORY_RETENTION_DAYS_ENV: &str = "REQUEST_HISTORY_RETENTION_DAYS";
const REQUEST_HISTORY_CLEANUP_INTERVAL_ENV: &str = "LLMLB_REQUEST_HISTORY_CLEANUP_INTERVAL_SECS";
const LEGACY_REQUEST_HISTORY_CLEANUP_INTERVAL_ENV: &str = "REQUEST_HISTORY_CLEANUP_INTERVAL_SECS";

/// リクエスト履歴ストレージ（SQLite版）
#[derive(Clone)]
pub struct RequestHistoryStorage {
    pool: SqlitePool,
}

impl RequestHistoryStorage {
    /// 新しいストレージインスタンスを作成
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// IDでレコードを取得
    pub async fn get_record_by_id(&self, id: Uuid) -> RouterResult<Option<RequestResponseRecord>> {
        let row = sqlx::query_as::<_, RequestHistoryRow>(
            "SELECT * FROM request_history WHERE id = ? LIMIT 1",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| LbError::Database(format!("Failed to load record: {}", e)))?;

        match row {
            Some(row) => Ok(Some(row.try_into()?)),
            None => Ok(None),
        }
    }

    /// レコードを保存
    pub async fn save_record(&self, record: &RequestResponseRecord) -> RouterResult<()> {
        self.insert_record(record, false).await?;
        Ok(())
    }

    /// 旧JSON履歴ファイルをSQLiteへインポート（存在すれば）
    pub async fn import_legacy_json_if_present(&self) -> RouterResult<usize> {
        let json_path = legacy_request_history_path()?;
        if !json_path.exists() {
            return Ok(0);
        }

        let contents = std::fs::read_to_string(&json_path).map_err(|e| {
            LbError::Internal(format!("Failed to read legacy request history: {}", e))
        })?;

        let records = parse_legacy_records(&contents)?;
        if records.is_empty() {
            tracing::info!(
                "Legacy request history file is empty: {}",
                json_path.display()
            );
        }

        let mut imported = 0usize;
        for record in &records {
            let inserted = self.insert_record(record, true).await?;
            imported += inserted as usize;
        }

        let migrated_path = legacy_migrated_path(&json_path);
        if let Err(err) = std::fs::rename(&json_path, &migrated_path) {
            tracing::warn!(
                "Failed to rename legacy request history to {}: {}",
                migrated_path.display(),
                err
            );
        } else {
            tracing::info!(
                "Legacy request history migrated: {} -> {}",
                json_path.display(),
                migrated_path.display()
            );
        }

        Ok(imported)
    }

    async fn insert_record(
        &self,
        record: &RequestResponseRecord,
        ignore_conflicts: bool,
    ) -> RouterResult<u64> {
        let id = record.id.to_string();
        let timestamp = record.timestamp.to_rfc3339();
        let request_type = format!("{:?}", record.request_type);
        let endpoint_id_str = record.endpoint_id.to_string();
        let endpoint_ip_str = record.endpoint_ip.to_string();
        let client_ip = record.client_ip.map(|ip| ip.to_string());
        let request_body = record.request_body.to_string();
        let response_body = record.response_body.as_ref().map(|v| v.to_string());
        let duration_ms = record.duration_ms as i64;
        let (status, error_message) = match &record.status {
            RecordStatus::Success => ("success".to_string(), None),
            RecordStatus::Error { message } => ("error".to_string(), Some(message.clone())),
        };
        let completed_at = record.completed_at.to_rfc3339();

        let input_tokens = record.input_tokens.map(|v| v as i64);
        let output_tokens = record.output_tokens.map(|v| v as i64);
        let total_tokens = record.total_tokens.map(|v| v as i64);

        let api_key_id = record.api_key_id.map(|id| id.to_string());

        let insert_sql = if ignore_conflicts {
            r#"
            INSERT OR IGNORE INTO request_history (
                id, timestamp, request_type, model, endpoint_id, endpoint_name,
                endpoint_ip, client_ip, request_body, response_body, duration_ms,
                status, error_message, completed_at, input_tokens, output_tokens, total_tokens,
                api_key_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        } else {
            r#"
            INSERT INTO request_history (
                id, timestamp, request_type, model, endpoint_id, endpoint_name,
                endpoint_ip, client_ip, request_body, response_body, duration_ms,
                status, error_message, completed_at, input_tokens, output_tokens, total_tokens,
                api_key_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        };

        let result = sqlx::query(insert_sql)
            .bind(&id)
            .bind(&timestamp)
            .bind(&request_type)
            .bind(&record.model)
            .bind(&endpoint_id_str)
            .bind(&record.endpoint_name)
            .bind(&endpoint_ip_str)
            .bind(&client_ip)
            .bind(&request_body)
            .bind(&response_body)
            .bind(duration_ms)
            .bind(&status)
            .bind(&error_message)
            .bind(&completed_at)
            .bind(input_tokens)
            .bind(output_tokens)
            .bind(total_tokens)
            .bind(&api_key_id)
            .execute(&self.pool)
            .await
            .map_err(|e| LbError::Database(format!("Failed to save record: {}", e)))?;

        Ok(result.rows_affected())
    }

    /// すべてのレコードを読み込み（タイムスタンプ降順）
    pub async fn load_records(&self) -> RouterResult<Vec<RequestResponseRecord>> {
        let rows = sqlx::query_as::<_, RequestHistoryRow>(
            "SELECT * FROM request_history ORDER BY timestamp DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| LbError::Database(format!("Failed to load records: {}", e)))?;

        rows.into_iter().map(|row| row.try_into()).collect()
    }

    /// 指定期間より古いレコードを削除
    pub async fn cleanup_old_records(&self, max_age: Duration) -> RouterResult<()> {
        let cutoff = (Utc::now() - max_age).to_rfc3339();

        sqlx::query("DELETE FROM request_history WHERE timestamp < ?")
            .bind(&cutoff)
            .execute(&self.pool)
            .await
            .map_err(|e| LbError::Database(format!("Failed to cleanup records: {}", e)))?;

        Ok(())
    }

    /// 直近N分のリクエスト履歴を分単位で集計して返す（起動時seeding用）
    pub async fn get_recent_history_by_minute(
        &self,
        minutes: i64,
    ) -> RouterResult<Vec<MinuteHistoryPoint>> {
        let cutoff = (Utc::now() - chrono::Duration::minutes(minutes)).to_rfc3339();

        let rows = sqlx::query_as::<_, MinuteHistoryRow>(
            r#"
            SELECT
                strftime('%Y-%m-%dT%H:%M:00Z', completed_at) AS minute,
                SUM(CASE WHEN status = 'success' THEN 1 ELSE 0 END) AS success_count,
                SUM(CASE WHEN status != 'success' THEN 1 ELSE 0 END) AS error_count
            FROM request_history
            WHERE completed_at >= ?
            GROUP BY strftime('%Y-%m-%dT%H:%M:00Z', completed_at)
            ORDER BY minute ASC
            "#,
        )
        .bind(&cutoff)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| LbError::Database(format!("Failed to get recent history by minute: {}", e)))?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    /// レコードをフィルタリング＆ページネーション
    pub async fn filter_and_paginate(
        &self,
        filter: &RecordFilter,
        page: usize,
        per_page: usize,
    ) -> RouterResult<FilteredRecords> {
        // クエリを動的に構築
        let mut conditions = Vec::new();
        let mut params: Vec<String> = Vec::new();

        if let Some(ref model) = filter.model {
            conditions.push("model LIKE ?");
            params.push(format!("%{}%", model));
        }

        if let Some(endpoint_id) = filter.endpoint_id {
            conditions.push("endpoint_id = ?");
            params.push(endpoint_id.to_string());
        }

        if let Some(ref status) = filter.status {
            conditions.push("status = ?");
            params.push(match status {
                FilterStatus::Success => "success".to_string(),
                FilterStatus::Error => "error".to_string(),
            });
        }

        if let Some(start_time) = filter.start_time {
            conditions.push("timestamp >= ?");
            params.push(start_time.to_rfc3339());
        }

        if let Some(end_time) = filter.end_time {
            conditions.push("timestamp <= ?");
            params.push(end_time.to_rfc3339());
        }

        if let Some(ref client_ip) = filter.client_ip {
            conditions.push("client_ip = ?");
            params.push(client_ip.clone());
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        // 総件数を取得
        let count_sql = format!(
            "SELECT COUNT(*) as count FROM request_history {}",
            where_clause
        );
        let total_count = self.execute_count_query(&count_sql, &params).await?;

        // ページネーション
        let offset = page.saturating_sub(1).saturating_mul(per_page);
        let data_sql = format!(
            "SELECT * FROM request_history {} ORDER BY timestamp DESC LIMIT ? OFFSET ?",
            where_clause
        );

        let rows = self
            .execute_select_query(&data_sql, &params, per_page as i64, offset as i64)
            .await?;

        let records: RouterResult<Vec<RequestResponseRecord>> =
            rows.into_iter().map(|row| row.try_into()).collect();

        Ok(FilteredRecords {
            records: records?,
            total_count,
            page,
            per_page,
        })
    }

    /// カウントクエリを実行
    async fn execute_count_query(&self, sql: &str, params: &[String]) -> RouterResult<usize> {
        // パラメータ数に応じて動的にバインド
        let result = match params.len() {
            0 => {
                sqlx::query_scalar::<_, i64>(sql)
                    .fetch_one(&self.pool)
                    .await
            }
            1 => {
                sqlx::query_scalar::<_, i64>(sql)
                    .bind(&params[0])
                    .fetch_one(&self.pool)
                    .await
            }
            2 => {
                sqlx::query_scalar::<_, i64>(sql)
                    .bind(&params[0])
                    .bind(&params[1])
                    .fetch_one(&self.pool)
                    .await
            }
            3 => {
                sqlx::query_scalar::<_, i64>(sql)
                    .bind(&params[0])
                    .bind(&params[1])
                    .bind(&params[2])
                    .fetch_one(&self.pool)
                    .await
            }
            4 => {
                sqlx::query_scalar::<_, i64>(sql)
                    .bind(&params[0])
                    .bind(&params[1])
                    .bind(&params[2])
                    .bind(&params[3])
                    .fetch_one(&self.pool)
                    .await
            }
            5 => {
                sqlx::query_scalar::<_, i64>(sql)
                    .bind(&params[0])
                    .bind(&params[1])
                    .bind(&params[2])
                    .bind(&params[3])
                    .bind(&params[4])
                    .fetch_one(&self.pool)
                    .await
            }
            _ => {
                sqlx::query_scalar::<_, i64>(sql)
                    .bind(&params[0])
                    .bind(&params[1])
                    .bind(&params[2])
                    .bind(&params[3])
                    .bind(&params[4])
                    .bind(&params[5])
                    .fetch_one(&self.pool)
                    .await
            }
        };

        result
            .map(|c| c as usize)
            .map_err(|e| LbError::Database(format!("Failed to count records: {}", e)))
    }

    /// SELECTクエリを実行
    async fn execute_select_query(
        &self,
        sql: &str,
        params: &[String],
        limit: i64,
        offset: i64,
    ) -> RouterResult<Vec<RequestHistoryRow>> {
        // パラメータ数に応じて動的にバインド
        let result = match params.len() {
            0 => {
                sqlx::query_as::<_, RequestHistoryRow>(sql)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(&self.pool)
                    .await
            }
            1 => {
                sqlx::query_as::<_, RequestHistoryRow>(sql)
                    .bind(&params[0])
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(&self.pool)
                    .await
            }
            2 => {
                sqlx::query_as::<_, RequestHistoryRow>(sql)
                    .bind(&params[0])
                    .bind(&params[1])
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(&self.pool)
                    .await
            }
            3 => {
                sqlx::query_as::<_, RequestHistoryRow>(sql)
                    .bind(&params[0])
                    .bind(&params[1])
                    .bind(&params[2])
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(&self.pool)
                    .await
            }
            4 => {
                sqlx::query_as::<_, RequestHistoryRow>(sql)
                    .bind(&params[0])
                    .bind(&params[1])
                    .bind(&params[2])
                    .bind(&params[3])
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(&self.pool)
                    .await
            }
            5 => {
                sqlx::query_as::<_, RequestHistoryRow>(sql)
                    .bind(&params[0])
                    .bind(&params[1])
                    .bind(&params[2])
                    .bind(&params[3])
                    .bind(&params[4])
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(&self.pool)
                    .await
            }
            _ => {
                sqlx::query_as::<_, RequestHistoryRow>(sql)
                    .bind(&params[0])
                    .bind(&params[1])
                    .bind(&params[2])
                    .bind(&params[3])
                    .bind(&params[4])
                    .bind(&params[5])
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(&self.pool)
                    .await
            }
        };

        result.map_err(|e| LbError::Database(format!("Failed to query records: {}", e)))
    }
}

fn legacy_request_history_path() -> RouterResult<PathBuf> {
    if let Ok(dir) = env::var(LEGACY_DATA_DIR_ENV) {
        return Ok(PathBuf::from(dir).join(LEGACY_REQUEST_HISTORY_FILE));
    }

    let home = env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .map_err(|_| LbError::Internal("Failed to resolve home directory".to_string()))?;

    Ok(PathBuf::from(home)
        .join(DEFAULT_DATA_DIR)
        .join(LEGACY_REQUEST_HISTORY_FILE))
}

fn legacy_migrated_path(original: &Path) -> PathBuf {
    let file_name = original
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(LEGACY_REQUEST_HISTORY_FILE);
    let migrated_name = format!("{}.migrated", file_name);
    original.with_file_name(migrated_name)
}

fn parse_legacy_records(contents: &str) -> RouterResult<Vec<RequestResponseRecord>> {
    if contents.trim().is_empty() {
        return Ok(Vec::new());
    }

    match serde_json::from_str::<Vec<RequestResponseRecord>>(contents) {
        Ok(records) => Ok(records),
        Err(primary_err) => {
            let mut records = Vec::new();
            let stream =
                serde_json::Deserializer::from_str(contents).into_iter::<RequestResponseRecord>();
            for record in stream {
                match record {
                    Ok(item) => records.push(item),
                    Err(err) => return Err(LbError::Common(err.into())),
                }
            }

            if records.is_empty() {
                return Err(LbError::Common(primary_err.into()));
            }

            Ok(records)
        }
    }
}

/// SQLiteから取得した行データ
#[derive(sqlx::FromRow)]
struct RequestHistoryRow {
    id: String,
    timestamp: String,
    request_type: String,
    model: String,
    endpoint_id: String,
    endpoint_name: String,
    endpoint_ip: String,
    client_ip: Option<String>,
    request_body: String,
    response_body: Option<String>,
    duration_ms: i64,
    status: String,
    error_message: Option<String>,
    #[allow(dead_code)]
    completed_at: String,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    total_tokens: Option<i64>,
    api_key_id: Option<String>,
}

impl TryFrom<RequestHistoryRow> for RequestResponseRecord {
    type Error = LbError;

    fn try_from(row: RequestHistoryRow) -> Result<Self, Self::Error> {
        let id = Uuid::parse_str(&row.id)
            .map_err(|e| LbError::Database(format!("Invalid UUID: {}", e)))?;

        let timestamp = DateTime::parse_from_rfc3339(&row.timestamp)
            .map_err(|e| LbError::Database(format!("Invalid timestamp: {}", e)))?
            .with_timezone(&Utc);

        let request_type = match row.request_type.as_str() {
            "AnthropicMessages" => RequestType::AnthropicMessages,
            "Chat" => RequestType::Chat,
            "Generate" => RequestType::Generate,
            "Embeddings" => RequestType::Embeddings,
            "Transcription" => RequestType::Transcription,
            "Speech" => RequestType::Speech,
            "ImageGeneration" => RequestType::ImageGeneration,
            "ImageEdit" => RequestType::ImageEdit,
            "ImageVariation" => RequestType::ImageVariation,
            _ => RequestType::Chat, // フォールバック
        };

        let endpoint_id = Uuid::parse_str(&row.endpoint_id)
            .map_err(|e| LbError::Database(format!("Invalid endpoint UUID: {}", e)))?;

        let endpoint_ip: IpAddr = row
            .endpoint_ip
            .parse()
            .map_err(|e| LbError::Database(format!("Invalid endpoint IP: {}", e)))?;

        let client_ip = row
            .client_ip
            .map(|ip| {
                ip.parse::<IpAddr>()
                    .map_err(|e| LbError::Database(format!("Invalid client IP: {}", e)))
            })
            .transpose()?;

        let request_body: serde_json::Value = serde_json::from_str(&row.request_body)
            .map_err(|e| LbError::Database(format!("Invalid request body: {}", e)))?;

        let response_body = row
            .response_body
            .map(|s| serde_json::from_str(&s))
            .transpose()
            .map_err(|e| LbError::Database(format!("Invalid response body: {}", e)))?;

        let status = match row.status.as_str() {
            "success" => RecordStatus::Success,
            "error" => RecordStatus::Error {
                message: row.error_message.unwrap_or_default(),
            },
            _ => RecordStatus::Success,
        };

        let completed_at = DateTime::parse_from_rfc3339(&row.completed_at)
            .map_err(|e| LbError::Database(format!("Invalid completed_at: {}", e)))?
            .with_timezone(&Utc);

        Ok(RequestResponseRecord {
            id,
            timestamp,
            request_type,
            model: row.model,
            endpoint_id,
            endpoint_name: row.endpoint_name,
            endpoint_ip,
            client_ip,
            request_body,
            response_body,
            duration_ms: row.duration_ms as u64,
            status,
            completed_at,
            input_tokens: row.input_tokens.map(|v| v as u32),
            output_tokens: row.output_tokens.map(|v| v as u32),
            total_tokens: row.total_tokens.map(|v| v as u32),
            api_key_id: row
                .api_key_id
                .map(|id| {
                    Uuid::parse_str(&id)
                        .map_err(|e| LbError::Database(format!("Invalid api_key_id UUID: {}", e)))
                })
                .transpose()?,
        })
    }
}

/// 分単位の履歴集計ポイント（起動時seeding用）
#[derive(Debug, Clone)]
pub struct MinuteHistoryPoint {
    /// 分（UTC ISO8601形式）
    pub minute: String,
    /// 成功リクエスト数
    pub success_count: i64,
    /// 失敗リクエスト数
    pub error_count: i64,
}

#[derive(sqlx::FromRow)]
struct MinuteHistoryRow {
    minute: String,
    success_count: i64,
    error_count: i64,
}

impl From<MinuteHistoryRow> for MinuteHistoryPoint {
    fn from(row: MinuteHistoryRow) -> Self {
        MinuteHistoryPoint {
            minute: row.minute,
            success_count: row.success_count,
            error_count: row.error_count,
        }
    }
}

/// レコードフィルタ
#[derive(Debug, Clone, Default)]
pub struct RecordFilter {
    /// モデル名フィルタ（部分一致）
    pub model: Option<String>,
    /// エンドポイントIDフィルタ
    pub endpoint_id: Option<Uuid>,
    /// ステータスフィルタ
    pub status: Option<FilterStatus>,
    /// 開始時刻フィルタ
    pub start_time: Option<DateTime<Utc>>,
    /// 終了時刻フィルタ
    pub end_time: Option<DateTime<Utc>>,
    /// クライアントIPフィルタ（完全一致）
    pub client_ip: Option<String>,
}

impl RecordFilter {
    /// レコードがフィルタ条件に一致するか（テスト用）
    #[cfg(test)]
    pub fn matches(&self, record: &RequestResponseRecord) -> bool {
        if let Some(ref model) = self.model {
            if !record.model.contains(model) {
                return false;
            }
        }

        if let Some(endpoint_id) = self.endpoint_id {
            if record.endpoint_id != endpoint_id {
                return false;
            }
        }

        if let Some(ref status) = self.status {
            match (status, &record.status) {
                (FilterStatus::Success, RecordStatus::Success) => {}
                (FilterStatus::Error, RecordStatus::Error { .. }) => {}
                _ => return false,
            }
        }

        if let Some(start_time) = self.start_time {
            if record.timestamp < start_time {
                return false;
            }
        }

        if let Some(end_time) = self.end_time {
            if record.timestamp > end_time {
                return false;
            }
        }

        if let Some(ref client_ip) = self.client_ip {
            match &record.client_ip {
                Some(ip) if ip.to_string() == *client_ip => {}
                _ => return false,
            }
        }

        true
    }
}

/// フィルタ用のステータス
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FilterStatus {
    /// 成功したリクエスト
    Success,
    /// 失敗したリクエスト
    Error,
}

/// フィルタ済みレコード
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FilteredRecords {
    /// フィルタ・ページネーション適用後のレコード
    pub records: Vec<RequestResponseRecord>,
    /// フィルタ適用後の総件数
    pub total_count: usize,
    /// 現在のページ番号
    pub page: usize,
    /// 1ページあたりの件数
    pub per_page: usize,
}

/// 定期クリーンアップタスクを開始
pub fn start_cleanup_task(storage: Arc<RequestHistoryStorage>) {
    let retention_days = get_env_with_fallback_parse(
        REQUEST_HISTORY_RETENTION_DAYS_ENV,
        LEGACY_REQUEST_HISTORY_RETENTION_DAYS_ENV,
        7i64,
    );
    let interval_secs = get_env_with_fallback_parse(
        REQUEST_HISTORY_CLEANUP_INTERVAL_ENV,
        LEGACY_REQUEST_HISTORY_CLEANUP_INTERVAL_ENV,
        3600u64,
    );

    if retention_days <= 0 {
        tracing::info!("Request history cleanup disabled ({} <= 0)", retention_days);
        return;
    }

    tokio::spawn(async move {
        // 起動時に1回実行
        let retention = Duration::days(retention_days);
        if let Err(e) = storage.cleanup_old_records(retention).await {
            tracing::error!("Initial cleanup failed: {}", e);
        }

        // 1時間ごとに実行
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
        loop {
            interval.tick().await;

            if let Err(e) = storage.cleanup_old_records(retention).await {
                tracing::error!("Periodic cleanup failed: {}", e);
            } else {
                tracing::info!("Periodic cleanup completed");
            }
        }
    });
}
