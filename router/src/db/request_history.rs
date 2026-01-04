//! リクエスト/レスポンス履歴のストレージ層
//!
//! SQLiteベースでリクエスト履歴を永続化（router.dbと統合）

use chrono::{DateTime, Duration, Utc};
use llm_router_common::{
    error::{RouterError, RouterResult},
    protocol::{RecordStatus, RequestResponseRecord, RequestType},
};
use sqlx::SqlitePool;
use std::env;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

const LEGACY_DATA_DIR_ENV: &str = "LLM_ROUTER_DATA_DIR";
const DEFAULT_DATA_DIR: &str = ".llm-router";
const LEGACY_REQUEST_HISTORY_FILE: &str = "request_history.json";

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
            RouterError::Internal(format!("Failed to read legacy request history: {}", e))
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
        let node_id = record.node_id.to_string();
        let node_ip = record.node_ip.to_string();
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

        let insert_sql = if ignore_conflicts {
            r#"
            INSERT OR IGNORE INTO request_history (
                id, timestamp, request_type, model, node_id, node_machine_name,
                node_ip, client_ip, request_body, response_body, duration_ms,
                status, error_message, completed_at, input_tokens, output_tokens, total_tokens
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        } else {
            r#"
            INSERT INTO request_history (
                id, timestamp, request_type, model, node_id, node_machine_name,
                node_ip, client_ip, request_body, response_body, duration_ms,
                status, error_message, completed_at, input_tokens, output_tokens, total_tokens
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        };

        let result = sqlx::query(insert_sql)
            .bind(&id)
            .bind(&timestamp)
            .bind(&request_type)
            .bind(&record.model)
            .bind(&node_id)
            .bind(&record.node_machine_name)
            .bind(&node_ip)
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
            .execute(&self.pool)
            .await
            .map_err(|e| RouterError::Database(format!("Failed to save record: {}", e)))?;

        Ok(result.rows_affected())
    }

    /// すべてのレコードを読み込み（タイムスタンプ降順）
    pub async fn load_records(&self) -> RouterResult<Vec<RequestResponseRecord>> {
        let rows = sqlx::query_as::<_, RequestHistoryRow>(
            "SELECT * FROM request_history ORDER BY timestamp DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RouterError::Database(format!("Failed to load records: {}", e)))?;

        rows.into_iter().map(|row| row.try_into()).collect()
    }

    /// 指定期間より古いレコードを削除
    pub async fn cleanup_old_records(&self, max_age: Duration) -> RouterResult<()> {
        let cutoff = (Utc::now() - max_age).to_rfc3339();

        sqlx::query("DELETE FROM request_history WHERE timestamp < ?")
            .bind(&cutoff)
            .execute(&self.pool)
            .await
            .map_err(|e| RouterError::Database(format!("Failed to cleanup records: {}", e)))?;

        Ok(())
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

        if let Some(node_id) = filter.node_id {
            conditions.push("node_id = ?");
            params.push(node_id.to_string());
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
        let offset = (page.saturating_sub(1)) * per_page;
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
            _ => {
                sqlx::query_scalar::<_, i64>(sql)
                    .bind(&params[0])
                    .bind(&params[1])
                    .bind(&params[2])
                    .bind(&params[3])
                    .bind(&params[4])
                    .fetch_one(&self.pool)
                    .await
            }
        };

        result
            .map(|c| c as usize)
            .map_err(|e| RouterError::Database(format!("Failed to count records: {}", e)))
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
            _ => {
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
        };

        result.map_err(|e| RouterError::Database(format!("Failed to query records: {}", e)))
    }

    /// トークン統計を取得（全体）
    pub async fn get_token_statistics(&self) -> RouterResult<TokenStatistics> {
        let row = sqlx::query_as::<_, TokenStatisticsRow>(
            r#"
            SELECT
                COALESCE(SUM(input_tokens), 0) as total_input_tokens,
                COALESCE(SUM(output_tokens), 0) as total_output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens
            FROM request_history
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| RouterError::Database(format!("Failed to get token statistics: {}", e)))?;

        Ok(TokenStatistics {
            total_input_tokens: row.total_input_tokens as u64,
            total_output_tokens: row.total_output_tokens as u64,
            total_tokens: row.total_tokens as u64,
        })
    }

    /// トークン統計を取得（モデル別）
    pub async fn get_token_statistics_by_model(&self) -> RouterResult<Vec<ModelTokenStatistics>> {
        let rows = sqlx::query_as::<_, ModelTokenStatisticsRow>(
            r#"
            SELECT
                model,
                COALESCE(SUM(input_tokens), 0) as total_input_tokens,
                COALESCE(SUM(output_tokens), 0) as total_output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count
            FROM request_history
            GROUP BY model
            ORDER BY total_tokens DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            RouterError::Database(format!("Failed to get token statistics by model: {}", e))
        })?;

        Ok(rows
            .into_iter()
            .map(|row| ModelTokenStatistics {
                model: row.model,
                total_input_tokens: row.total_input_tokens as u64,
                total_output_tokens: row.total_output_tokens as u64,
                total_tokens: row.total_tokens as u64,
                request_count: row.request_count as u64,
            })
            .collect())
    }

    /// トークン統計を取得（ノード別）
    pub async fn get_token_statistics_by_node(&self) -> RouterResult<Vec<NodeTokenStatistics>> {
        let rows = sqlx::query_as::<_, NodeTokenStatisticsRow>(
            r#"
            SELECT
                node_id,
                node_machine_name,
                COALESCE(SUM(input_tokens), 0) as total_input_tokens,
                COALESCE(SUM(output_tokens), 0) as total_output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count
            FROM request_history
            GROUP BY node_id
            ORDER BY total_tokens DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            RouterError::Database(format!("Failed to get token statistics by node: {}", e))
        })?;

        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let node_id = Uuid::parse_str(&row.node_id).ok()?;
                Some(NodeTokenStatistics {
                    node_id,
                    node_machine_name: row.node_machine_name,
                    total_input_tokens: row.total_input_tokens as u64,
                    total_output_tokens: row.total_output_tokens as u64,
                    total_tokens: row.total_tokens as u64,
                    request_count: row.request_count as u64,
                })
            })
            .collect())
    }
}

