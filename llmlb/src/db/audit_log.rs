//! 監査ログストレージ (SPEC-8301d106)

use crate::audit::types::{ActorType, AuditBatchHash, AuditLogEntry, AuditLogFilter};
use crate::common::error::{LbError, RouterResult};
use sqlx::SqlitePool;

mod archive;
mod batch_hash;
mod statistics;
#[cfg(test)]
mod tests;

pub use archive::create_archive_pool;
pub use statistics::{
    DailyTokenStatistics, ModelTokenStatistics, MonthlyTokenStatistics, TokenStatistics,
};

/// 監査ログのDB CRUD操作
#[derive(Clone)]
pub struct AuditLogStorage {
    pool: SqlitePool,
}

/// sqlx::FromRow用の行構造体
#[derive(Debug, sqlx::FromRow)]
struct AuditLogRow {
    id: i64,
    timestamp: String,
    http_method: String,
    request_path: String,
    status_code: i64,
    actor_type: String,
    actor_id: Option<String>,
    actor_username: Option<String>,
    api_key_owner_id: Option<String>,
    client_ip: Option<String>,
    duration_ms: Option<i64>,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    total_tokens: Option<i64>,
    model_name: Option<String>,
    endpoint_id: Option<String>,
    detail: Option<String>,
    batch_id: Option<i64>,
    is_migrated: i64,
}

/// sqlx::FromRow用の行構造体（バッチハッシュ）
#[derive(Debug, sqlx::FromRow)]
struct AuditBatchHashRow {
    id: i64,
    sequence_number: i64,
    batch_start: String,
    batch_end: String,
    record_count: i64,
    hash: String,
    previous_hash: String,
}

impl TryFrom<AuditBatchHashRow> for AuditBatchHash {
    type Error = LbError;

    fn try_from(row: AuditBatchHashRow) -> Result<Self, Self::Error> {
        let batch_start = chrono::DateTime::parse_from_rfc3339(&row.batch_start)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|e| LbError::Database(format!("Failed to parse batch_start: {}", e)))?;
        let batch_end = chrono::DateTime::parse_from_rfc3339(&row.batch_end)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|e| LbError::Database(format!("Failed to parse batch_end: {}", e)))?;

        Ok(AuditBatchHash {
            id: Some(row.id),
            sequence_number: row.sequence_number,
            batch_start,
            batch_end,
            record_count: row.record_count,
            hash: row.hash,
            previous_hash: row.previous_hash,
        })
    }
}

impl TryFrom<AuditLogRow> for AuditLogEntry {
    type Error = LbError;

    fn try_from(row: AuditLogRow) -> Result<Self, Self::Error> {
        let timestamp = chrono::DateTime::parse_from_rfc3339(&row.timestamp)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|e| LbError::Database(format!("Failed to parse timestamp: {}", e)))?;

        let status_code = u16::try_from(row.status_code)
            .map_err(|e| LbError::Database(format!("Invalid status_code: {}", e)))?;

        Ok(AuditLogEntry {
            id: Some(row.id),
            timestamp,
            http_method: row.http_method,
            request_path: row.request_path,
            status_code,
            actor_type: ActorType::from_str(&row.actor_type),
            actor_id: row.actor_id,
            actor_username: row.actor_username,
            api_key_owner_id: row.api_key_owner_id,
            client_ip: row.client_ip,
            duration_ms: row.duration_ms,
            input_tokens: row.input_tokens,
            output_tokens: row.output_tokens,
            total_tokens: row.total_tokens,
            model_name: row.model_name,
            endpoint_id: row.endpoint_id,
            detail: row.detail,
            batch_id: row.batch_id,
            is_migrated: row.is_migrated != 0,
        })
    }
}

impl AuditLogStorage {
    /// 新しいAuditLogStorageを作成
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 監査ログを一括挿入
    pub async fn insert_batch(&self, entries: &[AuditLogEntry]) -> RouterResult<()> {
        if entries.is_empty() {
            return Ok(());
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| LbError::Database(format!("Failed to begin transaction: {}", e)))?;

        for entry in entries {
            let timestamp_str = entry.timestamp.to_rfc3339();
            let actor_type_str = entry.actor_type.as_str();
            let status_code = entry.status_code as i64;
            let is_migrated: i64 = if entry.is_migrated { 1 } else { 0 };

            sqlx::query(
                r#"INSERT INTO audit_log_entries (
                    timestamp, http_method, request_path, status_code,
                    actor_type, actor_id, actor_username, api_key_owner_id,
                    client_ip, duration_ms, input_tokens, output_tokens,
                    total_tokens, model_name, endpoint_id, detail,
                    batch_id, is_migrated
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            )
            .bind(&timestamp_str)
            .bind(&entry.http_method)
            .bind(&entry.request_path)
            .bind(status_code)
            .bind(actor_type_str)
            .bind(&entry.actor_id)
            .bind(&entry.actor_username)
            .bind(&entry.api_key_owner_id)
            .bind(&entry.client_ip)
            .bind(entry.duration_ms)
            .bind(entry.input_tokens)
            .bind(entry.output_tokens)
            .bind(entry.total_tokens)
            .bind(&entry.model_name)
            .bind(&entry.endpoint_id)
            .bind(&entry.detail)
            .bind(entry.batch_id)
            .bind(is_migrated)
            .execute(&mut *tx)
            .await
            .map_err(|e| LbError::Database(format!("Failed to insert audit log: {}", e)))?;
        }

        tx.commit()
            .await
            .map_err(|e| LbError::Database(format!("Failed to commit transaction: {}", e)))?;

        Ok(())
    }

