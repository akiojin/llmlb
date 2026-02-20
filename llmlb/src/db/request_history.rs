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

        let api_key_id = record.api_key_id.map(|id| id.to_string());

        let insert_sql = if ignore_conflicts {
            r#"
            INSERT OR IGNORE INTO request_history (
                id, timestamp, request_type, model, node_id, node_machine_name,
                node_ip, client_ip, request_body, response_body, duration_ms,
                status, error_message, completed_at, input_tokens, output_tokens, total_tokens,
                api_key_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        } else {
            r#"
            INSERT INTO request_history (
                id, timestamp, request_type, model, node_id, node_machine_name,
                node_ip, client_ip, request_body, response_body, duration_ms,
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

    /// トークン統計を取得（全体）
    pub async fn get_token_statistics(&self) -> RouterResult<TokenStatistics> {
        let row = sqlx::query_as::<_, TokenStatisticsRow>(
            r#"
            SELECT
                COALESCE(SUM(input_tokens), 0) as total_input_tokens,
                COALESCE(SUM(output_tokens), 0) as total_output_tokens,
                COALESCE(
                    SUM(
                        COALESCE(
                            total_tokens,
                            COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)
                        )
                    ),
                    0
                ) as total_tokens
            FROM request_history
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| LbError::Database(format!("Failed to get token statistics: {}", e)))?;

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
                COALESCE(
                    SUM(
                        COALESCE(
                            total_tokens,
                            COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)
                        )
                    ),
                    0
                ) as total_tokens,
                COUNT(*) as request_count
            FROM request_history
            GROUP BY model
            ORDER BY total_tokens DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            LbError::Database(format!("Failed to get token statistics by model: {}", e))
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
                COALESCE(
                    SUM(
                        COALESCE(
                            total_tokens,
                            COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)
                        )
                    ),
                    0
                ) as total_tokens,
                COUNT(*) as request_count
            FROM request_history
            GROUP BY node_id
            ORDER BY total_tokens DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| LbError::Database(format!("Failed to get token statistics by node: {}", e)))?;

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

    /// 日次トークン統計を取得
    pub async fn get_daily_token_statistics(
        &self,
        days: u32,
    ) -> RouterResult<Vec<crate::api::dashboard::DailyTokenStats>> {
        let rows = sqlx::query_as::<_, DailyTokenStatisticsRow>(
            r#"
            SELECT
                DATE(timestamp) as date,
                COALESCE(SUM(input_tokens), 0) as total_input_tokens,
                COALESCE(SUM(output_tokens), 0) as total_output_tokens,
                COALESCE(
                    SUM(
                        COALESCE(
                            total_tokens,
                            COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)
                        )
                    ),
                    0
                ) as total_tokens,
                COUNT(*) as request_count
            FROM request_history
            WHERE timestamp >= DATE('now', '-' || ? || ' days')
            GROUP BY DATE(timestamp)
            ORDER BY date DESC
            "#,
        )
        .bind(days as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| LbError::Database(format!("Failed to get daily token statistics: {}", e)))?;

        Ok(rows
            .into_iter()
            .map(|row| crate::api::dashboard::DailyTokenStats {
                date: row.date,
                total_input_tokens: row.total_input_tokens as u64,
                total_output_tokens: row.total_output_tokens as u64,
                total_tokens: row.total_tokens as u64,
                request_count: row.request_count as u64,
            })
            .collect())
    }

    /// 月次トークン統計を取得
    pub async fn get_monthly_token_statistics(
        &self,
        months: u32,
    ) -> RouterResult<Vec<crate::api::dashboard::MonthlyTokenStats>> {
        let rows = sqlx::query_as::<_, MonthlyTokenStatisticsRow>(
            r#"
            SELECT
                strftime('%Y-%m', timestamp) as month,
                COALESCE(SUM(input_tokens), 0) as total_input_tokens,
                COALESCE(SUM(output_tokens), 0) as total_output_tokens,
                COALESCE(
                    SUM(
                        COALESCE(
                            total_tokens,
                            COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)
                        )
                    ),
                    0
                ) as total_tokens,
                COUNT(*) as request_count
            FROM request_history
            WHERE timestamp >= DATE('now', '-' || ? || ' months')
            GROUP BY strftime('%Y-%m', timestamp)
            ORDER BY month DESC
            "#,
        )
        .bind(months as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| LbError::Database(format!("Failed to get monthly token statistics: {}", e)))?;

        Ok(rows
            .into_iter()
            .map(|row| crate::api::dashboard::MonthlyTokenStats {
                month: row.month,
                total_input_tokens: row.total_input_tokens as u64,
                total_output_tokens: row.total_output_tokens as u64,
                total_tokens: row.total_tokens as u64,
                request_count: row.request_count as u64,
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
            .map_err(|e| LbError::Database(format!("Invalid node UUID: {}", e)))?;

        let node_ip: IpAddr = row
            .node_ip
            .parse()
            .map_err(|e| LbError::Database(format!("Invalid node IP: {}", e)))?;

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

/// SQLiteから取得したトークン統計行（日次）
#[derive(sqlx::FromRow)]
struct DailyTokenStatisticsRow {
    date: String,
    total_input_tokens: i64,
    total_output_tokens: i64,
    total_tokens: i64,
    request_count: i64,
}

/// SQLiteから取得したトークン統計行（月次）
#[derive(sqlx::FromRow)]
struct MonthlyTokenStatisticsRow {
    month: String,
    total_input_tokens: i64,
    total_output_tokens: i64,
    total_tokens: i64,
    request_count: i64,
}

/// IPランキングの1エントリ
#[derive(Debug, Clone, serde::Serialize)]
pub struct ClientIpRanking {
    /// IPアドレス（IPv6は/64プレフィックス）
    pub ip: String,
    /// リクエスト数
    pub request_count: i64,
    /// 最終アクセス時刻
    pub last_seen: String,
    /// 閾値超過フラグ（デフォルトfalse、API層で設定）
    pub is_alert: bool,
    /// 使用APIキー数
    pub api_key_count: i64,
}

/// IPランキングの集計結果
#[derive(Debug, Clone, serde::Serialize)]
pub struct ClientIpRankingResult {
    /// ランキング一覧
    pub rankings: Vec<ClientIpRanking>,
    /// 総件数
    pub total_count: usize,
    /// 現在のページ番号
    pub page: usize,
    /// 1ページあたりの件数
    pub per_page: usize,
}

/// SQLiteから取得したIP集計行
#[derive(sqlx::FromRow)]
struct ClientIpRow {
    client_ip: Option<String>,
    request_count: i64,
    last_seen: String,
}

impl RequestHistoryStorage {
    /// IPランキングを取得（リクエスト数降順、ページネーション付き）
    ///
    /// IPv6アドレスは/64プレフィックスでグルーピングして集計する。
    pub async fn get_client_ip_ranking(
        &self,
        hours: u32,
        page: usize,
        per_page: usize,
    ) -> RouterResult<ClientIpRankingResult> {
        let cutoff = Utc::now() - Duration::hours(hours as i64);
        let cutoff_str = cutoff.to_rfc3339();

        // SQLiteでIPごとの集計を取得（NULLのclient_ipは除外）
        let rows: Vec<ClientIpRow> = sqlx::query_as(
            "SELECT client_ip, COUNT(*) as request_count,
                    MAX(timestamp) as last_seen
             FROM request_history
             WHERE client_ip IS NOT NULL AND timestamp >= ?
             GROUP BY client_ip
             ORDER BY request_count DESC",
        )
        .bind(&cutoff_str)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| LbError::Database(e.to_string()))?;

        // IPv6 /64グルーピング: Rust側で集約
        use std::collections::HashMap;
        let mut grouped: HashMap<String, (i64, String)> = HashMap::new();

        for row in &rows {
            let ip_str = match &row.client_ip {
                Some(ip) => ip.as_str(),
                None => continue,
            };
            let key = crate::common::ip::ipv6_to_prefix64(ip_str);

            let entry = grouped.entry(key).or_insert_with(|| (0, String::new()));
            entry.0 += row.request_count;
            // last_seenは最新を保持
            if entry.1.is_empty() || row.last_seen > entry.1 {
                entry.1.clone_from(&row.last_seen);
            }
        }

        let mut rankings: Vec<ClientIpRanking> = grouped
            .into_iter()
            .map(|(ip, (count, last_seen))| ClientIpRanking {
                ip,
                request_count: count,
                last_seen,
                is_alert: false,
                api_key_count: 0, // Phase 8で正確な集計に更新
            })
            .collect();

        // リクエスト数降順ソート
        rankings.sort_by(|a, b| b.request_count.cmp(&a.request_count));

        let total_count = rankings.len();
        let offset = (page.saturating_sub(1)) * per_page;
        let paginated = rankings.into_iter().skip(offset).take(per_page).collect();

        Ok(ClientIpRankingResult {
            rankings: paginated,
            total_count,
            page,
            per_page,
        })
    }

    /// ユニークIP数の1時間刻みタイムラインを取得
    ///
    /// 指定時間数分の24ポイント（各時間帯のユニークIP数）を返す。
    /// データがない時間帯はunique_ips=0で埋める。
    pub async fn get_unique_ip_timeline(
        &self,
        hours: u32,
    ) -> RouterResult<Vec<UniqueIpTimelinePoint>> {
        let now = Utc::now();
        let cutoff = now - Duration::hours(hours as i64);
        let cutoff_str = cutoff.to_rfc3339();

        let rows: Vec<TimelineRow> = sqlx::query_as(
            "SELECT strftime('%Y-%m-%dT%H:00:00', timestamp) as hour,
                    COUNT(DISTINCT client_ip) as unique_ips
             FROM request_history
             WHERE client_ip IS NOT NULL AND timestamp >= ?
             GROUP BY strftime('%Y-%m-%dT%H:00:00', timestamp)
             ORDER BY hour ASC",
        )
        .bind(&cutoff_str)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| LbError::Database(e.to_string()))?;

        // 24時間分のポイントを生成（データがない時間帯は0埋め）
        use std::collections::HashMap;
        let row_map: HashMap<String, i64> =
            rows.into_iter().map(|r| (r.hour, r.unique_ips)).collect();

        let mut timeline = Vec::with_capacity(hours as usize);
        for h in (1..=hours).rev() {
            let point_time = now - Duration::hours(h as i64);
            let hour_key = point_time.format("%Y-%m-%dT%H:00:00").to_string();
            let unique_ips = row_map.get(&hour_key).copied().unwrap_or(0);
            timeline.push(UniqueIpTimelinePoint {
                hour: hour_key,
                unique_ips,
            });
        }

        Ok(timeline)
    }

    /// モデル別リクエスト分布を取得
    ///
    /// 指定時間数内のモデル別リクエスト数とパーセンテージを返す。
    pub async fn get_model_distribution_by_clients(
        &self,
        hours: u32,
    ) -> RouterResult<Vec<ModelDistribution>> {
        let cutoff = Utc::now() - Duration::hours(hours as i64);
        let cutoff_str = cutoff.to_rfc3339();

        let rows: Vec<ModelDistRow> = sqlx::query_as(
            "SELECT model, COUNT(*) as request_count
             FROM request_history
             WHERE client_ip IS NOT NULL AND timestamp >= ?
             GROUP BY model
             ORDER BY request_count DESC",
        )
        .bind(&cutoff_str)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| LbError::Database(e.to_string()))?;

        let total: i64 = rows.iter().map(|r| r.request_count).sum();

        let distributions = rows
            .into_iter()
            .map(|r| {
                let percentage = if total > 0 {
                    (r.request_count as f64 / total as f64) * 100.0
                } else {
                    0.0
                };
                ModelDistribution {
                    model: r.model,
                    request_count: r.request_count,
                    percentage: (percentage * 10.0).round() / 10.0, // 小数点1位
                }
            })
            .collect();

        Ok(distributions)
    }
}