fn legacy_request_history_path() -> RouterResult<PathBuf> {
    if let Ok(dir) = env::var(LEGACY_DATA_DIR_ENV) {
        return Ok(PathBuf::from(dir).join(LEGACY_REQUEST_HISTORY_FILE));
    }

    let home = env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .map_err(|_| RouterError::Internal("Failed to resolve home directory".to_string()))?;

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
                    Err(err) => return Err(RouterError::Common(err.into())),
                }
            }

            if records.is_empty() {
                return Err(RouterError::Common(primary_err.into()));
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
    node_id: String,
    node_machine_name: String,
    node_ip: String,
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
}

impl TryFrom<RequestHistoryRow> for RequestResponseRecord {
    type Error = RouterError;

    fn try_from(row: RequestHistoryRow) -> Result<Self, Self::Error> {
        let id = Uuid::parse_str(&row.id)
            .map_err(|e| RouterError::Database(format!("Invalid UUID: {}", e)))?;

        let timestamp = DateTime::parse_from_rfc3339(&row.timestamp)
            .map_err(|e| RouterError::Database(format!("Invalid timestamp: {}", e)))?
            .with_timezone(&Utc);

        let request_type = match row.request_type.as_str() {
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

        let node_id = Uuid::parse_str(&row.node_id)
            .map_err(|e| RouterError::Database(format!("Invalid node UUID: {}", e)))?;

        let node_ip: IpAddr = row
            .node_ip
            .parse()
            .map_err(|e| RouterError::Database(format!("Invalid node IP: {}", e)))?;

        let client_ip = row
            .client_ip
            .map(|ip| {
                ip.parse::<IpAddr>()
                    .map_err(|e| RouterError::Database(format!("Invalid client IP: {}", e)))
            })
            .transpose()?;

        let request_body: serde_json::Value = serde_json::from_str(&row.request_body)
            .map_err(|e| RouterError::Database(format!("Invalid request body: {}", e)))?;

        let response_body = row
            .response_body
            .map(|s| serde_json::from_str(&s))
            .transpose()
            .map_err(|e| RouterError::Database(format!("Invalid response body: {}", e)))?;

        let status = match row.status.as_str() {
            "success" => RecordStatus::Success,
            "error" => RecordStatus::Error {
                message: row.error_message.unwrap_or_default(),
            },
            _ => RecordStatus::Success,
        };

        let completed_at = DateTime::parse_from_rfc3339(&row.completed_at)
            .map_err(|e| RouterError::Database(format!("Invalid completed_at: {}", e)))?
            .with_timezone(&Utc);

        Ok(RequestResponseRecord {
            id,
            timestamp,
            request_type,
            model: row.model,
            node_id,
            node_machine_name: row.node_machine_name,
            node_ip,
            client_ip,
            request_body,
            response_body,
            duration_ms: row.duration_ms as u64,
            status,
            completed_at,
            input_tokens: row.input_tokens.map(|v| v as u32),
            output_tokens: row.output_tokens.map(|v| v as u32),
            total_tokens: row.total_tokens.map(|v| v as u32),
        })
    }
}

/// レコードフィルタ
#[derive(Debug, Clone, Default)]
pub struct RecordFilter {
    /// モデル名フィルタ（部分一致）
    pub model: Option<String>,
    /// ノードIDフィルタ
    pub node_id: Option<Uuid>,
    /// ステータスフィルタ
    pub status: Option<FilterStatus>,
    /// 開始時刻フィルタ
    pub start_time: Option<DateTime<Utc>>,
    /// 終了時刻フィルタ
    pub end_time: Option<DateTime<Utc>>,
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