    /// フィルタ条件に基づいて監査ログを検索
    pub async fn query(&self, filter: &AuditLogFilter) -> RouterResult<Vec<AuditLogEntry>> {
        let (where_clause, bind_values) = build_where_clause(filter);
        let page = filter.page.unwrap_or(1).max(1);
        let per_page = filter.per_page.unwrap_or(50).max(1);
        let offset = page.saturating_sub(1).saturating_mul(per_page);

        let sql = format!(
            "SELECT id, timestamp, http_method, request_path, status_code, \
             actor_type, actor_id, actor_username, api_key_owner_id, client_ip, \
             duration_ms, input_tokens, output_tokens, total_tokens, \
             model_name, endpoint_id, detail, batch_id, is_migrated \
             FROM audit_log_entries {} ORDER BY timestamp DESC LIMIT ? OFFSET ?",
            where_clause
        );

        let mut query = sqlx::query_as::<_, AuditLogRow>(&sql);
        for val in &bind_values {
            query = query.bind(val.as_str());
        }
        query = query.bind(per_page).bind(offset);

        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| LbError::Database(format!("Failed to query audit logs: {}", e)))?;

        rows.into_iter()
            .map(AuditLogEntry::try_from)
            .collect::<Result<Vec<_>, _>>()
    }

    /// フィルタ条件に基づいてレコード数を取得
    pub async fn count(&self, filter: &AuditLogFilter) -> RouterResult<i64> {
        let (where_clause, bind_values) = build_where_clause(filter);
        let sql = format!(
            "SELECT COUNT(*) as cnt FROM audit_log_entries {}",
            where_clause
        );

        let mut query = sqlx::query_scalar::<_, i64>(&sql);
        for val in &bind_values {
            query = query.bind(val.as_str());
        }

        query
            .fetch_one(&self.pool)
            .await
            .map_err(|e| LbError::Database(format!("Failed to count audit logs: {}", e)))
    }

    /// IDで監査ログを取得
    pub async fn get_by_id(&self, id: i64) -> RouterResult<Option<AuditLogEntry>> {
        let row = sqlx::query_as::<_, AuditLogRow>(
            "SELECT id, timestamp, http_method, request_path, status_code, \
             actor_type, actor_id, actor_username, api_key_owner_id, client_ip, \
             duration_ms, input_tokens, output_tokens, total_tokens, \
             model_name, endpoint_id, detail, batch_id, is_migrated \
             FROM audit_log_entries WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| LbError::Database(format!("Failed to get audit log by id: {}", e)))?;

        match row {
            Some(r) => Ok(Some(AuditLogEntry::try_from(r)?)),
            None => Ok(None),
        }
    }

    /// FTS5全文検索で監査ログを検索
    pub async fn search_fts(
        &self,
        search_query: &str,
        filter: &AuditLogFilter,
    ) -> RouterResult<Vec<AuditLogEntry>> {
        let sanitized = sanitize_fts_query(search_query);
        if sanitized.is_empty() {
            return Ok(Vec::new());
        }

        let (where_clause, bind_values) = build_where_clause(filter);
        let page = filter.page.unwrap_or(1).max(1);
        let per_page = filter.per_page.unwrap_or(50).max(1);
        let offset = page.saturating_sub(1).saturating_mul(per_page);

        let extra_where = build_extra_where(&where_clause);

        let sql = format!(
            "SELECT e.id, e.timestamp, e.http_method, e.request_path, e.status_code, \
             e.actor_type, e.actor_id, e.actor_username, e.api_key_owner_id, e.client_ip, \
             e.duration_ms, e.input_tokens, e.output_tokens, e.total_tokens, \
             e.model_name, e.endpoint_id, e.detail, e.batch_id, e.is_migrated \
             FROM audit_log_fts fts \
             JOIN audit_log_entries e ON fts.rowid = e.id \
             WHERE fts.audit_log_fts MATCH ? {} \
             ORDER BY e.timestamp DESC LIMIT ? OFFSET ?",
            extra_where
        );

        let mut query = sqlx::query_as::<_, AuditLogRow>(&sql).bind(&sanitized);
        for val in &bind_values {
            query = query.bind(val.as_str());
        }
        query = query.bind(per_page).bind(offset);

        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| LbError::Database(format!("Failed to search audit logs: {}", e)))?;

        rows.into_iter()
            .map(AuditLogEntry::try_from)
            .collect::<Result<Vec<_>, _>>()
    }

    /// FTS5検索結果のカウント
    pub async fn count_fts(
        &self,
        search_query: &str,
        filter: &AuditLogFilter,
    ) -> RouterResult<i64> {
        let sanitized = sanitize_fts_query(search_query);
        if sanitized.is_empty() {
            return Ok(0);
        }

        let (where_clause, bind_values) = build_where_clause(filter);
        let extra_where = build_extra_where(&where_clause);

        let sql = format!(
            "SELECT COUNT(*) FROM audit_log_fts fts \
             JOIN audit_log_entries e ON fts.rowid = e.id \
             WHERE fts.audit_log_fts MATCH ? {}",
            extra_where
        );

        let mut query = sqlx::query_scalar::<_, i64>(&sql).bind(&sanitized);
        for val in &bind_values {
            query = query.bind(val.as_str());
        }

        query
            .fetch_one(&self.pool)
            .await
            .map_err(|e| LbError::Database(format!("Failed to count FTS results: {}", e)))
    }

    /// バッチ未割当のエントリを取得（batch_id IS NULL AND is_migrated = 0）
    pub async fn get_unbatched_entries(&self) -> RouterResult<Vec<AuditLogEntry>> {
        let rows = sqlx::query_as::<_, AuditLogRow>(
            "SELECT id, timestamp, http_method, request_path, status_code, \
             actor_type, actor_id, actor_username, api_key_owner_id, client_ip, \
             duration_ms, input_tokens, output_tokens, total_tokens, \
             model_name, endpoint_id, detail, batch_id, is_migrated \
             FROM audit_log_entries WHERE batch_id IS NULL AND is_migrated = 0 \
             ORDER BY timestamp ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| LbError::Database(format!("Failed to get unbatched entries: {}", e)))?;

        rows.into_iter()
            .map(AuditLogEntry::try_from)
            .collect::<Result<Vec<_>, _>>()
    }

    /// HTTPメソッド別のエントリ数を取得
    pub async fn count_by_method(&self) -> RouterResult<Vec<(String, i64)>> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT http_method, COUNT(*) as cnt FROM audit_log_entries GROUP BY http_method ORDER BY cnt DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| LbError::Database(format!("Failed to count by method: {}", e)))?;
        Ok(rows)
    }

    /// アクター種別のエントリ数を取得
    pub async fn count_by_actor_type(&self) -> RouterResult<Vec<(String, i64)>> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT actor_type, COUNT(*) as cnt FROM audit_log_entries GROUP BY actor_type ORDER BY cnt DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| LbError::Database(format!("Failed to count by actor type: {}", e)))?;
        Ok(rows)
    }
}