/// タイムラインの1ポイント
#[derive(Debug, Clone, serde::Serialize)]
pub struct UniqueIpTimelinePoint {
    /// 時間帯（ISO 8601形式、時間単位に丸め）
    pub hour: String,
    /// ユニークIP数
    pub unique_ips: i64,
}

/// モデル別リクエスト分布
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelDistribution {
    /// モデル名
    pub model: String,
    /// リクエスト数
    pub request_count: i64,
    /// パーセンテージ（小数点1位）
    pub percentage: f64,
}

/// SQLiteから取得したタイムライン行
#[derive(sqlx::FromRow)]
struct TimelineRow {
    hour: String,
    unique_ips: i64,
}

/// SQLiteから取得したモデル分布行
#[derive(sqlx::FromRow)]
struct ModelDistRow {
    model: String,
    request_count: i64,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::protocol::RequestType;
    use crate::db::migrations::initialize_database;
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
            api_key_id: None,
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
    async fn test_token_aggregation_total_infers_total_tokens_when_null() {
        let pool = create_test_pool().await;
        let storage = RequestHistoryStorage::new(pool);

        let now = Utc::now();

        // total_tokens が NULL の場合は input_tokens + output_tokens を合算する
        let mut record_1 = create_test_record(now);
        record_1.input_tokens = Some(100);
        record_1.output_tokens = Some(50);
        record_1.total_tokens = None;
        storage.save_record(&record_1).await.unwrap();

        // output_tokens が NULL の場合は input_tokens のみをカウントする（SQL上は +0 になる）
        let mut record_2 = create_test_record(now - Duration::seconds(1));
        record_2.input_tokens = Some(10);
        record_2.output_tokens = None;
        record_2.total_tokens = None;
        storage.save_record(&record_2).await.unwrap();

        // input_tokens が NULL の場合は output_tokens のみをカウントする（SQL上は 0+output になる）
        let mut record_3 = create_test_record(now - Duration::seconds(2));
        record_3.input_tokens = None;
        record_3.output_tokens = Some(5);
        record_3.total_tokens = None;
        storage.save_record(&record_3).await.unwrap();

        // total_tokens がある場合はそれを優先する
        let mut record_4 = create_test_record(now - Duration::seconds(3));
        record_4.input_tokens = None;
        record_4.output_tokens = None;
        record_4.total_tokens = Some(7);
        storage.save_record(&record_4).await.unwrap();

        let stats = storage.get_token_statistics().await.unwrap();
        assert_eq!(stats.total_input_tokens, 110);
        assert_eq!(stats.total_output_tokens, 55);
        assert_eq!(stats.total_tokens, 172);
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
    async fn test_token_aggregation_by_model_infers_total_tokens_when_null() {
        let pool = create_test_pool().await;
        let storage = RequestHistoryStorage::new(pool);

        let mut record_a = create_test_record(Utc::now());
        record_a.model = "model-a".to_string();
        record_a.input_tokens = Some(100);
        record_a.output_tokens = Some(50);
        record_a.total_tokens = None;
        storage.save_record(&record_a).await.unwrap();

        let mut record_b = create_test_record(Utc::now());
        record_b.id = Uuid::new_v4();
        record_b.model = "model-b".to_string();
        record_b.input_tokens = Some(10);
        record_b.output_tokens = None;
        record_b.total_tokens = None;
        storage.save_record(&record_b).await.unwrap();

        let stats = storage.get_token_statistics_by_model().await.unwrap();

        let model_a_stats = stats.iter().find(|s| s.model == "model-a").unwrap();
        assert_eq!(model_a_stats.total_tokens, 150);

        let model_b_stats = stats.iter().find(|s| s.model == "model-b").unwrap();
        assert_eq!(model_b_stats.total_tokens, 10);
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

    #[tokio::test]
    async fn test_daily_token_stats_infer_total_tokens_when_null() {
        let pool = create_test_pool().await;
        let storage = RequestHistoryStorage::new(pool);

        let mut record = create_test_record(Utc::now());
        record.input_tokens = Some(100);
        record.output_tokens = Some(50);
        record.total_tokens = None;
        storage.save_record(&record).await.unwrap();

        let stats = storage.get_daily_token_statistics(30).await.unwrap();
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].total_input_tokens, 100);
        assert_eq!(stats[0].total_output_tokens, 50);
        assert_eq!(stats[0].total_tokens, 150);
        assert_eq!(stats[0].request_count, 1);
    }

    #[tokio::test]
    async fn test_monthly_token_stats_infer_total_tokens_when_null() {
        let pool = create_test_pool().await;
        let storage = RequestHistoryStorage::new(pool);

        let mut record = create_test_record(Utc::now());
        record.input_tokens = Some(100);
        record.output_tokens = Some(50);
        record.total_tokens = None;
        storage.save_record(&record).await.unwrap();

        let stats = storage.get_monthly_token_statistics(12).await.unwrap();
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].total_input_tokens, 100);
        assert_eq!(stats[0].total_output_tokens, 50);
        assert_eq!(stats[0].total_tokens, 150);
        assert_eq!(stats[0].request_count, 1);
    }
}