        if let Some(node_id) = self.node_id {
            if record.node_id != node_id {
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

        true
    }
}

/// フィルタ用のステータス
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// トークン統計（全体）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TokenStatistics {
    /// 入力トークン合計
    pub total_input_tokens: u64,
    /// 出力トークン合計
    pub total_output_tokens: u64,
    /// 総トークン合計
    pub total_tokens: u64,
}

/// トークン統計（モデル別）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelTokenStatistics {
    /// モデル名
    pub model: String,
    /// 入力トークン合計
    pub total_input_tokens: u64,
    /// 出力トークン合計
    pub total_output_tokens: u64,
    /// 総トークン合計
    pub total_tokens: u64,
    /// リクエスト数
    pub request_count: u64,
}

/// トークン統計（ノード別）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodeTokenStatistics {
    /// ノードID
    pub node_id: Uuid,
    /// ノードマシン名
    pub node_machine_name: String,
    /// 入力トークン合計
    pub total_input_tokens: u64,
    /// 出力トークン合計
    pub total_output_tokens: u64,
    /// 総トークン合計
    pub total_tokens: u64,
    /// リクエスト数
    pub request_count: u64,
}

/// SQLiteから取得したトークン統計行（全体）
#[derive(sqlx::FromRow)]
struct TokenStatisticsRow {
    total_input_tokens: i64,
    total_output_tokens: i64,
    total_tokens: i64,
}

/// SQLiteから取得したトークン統計行（モデル別）
#[derive(sqlx::FromRow)]
struct ModelTokenStatisticsRow {
    model: String,
    total_input_tokens: i64,
    total_output_tokens: i64,
    total_tokens: i64,
    request_count: i64,
}

/// SQLiteから取得したトークン統計行（ノード別）
#[derive(sqlx::FromRow)]
struct NodeTokenStatisticsRow {
    node_id: String,
    node_machine_name: String,
    total_input_tokens: i64,
    total_output_tokens: i64,
    total_tokens: i64,
    request_count: i64,
}