/// フィルタからWHERE句とバインド値を構築
fn build_where_clause(filter: &AuditLogFilter) -> (String, Vec<String>) {
    let mut conditions: Vec<String> = Vec::new();
    let mut bind_values: Vec<String> = Vec::new();

    if let Some(ref actor_type) = filter.actor_type {
        conditions.push("actor_type = ?".to_string());
        bind_values.push(actor_type.clone());
    }

    if let Some(ref actor_id) = filter.actor_id {
        conditions.push("actor_id = ?".to_string());
        bind_values.push(actor_id.clone());
    }

    if let Some(ref http_method) = filter.http_method {
        conditions.push("http_method = ?".to_string());
        bind_values.push(http_method.clone());
    }

    if let Some(ref request_path) = filter.request_path {
        conditions.push("request_path = ?".to_string());
        bind_values.push(request_path.clone());
    }

    if let Some(status_code) = filter.status_code {
        conditions.push("status_code = ?".to_string());
        bind_values.push(status_code.to_string());
    }

    if let Some(ref time_from) = filter.time_from {
        conditions.push("timestamp >= ?".to_string());
        bind_values.push(time_from.to_rfc3339());
    }

    if let Some(ref time_to) = filter.time_to {
        conditions.push("timestamp <= ?".to_string());
        bind_values.push(time_to.to_rfc3339());
    }

    if let Some(ref client_ip) = filter.client_ip {
        conditions.push("client_ip LIKE ?".to_string());
        bind_values.push(format!("{}%", client_ip));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    (where_clause, bind_values)
}

/// WHERE句をFTS JOIN用のAND条件に変換
fn build_extra_where(where_clause: &str) -> String {
    if where_clause.is_empty() {
        String::new()
    } else {
        // "WHERE x = ? AND y = ?" -> "AND x = ? AND y = ?"
        // build_where_clauseでカラム名にテーブルプレフィックスがないので
        // JOINクエリ用にeプレフィックスを付与
        let conditions = &where_clause[6..]; // "WHERE "を除去
        let prefixed = conditions
            .replace("actor_type", "e.actor_type")
            .replace("actor_id", "e.actor_id")
            .replace("http_method", "e.http_method")
            .replace("request_path", "e.request_path")
            .replace("status_code", "e.status_code")
            .replace("timestamp", "e.timestamp")
            .replace("client_ip", "e.client_ip");
        format!("AND {}", prefixed)
    }
}

/// FTS5クエリの特殊文字をサニタイズ
fn sanitize_fts_query(query: &str) -> String {
    query
        .split_whitespace()
        .map(|word| {
            let clean = word.replace('"', "");
            if clean.is_empty() {
                return String::new();
            }
            format!("\"{}\"", clean)
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
