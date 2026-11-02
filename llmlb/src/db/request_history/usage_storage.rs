//! Raw history and durable usage commit together; late writers never reset a terminal receipt.
use super::*;
use crate::common::usage::{UsageAttribution, UsageTokens};
use futures::FutureExt;
use sqlx::SqliteConnection;

/// Successful tasks are pruned, while a sticky failure survives until recovery
/// in a new storage lifetime. Otherwise a later drain could falsely report PASS.
pub(super) enum UsageWriter {
    Running(tokio::task::JoinHandle<RouterResult<()>>),
    Failed(String),
}

fn remember_failure(writers: &mut Vec<UsageWriter>, message: String) {
    // Every failure is logged by its writer and remains a durable pending
    // receipt. One sticky diagnostic is enough to keep drain fail-closed.
    if !writers
        .iter()
        .any(|writer| matches!(writer, UsageWriter::Failed(_)))
    {
        writers.push(UsageWriter::Failed(message));
    }
}

fn prune_completed(writers: &mut Vec<UsageWriter>) {
    let previous = std::mem::take(writers);
    for writer in previous {
        match writer {
            UsageWriter::Running(task) if task.is_finished() => match task.now_or_never() {
                Some(Ok(Ok(()))) => {}
                Some(Ok(Err(error))) => remember_failure(writers, error.to_string()),
                Some(Err(error)) => {
                    remember_failure(writers, format!("Terminal usage task failed: {error}"))
                }
                None => {
                    remember_failure(writers, "Terminal usage result is unavailable".to_string())
                }
            },
            UsageWriter::Failed(message) => remember_failure(writers, message),
            running => writers.push(running),
        }
    }
}

fn permanent(error: &LbError) -> bool {
    matches!(
        error,
        LbError::Common(crate::common::error::CommonError::Validation(_))
            | LbError::Conflict(_)
            | LbError::NotFound(_)
    )
}

fn database(error: sqlx::Error) -> LbError {
    if error
        .as_database_error()
        .is_some_and(|database| database.is_check_violation())
    {
        tracing::error!(%error, "Accepted inference metadata storage constraint rejected a write");
        return LbError::Common(crate::common::error::CommonError::Validation(
            "Accepted inference metadata is outside its supported storage bounds".to_string(),
        ));
    }
    LbError::Database(format!("Failed to persist accepted inference: {error}"))
}

impl RequestHistoryStorage {
    /// Persist the immutable receipt and initial raw record before any upstream send.
    pub async fn accept_usage(
        &self,
        attribution: &UsageAttribution,
        record: &RequestResponseRecord,
    ) -> RouterResult<()> {
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(database)?;
        crate::db::usage::accept_in_transaction(&mut transaction, attribution).await?;
        let finalized: Option<String> =
            sqlx::query_scalar("SELECT finalized_at FROM usage_receipts WHERE request_id=?")
                .bind(attribution.request_id.to_string())
                .fetch_one(&mut *transaction)
                .await
                .map_err(database)?;
        if finalized.is_none() {
            write_raw(&mut transaction, attribution, record, None, false).await?;
        }
        transaction.commit().await.map_err(database)
    }

    /// Commit final raw metadata and token rollup once in the same transaction.
    pub async fn finish_usage(
        &self,
        attribution: &UsageAttribution,
        record: &RequestResponseRecord,
        tokens: &UsageTokens,
    ) -> RouterResult<()> {
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(database)?;
        // This also verifies every immutable snapshot field on duplicate calls.
        crate::db::usage::accept_in_transaction(&mut transaction, attribution).await?;
        let finalized: Option<String> =
            sqlx::query_scalar("SELECT finalized_at FROM usage_receipts WHERE request_id=?")
                .bind(attribution.request_id.to_string())
                .fetch_one(&mut *transaction)
                .await
                .map_err(database)?;
        if finalized.is_none() {
            write_raw(&mut transaction, attribution, record, Some(tokens), true).await?;
            crate::db::usage::finish_with_outcome_in_transaction(
                &mut transaction,
                attribution.request_id,
                tokens,
                Some(if matches!(&record.status, RecordStatus::Success) {
                    crate::db::usage::UsageOutcome::Success
                } else {
                    crate::db::usage::UsageOutcome::Error
                }),
            )
            .await?;
        }
        transaction.commit().await.map_err(database)
    }