/// 定期クリーンアップタスクを開始
pub fn start_cleanup_task(storage: Arc<RequestHistoryStorage>) {
    tokio::spawn(async move {
        // 起動時に1回実行
        if let Err(e) = storage.cleanup_old_records(Duration::days(7)).await {
            tracing::error!("Initial cleanup failed: {}", e);
        }

        // 1時間ごとに実行
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;

            if let Err(e) = storage.cleanup_old_records(Duration::days(7)).await {
                tracing::error!("Periodic cleanup failed: {}", e);
            } else {
                tracing::info!("Periodic cleanup completed");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::initialize_database;
    use llm_router_common::protocol::RequestType;
    use serial_test::serial;
    use tempfile::tempdir;

    async fn create_test_pool() -> SqlitePool {
        initialize_database("sqlite::memory:")
            .await
            .expect("Failed to create test database")
    }

    fn create_test_record(timestamp: DateTime<Utc>) -> RequestResponseRecord {
        RequestResponseRecord {
            id: Uuid::new_v4(),
            timestamp,
            request_type: RequestType::Chat,
            model: "test-model".to_string(),
            node_id: Uuid::new_v4(),
            node_machine_name: "test-node".to_string(),
            node_ip: "192.168.1.100".parse::<IpAddr>().unwrap(),
            client_ip: Some("10.0.0.10".parse::<IpAddr>().unwrap()),
            request_body: serde_json::json!({"test": "request"}),
            response_body: Some(serde_json::json!({"test": "response"})),
            duration_ms: 1000,
            status: RecordStatus::Success,
            completed_at: timestamp + Duration::seconds(1),
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
        }
    }

    #[tokio::test]
    async fn test_save_and_load_record() {
        let pool = create_test_pool().await;
        let storage = RequestHistoryStorage::new(pool);
        let record = create_test_record(Utc::now());

        storage.save_record(&record).await.unwrap();

        let loaded = storage.load_records().await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, record.id);
    }

    #[tokio::test]
    async fn test_cleanup_old_records() {
        let pool = create_test_pool().await;
        let storage = RequestHistoryStorage::new(pool);

        // 8日前のレコード（削除される）
        let old_record = create_test_record(Utc::now() - Duration::days(8));
        storage.save_record(&old_record).await.unwrap();

        // 6日前のレコード（残る）
        let new_record = create_test_record(Utc::now() - Duration::days(6));
        storage.save_record(&new_record).await.unwrap();

        storage
            .cleanup_old_records(Duration::days(7))
            .await
            .unwrap();

        let loaded = storage.load_records().await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, new_record.id);
    }

    #[tokio::test]
    async fn test_filter_by_model() {
        let pool = create_test_pool().await;
        let storage = RequestHistoryStorage::new(pool);

        let mut record1 = create_test_record(Utc::now());
        record1.model = "llama2".to_string();
        storage.save_record(&record1).await.unwrap();

        let mut record2 = create_test_record(Utc::now());
        record2.model = "codellama".to_string();
        storage.save_record(&record2).await.unwrap();

        let filter = RecordFilter {
            model: Some("llama2".to_string()),
            ..Default::default()
        };

        let result = storage.filter_and_paginate(&filter, 1, 10).await.unwrap();
        assert_eq!(result.records.len(), 1);
        assert_eq!(result.records[0].model, "llama2");
    }

    #[tokio::test]
    async fn test_pagination() {
        let pool = create_test_pool().await;
        let storage = RequestHistoryStorage::new(pool);

        // 5件のレコードを作成
        for i in 0..5 {
            let mut record = create_test_record(Utc::now() - Duration::seconds(i));
            record.id = Uuid::new_v4(); // 一意のIDを確保
            storage.save_record(&record).await.unwrap();
        }

        // 1ページ目（2件）
        let filter = RecordFilter::default();
        let result = storage.filter_and_paginate(&filter, 1, 2).await.unwrap();
        assert_eq!(result.records.len(), 2);
        assert_eq!(result.total_count, 5);
        assert_eq!(result.page, 1);

        // 2ページ目（2件）
        let result = storage.filter_and_paginate(&filter, 2, 2).await.unwrap();
        assert_eq!(result.records.len(), 2);

        // 3ページ目（1件）
        let result = storage.filter_and_paginate(&filter, 3, 2).await.unwrap();
        assert_eq!(result.records.len(), 1);
    }

    #[tokio::test]
    #[serial]
    async fn test_import_legacy_request_history_json() {
        let temp_dir = tempdir().expect("temp dir");
        std::env::set_var(LEGACY_DATA_DIR_ENV, temp_dir.path());

        let json_path = temp_dir.path().join(LEGACY_REQUEST_HISTORY_FILE);
        let record = create_test_record(Utc::now());
        let records = vec![record.clone()];
        std::fs::write(&json_path, serde_json::to_string(&records).unwrap()).unwrap();

        let pool = create_test_pool().await;
        let storage = RequestHistoryStorage::new(pool);

        let imported = storage.import_legacy_json_if_present().await.unwrap();
        assert_eq!(imported, 1);

        let loaded = storage.load_records().await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, record.id);

        let migrated = temp_dir
            .path()
            .join(format!("{}.migrated", LEGACY_REQUEST_HISTORY_FILE));
        assert!(migrated.exists());

        std::env::remove_var(LEGACY_DATA_DIR_ENV);
    }

    // T-6: request_historyテーブルへのトークン保存テスト
    #[tokio::test]
    async fn test_save_and_load_record_with_tokens() {
        let pool = create_test_pool().await;
        let storage = RequestHistoryStorage::new(pool);

        let mut record = create_test_record(Utc::now());
        record.input_tokens = Some(100);
        record.output_tokens = Some(50);
        record.total_tokens = Some(150);

        storage.save_record(&record).await.unwrap();

        let loaded = storage.load_records().await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].input_tokens, Some(100));
        assert_eq!(loaded[0].output_tokens, Some(50));
        assert_eq!(loaded[0].total_tokens, Some(150));
    }

    #[tokio::test]
    async fn test_save_and_load_record_with_partial_tokens() {
        let pool = create_test_pool().await;
        let storage = RequestHistoryStorage::new(pool);

        let mut record = create_test_record(Utc::now());
        record.input_tokens = Some(100);
        // output_tokens と total_tokens は None

        storage.save_record(&record).await.unwrap();

        let loaded = storage.load_records().await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].input_tokens, Some(100));
        assert_eq!(loaded[0].output_tokens, None);
        assert_eq!(loaded[0].total_tokens, None);
    }

    // T-7: トークン集計クエリテスト（累計/日次/月次）
    #[tokio::test]
    async fn test_token_aggregation_total() {
        let pool = create_test_pool().await;
        let storage = RequestHistoryStorage::new(pool);

        // 複数レコードを作成
        for i in 0..3 {
            let mut record = create_test_record(Utc::now() - Duration::seconds(i));
            record.id = Uuid::new_v4();
            record.input_tokens = Some(100);
            record.output_tokens = Some(50);
            record.total_tokens = Some(150);
            storage.save_record(&record).await.unwrap();
        }

        let stats = storage.get_token_statistics().await.unwrap();
        assert_eq!(stats.total_input_tokens, 300);
        assert_eq!(stats.total_output_tokens, 150);
        assert_eq!(stats.total_tokens, 450);
    }

    #[tokio::test]
    async fn test_token_aggregation_by_model() {
        let pool = create_test_pool().await;
        let storage = RequestHistoryStorage::new(pool);

        // モデルAのレコード
        let mut record_a = create_test_record(Utc::now());
        record_a.model = "model-a".to_string();
        record_a.input_tokens = Some(100);
        record_a.output_tokens = Some(50);
        record_a.total_tokens = Some(150);
        storage.save_record(&record_a).await.unwrap();

        // モデルBのレコード
        let mut record_b = create_test_record(Utc::now());
        record_b.id = Uuid::new_v4();
        record_b.model = "model-b".to_string();
        record_b.input_tokens = Some(200);
        record_b.output_tokens = Some(100);
        record_b.total_tokens = Some(300);
        storage.save_record(&record_b).await.unwrap();

        let stats = storage.get_token_statistics_by_model().await.unwrap();
        assert_eq!(stats.len(), 2);

        let model_a_stats = stats.iter().find(|s| s.model == "model-a").unwrap();
        assert_eq!(model_a_stats.total_input_tokens, 100);
        assert_eq!(model_a_stats.total_output_tokens, 50);

        let model_b_stats = stats.iter().find(|s| s.model == "model-b").unwrap();
        assert_eq!(model_b_stats.total_input_tokens, 200);
        assert_eq!(model_b_stats.total_output_tokens, 100);
    }

    #[tokio::test]
    async fn test_token_aggregation_by_node() {
        let pool = create_test_pool().await;
        let storage = RequestHistoryStorage::new(pool);

        let node_id_1 = Uuid::new_v4();
        let node_id_2 = Uuid::new_v4();

        // ノード1のレコード
        let mut record_1 = create_test_record(Utc::now());
        record_1.node_id = node_id_1;
        record_1.input_tokens = Some(100);
        record_1.output_tokens = Some(50);
        record_1.total_tokens = Some(150);
        storage.save_record(&record_1).await.unwrap();

        // ノード2のレコード
        let mut record_2 = create_test_record(Utc::now());
        record_2.id = Uuid::new_v4();
        record_2.node_id = node_id_2;
        record_2.input_tokens = Some(200);
        record_2.output_tokens = Some(100);
        record_2.total_tokens = Some(300);
        storage.save_record(&record_2).await.unwrap();

        let stats = storage.get_token_statistics_by_node().await.unwrap();
        assert_eq!(stats.len(), 2);

        let node_1_stats = stats.iter().find(|s| s.node_id == node_id_1).unwrap();
        assert_eq!(node_1_stats.total_input_tokens, 100);

        let node_2_stats = stats.iter().find(|s| s.node_id == node_id_2).unwrap();
        assert_eq!(node_2_stats.total_input_tokens, 200);
    }
}
