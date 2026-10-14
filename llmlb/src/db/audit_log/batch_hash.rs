//! 監査ログのバッチハッシュ（ハッシュチェーン）の保存・移行

use super::{AuditBatchHashRow, AuditLogRow, AuditLogStorage};
use crate::audit::{
    hash_chain::{self, GENESIS_HASH},
    types::{AuditBatchHash, AuditLogEntry},
};
use crate::common::error::{LbError, RouterResult};
use std::collections::HashSet;

impl AuditLogStorage {
    /// `client_ip` を含まない v1 バッチを現行 v2 ハッシュへ一度だけ移行する。
    ///
    /// 保存済みチェーンが v1 または v2 のどちらかで正当な場合だけ全バッチを
    /// v2 で再計算する。更新と方式バージョンの確定は同一トランザクションで
    /// 行い、改ざんや途中失敗を新しい基準として確定しない。
    pub(crate) async fn migrate_hash_chain_to_v2(&self) -> RouterResult<i64> {
        self.migrate_hash_chain_to_v2_inner(false).await
    }

    /// アーカイブ済みチェーンを、外部DBにある直前バッチへの参照を保って移行する。
    pub(super) async fn migrate_archive_hash_chain_to_v2(&self) -> RouterResult<i64> {
        self.migrate_hash_chain_to_v2_inner(true).await
    }

    async fn migrate_hash_chain_to_v2_inner(
        &self,
        allow_external_predecessors: bool,
    ) -> RouterResult<i64> {
        const CURRENT_VERSION: i64 = 2;

        let mut tx = self.pool.begin().await.map_err(|e| {
            LbError::Database(format!("Failed to begin audit hash chain migration: {e}"))
        })?;
        let version: i64 =
            sqlx::query_scalar("SELECT algorithm_version FROM audit_hash_chain_state WHERE id = 1")
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| {
                    LbError::Database(format!("Failed to read audit hash chain version: {e}"))
                })?;

        if version == CURRENT_VERSION {
            tx.commit().await.map_err(|e| {
                LbError::Database(format!("Failed to finish audit hash chain check: {e}"))
            })?;
            return Ok(0);
        }
        if version != 1 {
            return Err(LbError::Database(format!(
                "Unsupported audit hash chain version: {version}"
            )));
        }

        let rows = sqlx::query_as::<_, AuditBatchHashRow>(
            "SELECT id, sequence_number, batch_start, batch_end, \
             record_count, hash, previous_hash \
             FROM audit_batch_hashes ORDER BY sequence_number ASC",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| {
            LbError::Database(format!(
                "Failed to load audit hash chain for migration: {e}"
            ))
        })?;

        let stored_hashes = rows
            .iter()
            .map(|row| row.hash.clone())
            .collect::<HashSet<_>>();
        let mut seen_hashes = HashSet::with_capacity(rows.len());
        let mut expected_stored_previous = GENESIS_HASH.to_string();
        let mut continuous_new_previous = GENESIS_HASH.to_string();
        let mut previous_sequence = None;
        let mut updates = Vec::with_capacity(rows.len());

