//! Durable usage receipts and daily rollups.

pub mod query;

use crate::common::{
    error::{CommonError, LbError},
    usage::{TokenSource, UsageAttribution, UsageTokens},
};
use chrono::SecondsFormat;
use sqlx::{Row, SqliteConnection, SqlitePool};
use uuid::Uuid;

type UsageResult<T> = Result<T, LbError>;

/// A known terminal result; absence means the outcome could not be proven.
#[derive(Debug, Clone, Copy)]
pub enum UsageOutcome {
    /// The downstream response body completed successfully.
    Success,
    /// A failed or interrupted response body.
    Error,
}

fn database_error(error: sqlx::Error) -> LbError {
    if error
        .as_database_error()
        .is_some_and(|database| database.is_check_violation())
    {
        tracing::error!(%error, "Inference usage storage constraint rejected a write");
        return LbError::Common(CommonError::Validation(
            "Inference usage is outside its supported storage bounds".to_string(),
        ));
    }
    LbError::Database(format!("Failed to persist inference usage: {error}"))
}

#[derive(sqlx::FromRow, PartialEq, Eq)]
struct ReceiptSnapshot {
    account_id: Option<String>,
    organization_id: Option<String>,
    received_at: String,
    date: String,
    api_key_id: Option<String>,
    auth_source: String,
}

impl From<&UsageAttribution> for ReceiptSnapshot {
    fn from(attribution: &UsageAttribution) -> Self {
        Self {
            account_id: attribution.account_id.map(|id| id.to_string()),
            organization_id: attribution.organization_id.map(|id| id.to_string()),
            received_at: attribution
                .received_at
                .to_rfc3339_opts(SecondsFormat::Nanos, true),
            date: attribution.received_at.date_naive().to_string(),
            api_key_id: attribution.api_key_id.map(|id| id.to_string()),
            auth_source: attribution.auth_source.clone(),
        }
    }
}

/// Persist acceptance and its request count atomically before forwarding.
pub async fn accept(pool: &SqlitePool, attribution: &UsageAttribution) -> UsageResult<()> {
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(database_error)?;
    if let Err(error) = accept_in_transaction(&mut transaction, attribution).await {
        let _ = transaction.rollback().await;
        return Err(error);
    }
    transaction.commit().await.map_err(database_error)
}

/// Acceptance inside an existing raw-history transaction.
///
/// The caller must begin a transaction and roll it back on any error. Receipts
/// keep only immutable metadata and once-state, without a second body ledger.
pub async fn accept_in_transaction(
    connection: &mut SqliteConnection,
    attribution: &UsageAttribution,
) -> UsageResult<()> {
    let snapshot = ReceiptSnapshot::from(attribution);
    let inserted = sqlx::query(
        "INSERT INTO usage_receipts
             (request_id, account_id, organization_id, received_at, date, api_key_id, auth_source)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(request_id) DO NOTHING",
    )
    .bind(attribution.request_id.to_string())
    .bind(&snapshot.account_id)
    .bind(&snapshot.organization_id)
    .bind(&snapshot.received_at)
    .bind(&snapshot.date)
    .bind(&snapshot.api_key_id)
    .bind(&snapshot.auth_source)
    .execute(&mut *connection)
    .await
    .map_err(database_error)?
    .rows_affected();

    if inserted == 0 {
        let original = sqlx::query_as::<_, ReceiptSnapshot>(
            "SELECT account_id, organization_id, received_at, date, api_key_id, auth_source
             FROM usage_receipts WHERE request_id = ?",
        )
        .bind(attribution.request_id.to_string())
        .fetch_one(&mut *connection)
        .await
        .map_err(database_error)?;
        if original != snapshot {
            return Err(LbError::Conflict(
                "Inference receipt attribution is immutable".to_string(),
            ));
        }
        return Ok(());
    }

    // Empty IDs are the unique unattributed bucket. No identity foreign keys
    // may cascade-delete historical usage when a user, key or org is deleted.
    sqlx::query(
        "INSERT INTO usage_daily
             (date, organization_id, account_id, request_count, pending_requests)
         VALUES (?, ?, ?, 1, 1)
         ON CONFLICT(date, organization_id, account_id) DO UPDATE SET
             request_count = request_count + 1,
             pending_requests = pending_requests + 1",
    )
    .bind(&snapshot.date)
    .bind(snapshot.organization_id.as_deref().unwrap_or_default())
    .bind(snapshot.account_id.as_deref().unwrap_or_default())
    .execute(&mut *connection)
    .await
    .map_err(database_error)?;
    Ok(())
}

struct StoredTokens {
    input: i64,
    output: i64,
    total: i64,
    input_missing: i64,
    output_missing: i64,
    total_missing: i64,
    source: TokenSource,
}

impl TryFrom<&UsageTokens> for StoredTokens {
    type Error = LbError;

    fn try_from(tokens: &UsageTokens) -> UsageResult<Self> {
        let any_value = tokens.input_tokens.is_some()
            || tokens.output_tokens.is_some()
            || tokens.total_tokens.is_some();
        if tokens.source == TokenSource::Missing && any_value {
            return Err(LbError::Common(CommonError::Validation(
                "Missing token usage cannot contain reported values".to_string(),
            )));
        }
        let checked = |value: Option<u64>| {
            i64::try_from(value.unwrap_or_default()).map_err(|_| {
                LbError::Common(CommonError::Validation(
                    "Token count exceeds the supported integer range".to_string(),
                ))
            })
        };
        Ok(Self {
            input: checked(tokens.input_tokens)?,
            output: checked(tokens.output_tokens)?,
            total: checked(tokens.total_tokens)?,
            input_missing: i64::from(tokens.input_tokens.is_none()),
            output_missing: i64::from(tokens.output_tokens.is_none()),
            total_missing: i64::from(tokens.total_tokens.is_none()),
            source: if any_value {
                tokens.source
            } else {
                TokenSource::Missing
            },
        })
    }
}