    /// Cancellation can race SQLite's COMMIT acknowledgement. Serialize with
    /// that transaction, then settle only a receipt which actually exists.
    /// Never call the creating finish path for a rejected/unstarted admission.
    async fn finish_usage_if_accepted(
        &self,
        attribution: &UsageAttribution,
        record: &RequestResponseRecord,
    ) -> RouterResult<()> {
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(database)?;
        let existing: Option<Option<String>> =
            sqlx::query_scalar("SELECT finalized_at FROM usage_receipts WHERE request_id=?")
                .bind(attribution.request_id.to_string())
                .fetch_optional(&mut *transaction)
                .await
                .map_err(database)?;
        if let Some(finalized) = existing {
            // Under the same write lock, this can only validate an existing
            // identity. The absent-receipt branch never inserts anything.
            crate::db::usage::accept_in_transaction(&mut transaction, attribution).await?;
            if finalized.is_none() {
                let mut terminal = record.clone();
                if matches!(terminal.status, RecordStatus::Success) {
                    terminal.status = RecordStatus::Error {
                        message: "Inference admission was cancelled before acknowledgement"
                            .to_string(),
                    };
                }
                let tokens = UsageTokens::missing();
                write_raw(
                    &mut transaction,
                    attribution,
                    &terminal,
                    Some(&tokens),
                    true,
                )
                .await?;
                crate::db::usage::finish_with_outcome_in_transaction(
                    &mut transaction,
                    attribution.request_id,
                    &tokens,
                    Some(crate::db::usage::UsageOutcome::Error),
                )
                .await?;
            }
        }
        transaction.commit().await.map_err(database)
    }

    /// Track EOF/error/Drop writes until they commit; an unavailable DB is retried.
    /// The persisted unfinished receipt remains recoverable after process shutdown.
    pub fn enqueue_usage_final(
        &self,
        attribution: UsageAttribution,
        record: RequestResponseRecord,
        tokens: UsageTokens,
    ) {
        self.enqueue_terminal(attribution, record, tokens, false);
    }

    /// Finalize a possibly committed admission after its parent future vanished.
    /// A receipt which never committed remains completely uncounted.
    pub fn enqueue_usage_cancellation(
        &self,
        attribution: UsageAttribution,
        record: RequestResponseRecord,
    ) {
        self.enqueue_terminal(attribution, record, UsageTokens::missing(), true);
    }

    fn enqueue_terminal(
        &self,
        attribution: UsageAttribution,
        record: RequestResponseRecord,
        tokens: UsageTokens,
        existing_only: bool,
    ) {
        let storage = self.clone();
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            tracing::error!(request_id=%attribution.request_id,"No runtime for terminal usage writer; persisted receipt requires startup recovery");
            let mut writers = self
                .usage_writers
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            remember_failure(
                &mut writers,
                format!(
                    "No runtime for terminal usage receipt {}",
                    attribution.request_id
                ),
            );
            return;
        };
        let task = runtime.spawn(async move {
            let mut delay = std::time::Duration::from_millis(25);
            loop {
                let result = if existing_only {
                    storage.finish_usage_if_accepted(&attribution,&record).await
                } else {
                    storage.finish_usage(&attribution,&record,&tokens).await
                };
                match result {
                    Ok(()) => return Ok(()),
                    Err(error) => {
                        tracing::error!(request_id=%attribution.request_id,%error,"Terminal usage write failed; retaining pending receipt");
                        if permanent(&error) || storage.pool.is_closed() {
                            return Err(error);
                        }
                        tokio::time::sleep(delay).await;
                        delay = (delay * 2).min(std::time::Duration::from_secs(2));
                    }
                }
            }
        });
        let mut writers = self
            .usage_writers
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        prune_completed(&mut writers);
        writers.push(UsageWriter::Running(task));
    }

    /// Await pending terminal writes before closing the application database.
    pub async fn drain_usage_writers(&self) -> RouterResult<()> {
        let mut failure: Option<String> = None;
        loop {
            let batch = {
                let mut writers = self
                    .usage_writers
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner());
                if writers.is_empty() {
                    return match failure {
                        Some(message) => {
                            remember_failure(&mut writers, message.clone());
                            Err(LbError::Database(format!("Unresolved terminal usage write; persisted receipt remains incomplete: {message}")))
                        }
                        None => Ok(()),
                    };
                }
                std::mem::take(&mut *writers)
            };
            for writer in batch {
                let result = match writer {
                    UsageWriter::Running(task) => match task.await {
                        Ok(result) => result.map_err(|error| error.to_string()),
                        Err(error) => Err(format!("Terminal usage task failed: {error}")),
                    },
                    UsageWriter::Failed(message) => Err(message),
                };
                if let Err(message) = result {
                    failure.get_or_insert(message);
                }
            }
        }
    }
}