        for row in rows {
            let batch = AuditBatchHash::try_from(row)?;
            let batch_id = batch.id.ok_or_else(|| {
                LbError::Database("Audit hash chain batch is missing its id".to_string())
            })?;

            if !seen_hashes.insert(batch.hash.clone()) {
                return Err(LbError::Database(format!(
                    "Invalid audit hash chain before migration at batch {}: duplicate hash",
                    batch.sequence_number
                )));
            }

            if !allow_external_predecessors && batch.previous_hash != expected_stored_previous {
                return Err(LbError::Database(format!(
                    "Invalid audit hash chain before migration at batch {}: previous hash mismatch",
                    batch.sequence_number
                )));
            }

            let new_previous = if allow_external_predecessors {
                let is_first_archived_batch = previous_sequence.is_none();
                if is_first_archived_batch {
                    let valid_external_anchor = batch.previous_hash.len() == 64
                        && batch
                            .previous_hash
                            .bytes()
                            .all(|byte| byte.is_ascii_hexdigit());
                    if !valid_external_anchor
                        || stored_hashes.contains(&batch.previous_hash)
                        || (batch.sequence_number == 1 && batch.previous_hash != GENESIS_HASH)
                    {
                        return Err(LbError::Database(format!(
                            "Invalid audit hash chain before migration at batch {}: invalid external predecessor",
                            batch.sequence_number
                        )));
                    }
                    batch.previous_hash.clone()
                } else if batch.previous_hash == GENESIS_HASH {
                    // Each archival run re-roots the first remaining main-DB batch at genesis.
                    // A later run can therefore append a genesis-rooted segment even when its
                    // sequence immediately follows the preceding archived segment.
                    GENESIS_HASH.to_string()
                } else if previous_sequence.is_some_and(|sequence: i64| {
                    sequence.checked_add(1) == Some(batch.sequence_number)
                }) {
                    if batch.previous_hash != expected_stored_previous {
                        return Err(LbError::Database(format!(
                            "Invalid audit hash chain before migration at batch {}: contiguous predecessor mismatch",
                            batch.sequence_number
                        )));
                    }
                    continuous_new_previous.clone()
                } else {
                    return Err(LbError::Database(format!(
                        "Invalid audit hash chain before migration at batch {}: invalid external predecessor",
                        batch.sequence_number
                    )));
                }
            } else {
                continuous_new_previous.clone()
            };

            if allow_external_predecessors
                && previous_sequence.is_some_and(|sequence| sequence >= batch.sequence_number)
            {
                return Err(LbError::Database(format!(
                    "Invalid audit hash chain before migration at batch {}: sequence order mismatch",
                    batch.sequence_number
                )));
            }

            let entry_rows = sqlx::query_as::<_, AuditLogRow>(
                "SELECT id, timestamp, http_method, request_path, status_code, \
                 actor_type, actor_id, actor_username, api_key_owner_id, client_ip, \
                 duration_ms, input_tokens, output_tokens, total_tokens, \
                 model_name, endpoint_id, detail, batch_id, is_migrated \
                 FROM audit_log_entries WHERE batch_id = ? ORDER BY timestamp ASC",
            )
            .bind(batch_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| {
                LbError::Database(format!(
                    "Failed to load audit batch {} for migration: {e}",
                    batch.sequence_number
                ))
            })?;
            let entries = entry_rows
                .into_iter()
                .map(AuditLogEntry::try_from)
                .collect::<Result<Vec<_>, _>>()?;

            if batch.record_count != entries.len() as i64 {
                return Err(LbError::Database(format!(
                    "Invalid audit hash chain before migration at batch {}: record count mismatch",
                    batch.sequence_number
                )));
            }

            let current_hash = hash_chain::compute_batch_hash(
                &batch.previous_hash,
                batch.sequence_number,
                &batch.batch_start,
                &batch.batch_end,
                batch.record_count,
                &entries,
            );
            let legacy_hash = hash_chain::compute_legacy_batch_hash(
                &batch.previous_hash,
                batch.sequence_number,
                &batch.batch_start,
                &batch.batch_end,
                batch.record_count,
                &entries,
            );
            if batch.hash != current_hash && batch.hash != legacy_hash {
                return Err(LbError::Database(format!(
                    "Invalid audit hash chain before migration at batch {}: hash mismatch",
                    batch.sequence_number
                )));
            }

            let new_hash = hash_chain::compute_batch_hash(
                &new_previous,
                batch.sequence_number,
                &batch.batch_start,
                &batch.batch_end,
                batch.record_count,
                &entries,
            );
            updates.push((batch_id, new_hash.clone(), new_previous));
            expected_stored_previous = batch.hash;
            continuous_new_previous = new_hash;
            previous_sequence = Some(batch.sequence_number);
        }