/// Apply the final token observation at most once, in the original UTC bucket.
pub async fn finish(pool: &SqlitePool, id: Uuid, tokens: &UsageTokens) -> UsageResult<()> {
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(database_error)?;
    if let Err(error) = finish_in_transaction(&mut transaction, id, tokens).await {
        let _ = transaction.rollback().await;
        return Err(error);
    }
    transaction.commit().await.map_err(database_error)
}

/// Finalize inside the same transaction as the existing raw-record completion.
///
/// The caller owns the transaction and must roll it back on error. Repeated or
/// delayed terminal callbacks cannot replace an already finalized observation.
pub async fn finish_in_transaction(
    connection: &mut SqliteConnection,
    id: Uuid,
    tokens: &UsageTokens,
) -> UsageResult<()> {
    finish_with_outcome_in_transaction(connection, id, tokens, None).await
}

/// Finalize tokens and a proven response outcome in the raw-history transaction.
pub async fn finish_with_outcome_in_transaction(
    connection: &mut SqliteConnection,
    id: Uuid,
    tokens: &UsageTokens,
    outcome: Option<UsageOutcome>,
) -> UsageResult<()> {
    let tokens = StoredTokens::try_from(tokens)?;
    let receipt = sqlx::query(
        "UPDATE usage_receipts
         SET finalized_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), token_source = ?, outcome = ?
         WHERE request_id = ? AND finalized_at IS NULL
         RETURNING date, organization_id, account_id",
    )
    .bind(tokens.source.as_str())
    .bind(outcome.map(|value| match value {
        UsageOutcome::Success => "success",
        UsageOutcome::Error => "error",
    }))
    .bind(id.to_string())
    .fetch_optional(&mut *connection)
    .await
    .map_err(database_error)?;

    let Some(receipt) = receipt else {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM usage_receipts WHERE request_id = ?)")
                .bind(id.to_string())
                .fetch_one(&mut *connection)
                .await
                .map_err(database_error)?;
        return if exists {
            Ok(())
        } else {
            Err(LbError::NotFound("Inference receipt not found".to_string()))
        };
    };

    let organization_id: Option<String> = receipt.get("organization_id");
    let account_id: Option<String> = receipt.get("account_id");
    let updated = sqlx::query(
        "UPDATE usage_daily SET
             pending_requests = pending_requests - 1,
             input_tokens = input_tokens + ?,
             output_tokens = output_tokens + ?,
             total_tokens = total_tokens + ?,
             provider_count = provider_count + ?,
             estimated_count = estimated_count + ?,
             missing_count = missing_count + ?,
             input_missing_count = input_missing_count + ?,
             output_missing_count = output_missing_count + ?,
             total_missing_count = total_missing_count + ?
             ,successful_requests = successful_requests + ?
             ,failed_requests = failed_requests + ?
         WHERE date = ? AND organization_id = ? AND account_id = ?",
    )
    .bind(tokens.input)
    .bind(tokens.output)
    .bind(tokens.total)
    .bind(i64::from(tokens.source == TokenSource::Provider))
    .bind(i64::from(tokens.source == TokenSource::Estimated))
    .bind(i64::from(tokens.source == TokenSource::Missing))
    .bind(tokens.input_missing)
    .bind(tokens.output_missing)
    .bind(tokens.total_missing)
    .bind(i64::from(matches!(outcome, Some(UsageOutcome::Success))))
    .bind(i64::from(matches!(outcome, Some(UsageOutcome::Error))))
    .bind(receipt.get::<String, _>("date"))
    .bind(organization_id.as_deref().unwrap_or_default())
    .bind(account_id.as_deref().unwrap_or_default())
    .execute(&mut *connection)
    .await
    .map_err(database_error)?
    .rows_affected();
    if updated != 1 {
        return Err(LbError::Database(
            "Inference receipt is missing its daily usage bucket".to_string(),
        ));
    }
    Ok(())
}

/// At startup, before accepting requests, settle interrupted receipts as missing.
///
/// This is intentionally not a live-request timeout: active bodies must retain
/// their own finalizer. Dedupe metadata is kept across raw retention indefinitely.
pub async fn recover_incomplete(pool: &SqlitePool) -> UsageResult<u64> {
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(database_error)?;
    let ids: Vec<String> =
        sqlx::query_scalar("SELECT request_id FROM usage_receipts WHERE finalized_at IS NULL")
            .fetch_all(&mut *transaction)
            .await
            .map_err(database_error)?;
    for id in &ids {
        let parsed = Uuid::parse_str(id).map_err(|error| {
            LbError::Database(format!("Invalid inference receipt UUID: {error}"))
        })?;
        if let Err(error) =
            finish_in_transaction(&mut transaction, parsed, &UsageTokens::missing()).await
        {
            let _ = transaction.rollback().await;
            return Err(error);
        }
    }
    transaction.commit().await.map_err(database_error)?;
    Ok(ids.len() as u64)
}

#[cfg(test)]
mod tests;