async fn write_raw(
    connection: &mut SqliteConnection,
    attribution: &UsageAttribution,
    record: &RequestResponseRecord,
    tokens: Option<&UsageTokens>,
    terminal: bool,
) -> RouterResult<()> {
    let (status, error_message) = match &record.status {
        RecordStatus::Success => ("success", None),
        RecordStatus::Error { message } => ("error", Some(message.as_str())),
    };
    let token = |value: Option<u64>| {
        value.map(i64::try_from).transpose().map_err(|_| {
            LbError::Common(crate::common::error::CommonError::Validation(
                "Token count exceeds the supported integer range".into(),
            ))
        })
    };
    let input = token(tokens.and_then(|tokens| tokens.input_tokens))?;
    let output = token(tokens.and_then(|tokens| tokens.output_tokens))?;
    let total = token(tokens.and_then(|tokens| tokens.total_tokens))?;
    let base="INSERT INTO request_history (id,timestamp,request_type,model,endpoint_id,endpoint_name,endpoint_ip,client_ip,request_body,response_body,duration_ms,status,error_message,completed_at,input_tokens,output_tokens,total_tokens,api_key_id) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)";
    let suffix = if terminal {
        " ON CONFLICT(id) DO UPDATE SET model=excluded.model,endpoint_id=excluded.endpoint_id,endpoint_name=excluded.endpoint_name,endpoint_ip=excluded.endpoint_ip,client_ip=COALESCE(excluded.client_ip,request_history.client_ip),response_body=excluded.response_body,duration_ms=excluded.duration_ms,status=excluded.status,error_message=excluded.error_message,completed_at=excluded.completed_at,input_tokens=excluded.input_tokens,output_tokens=excluded.output_tokens,total_tokens=excluded.total_tokens"
    } else {
        " ON CONFLICT(id) DO NOTHING"
    };
    sqlx::query(&(base.to_string() + suffix))
        .bind(attribution.request_id.to_string())
        .bind(attribution.received_at.to_rfc3339())
        .bind(format!("{:?}", record.request_type))
        .bind(&record.model)
        .bind(record.endpoint_id.to_string())
        .bind(&record.endpoint_name)
        .bind(record.endpoint_ip.to_string())
        .bind(record.client_ip.map(|ip| ip.to_string()))
        .bind(record.request_body.to_string())
        .bind(
            record
                .response_body
                .as_ref()
                .map(serde_json::Value::to_string),
        )
        .bind(i64::try_from(record.duration_ms).unwrap_or(i64::MAX))
        .bind(status)
        .bind(error_message)
        .bind(record.completed_at.to_rfc3339())
        .bind(input)
        .bind(output)
        .bind(total)
        .bind(attribution.api_key_id.map(|id| id.to_string()))
        .execute(connection)
        .await
        .map_err(database)?;
    Ok(())
}