        let migrated_count = updates.len() as i64;
        for (batch_id, hash, previous_hash) in updates {
            sqlx::query("UPDATE audit_batch_hashes SET hash = ?, previous_hash = ? WHERE id = ?")
                .bind(hash)
                .bind(previous_hash)
                .bind(batch_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    LbError::Database(format!("Failed to update audit hash chain: {e}"))
                })?;
        }

        let result = sqlx::query(
            "UPDATE audit_hash_chain_state SET algorithm_version = ? \
             WHERE id = 1 AND algorithm_version = 1",
        )
        .bind(CURRENT_VERSION)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            LbError::Database(format!(
                "Failed to finalize audit hash chain migration: {e}"
            ))
        })?;
        if result.rows_affected() != 1 {
            return Err(LbError::Database(
                "Audit hash chain version changed during migration".to_string(),
            ));
        }

        tx.commit().await.map_err(|e| {
            LbError::Database(format!("Failed to commit audit hash chain migration: {e}"))
        })?;

        Ok(migrated_count)
    }

    /// バッチハッシュを挿入してIDを返す
    pub async fn insert_batch_hash(&self, batch: &AuditBatchHash) -> RouterResult<i64> {
        let batch_start_str = batch.batch_start.to_rfc3339();
        let batch_end_str = batch.batch_end.to_rfc3339();

        let result = sqlx::query(
            r#"INSERT INTO audit_batch_hashes (
                sequence_number, batch_start, batch_end, record_count, hash, previous_hash
            ) VALUES (?, ?, ?, ?, ?, ?)"#,
        )
        .bind(batch.sequence_number)
        .bind(&batch_start_str)
        .bind(&batch_end_str)
        .bind(batch.record_count)
        .bind(&batch.hash)
        .bind(&batch.previous_hash)
        .execute(&self.pool)
        .await
        .map_err(|e| LbError::Database(format!("Failed to insert batch hash: {}", e)))?;

        Ok(result.last_insert_rowid())
    }

    /// 全バッチハッシュを連番順で取得
    pub async fn get_all_batch_hashes(&self) -> RouterResult<Vec<AuditBatchHash>> {
        let rows = sqlx::query_as::<_, AuditBatchHashRow>(
            "SELECT id, sequence_number, batch_start, batch_end, \
             record_count, hash, previous_hash \
             FROM audit_batch_hashes ORDER BY sequence_number ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| LbError::Database(format!("Failed to get batch hashes: {}", e)))?;

        rows.into_iter()
            .map(AuditBatchHash::try_from)
            .collect::<Result<Vec<_>, _>>()
    }

    /// 最新バッチハッシュを取得
    pub async fn get_latest_batch_hash(&self) -> RouterResult<Option<AuditBatchHash>> {
        let row = sqlx::query_as::<_, AuditBatchHashRow>(
            "SELECT id, sequence_number, batch_start, batch_end, \
             record_count, hash, previous_hash \
             FROM audit_batch_hashes ORDER BY sequence_number DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| LbError::Database(format!("Failed to get latest batch hash: {}", e)))?;

        match row {
            Some(r) => Ok(Some(AuditBatchHash::try_from(r)?)),
            None => Ok(None),
        }
    }

    /// バッチ内エントリを取得
    pub async fn get_entries_for_batch(&self, batch_id: i64) -> RouterResult<Vec<AuditLogEntry>> {
        let rows = sqlx::query_as::<_, AuditLogRow>(
            "SELECT id, timestamp, http_method, request_path, status_code, \
             actor_type, actor_id, actor_username, api_key_owner_id, client_ip, \
             duration_ms, input_tokens, output_tokens, total_tokens, \
             model_name, endpoint_id, detail, batch_id, is_migrated \
             FROM audit_log_entries WHERE batch_id = ? ORDER BY timestamp ASC",
        )
        .bind(batch_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| LbError::Database(format!("Failed to get entries for batch: {}", e)))?;

        rows.into_iter()
            .map(AuditLogEntry::try_from)
            .collect::<Result<Vec<_>, _>>()
    }

    /// エントリのbatch_idを更新
    pub async fn update_entries_batch_id(
        &self,
        entry_ids: &[i64],
        batch_id: i64,
    ) -> RouterResult<()> {
        if entry_ids.is_empty() {
            return Ok(());
        }

        let placeholders: Vec<&str> = entry_ids.iter().map(|_| "?").collect();
        let sql = format!(
            "UPDATE audit_log_entries SET batch_id = ? WHERE id IN ({})",
            placeholders.join(", ")
        );

        let mut query = sqlx::query(&sql).bind(batch_id);
        for id in entry_ids {
            query = query.bind(id);
        }

        query
            .execute(&self.pool)
            .await
            .map_err(|e| LbError::Database(format!("Failed to update batch_id: {}", e)))?;

        Ok(())
    }

    /// バッチハッシュ挿入とエントリの batch_id 更新を単一トランザクションで実行
    ///
    /// `insert_batch_hash`（バッチ行のINSERT）と `update_entries_batch_id`
    /// （対象エントリのbatch_id更新）を同一の sqlx トランザクションに包む。
    /// batch_id 更新が失敗した場合はバッチ行の挿入もロールバックされるため、
    /// 「record_count=N のバッチ行は残るのに N 件のエントリは batch_id=NULL のまま」
    /// という状態が生じない。これがないと未割当エントリが再取得されて別バッチへ
    /// 二重に組み込まれ、ハッシュチェーン検証が恒久的に不整合となりうる。
    ///
    /// ハッシュ計算自体は呼び出し側で行い、ここではトランザクション境界のみを担う。
    pub async fn insert_batch_hash_with_entries(
        &self,
        batch: &AuditBatchHash,
        entry_ids: &[i64],
    ) -> RouterResult<i64> {
        let batch_start_str = batch.batch_start.to_rfc3339();
        let batch_end_str = batch.batch_end.to_rfc3339();

        let mut tx = self.pool.begin().await.map_err(|e| {
            LbError::Database(format!("Failed to begin batch hash transaction: {}", e))
        })?;

        let result = sqlx::query(
            r#"INSERT INTO audit_batch_hashes (
                sequence_number, batch_start, batch_end, record_count, hash, previous_hash
            ) VALUES (?, ?, ?, ?, ?, ?)"#,
        )
        .bind(batch.sequence_number)
        .bind(&batch_start_str)
        .bind(&batch_end_str)
        .bind(batch.record_count)
        .bind(&batch.hash)
        .bind(&batch.previous_hash)
        .execute(&mut *tx)
        .await
        .map_err(|e| LbError::Database(format!("Failed to insert batch hash: {}", e)))?;

        let batch_id = result.last_insert_rowid();

        if !entry_ids.is_empty() {
            let placeholders: Vec<&str> = entry_ids.iter().map(|_| "?").collect();
            let sql = format!(
                "UPDATE audit_log_entries SET batch_id = ? WHERE id IN ({})",
                placeholders.join(", ")
            );

            let mut query = sqlx::query(&sql).bind(batch_id);
            for id in entry_ids {
                query = query.bind(id);
            }

            query
                .execute(&mut *tx)
                .await
                .map_err(|e| LbError::Database(format!("Failed to update batch_id: {}", e)))?;
        }

        tx.commit().await.map_err(|e| {
            LbError::Database(format!("Failed to commit batch hash transaction: {}", e))
        })?;

        Ok(batch_id)
    }
}
