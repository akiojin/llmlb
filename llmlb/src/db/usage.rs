//! Transactional, idempotent usage settlement independent of raw retention.

use crate::common::error::{LbError, RouterResult};
use crate::common::usage::{TokenMeasurement, TokenSource, UsageAttribution, UsageTokens};
use sqlx::{Sqlite, Transaction};

#[cfg(test)]
mod tests;

#[derive(sqlx::FromRow)]
struct StoredReceipt {
    id: String,
    received_at: String,
    organization_id: Option<String>,
    account_id: Option<String>,
    api_key_id: Option<String>,
    auth_source: String,
    endpoint_id: String,
    model: String,
    finalized: bool,
    successful: bool,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    total_tokens: Option<i64>,
    input_source: String,
    output_source: String,
    total_source: String,
}

impl StoredReceipt {
    fn tokens(&self) -> UsageTokens {
        let measurement = |value: Option<i64>, source: &str| {
            TokenMeasurement::new(
                value.and_then(|value| u32::try_from(value).ok()),
                TokenSource::from_stored(source),
            )
        };
        UsageTokens {
            input: measurement(self.input_tokens, &self.input_source),
            output: measurement(self.output_tokens, &self.output_source),
            total: measurement(self.total_tokens, &self.total_source),
        }
    }
}

fn database_error(error: sqlx::Error) -> LbError {
    LbError::Database(format!("Failed to settle request usage: {error}"))
}

/// Persist an admission/terminal observation in the caller's raw-save transaction.
/// Repeated saves replace the previous contribution instead of adding requests.
#[allow(clippy::too_many_arguments)]
pub async fn save_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    attribution: &UsageAttribution,
    endpoint_id: &str,
    model: &str,
    tokens: &UsageTokens,
    finalized: bool,
    successful: bool,
) -> RouterResult<bool> {
    let id = attribution.request_id.to_string();
    let received_at = attribution.received_at.to_rfc3339();
    let organization_id = attribution.organization_id.map(|id| id.to_string());
    let account_id = attribution.account_id.map(|id| id.to_string());
    let api_key_id = attribution.api_key_id.map(|id| id.to_string());
    // Write first: acquire SQLite's writer lock before reading the previous
    // contribution, so concurrent initial/terminal tasks cannot lose updates.
    let inserted = sqlx::query(
        "INSERT OR IGNORE INTO usage_receipts (id, received_at, organization_id, account_id, api_key_id, auth_source, endpoint_id, model, finalized, successful, input_tokens, output_tokens, total_tokens, input_source, output_source, total_source) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&received_at)
    .bind(&organization_id)
    .bind(&account_id)
    .bind(&api_key_id)
    .bind(&attribution.auth_source)
    .bind(endpoint_id)
    .bind(model)
    .bind(finalized)
    .bind(successful)
    .bind(tokens.input.value.map(i64::from))
    .bind(tokens.output.value.map(i64::from))
    .bind(tokens.total.value.map(i64::from))
    .bind(tokens.input.source.as_str())
    .bind(tokens.output.source.as_str())
    .bind(tokens.total.source.as_str())
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?
    .rows_affected() == 1;

    let old = sqlx::query_as::<_, StoredReceipt>("SELECT id, received_at, organization_id, account_id, api_key_id, auth_source, endpoint_id, model, finalized, successful, input_tokens, output_tokens, total_tokens, input_source, output_source, total_source FROM usage_receipts WHERE id = ?")
        .bind(&id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(database_error)?;
    if old.received_at != received_at
        || old.organization_id != organization_id
        || old.account_id != account_id
        || old.api_key_id != api_key_id
        || old.auth_source != attribution.auth_source
    {
        return Err(LbError::Database(
            "Usage receipt identity is immutable".into(),
        ));
    }

    let late_initial = old.finalized && !finalized;
    let merged_tokens = if late_initial {
        old.tokens()
    } else {
        old.tokens().merge(tokens)
    };
    let merged_finalized = old.finalized || finalized;
    // A late initial save must never change the already recorded terminal result.
    let merged_successful = if old.finalized {
        old.successful
    } else {
        successful
    };
    let merged_endpoint = if late_initial || endpoint_id.is_empty() {
        &old.endpoint_id
    } else {
        endpoint_id
    };
    let merged_model = if late_initial || model.is_empty() {
        &old.model
    } else {
        model
    };
    let date = attribution.received_at.format("%Y-%m-%d").to_string();
    if !inserted {
        adjust_daily(
            transaction,
            &date,
            organization_id.as_deref().unwrap_or(""),
            account_id.as_deref().unwrap_or(""),
            &old.endpoint_id,
            &old.model,
            &old.tokens(),
            old.finalized,
            old.successful,
            -1,
        )
        .await?;
    }
    adjust_daily(
        transaction,
        &date,
        organization_id.as_deref().unwrap_or(""),
        account_id.as_deref().unwrap_or(""),
        merged_endpoint,
        merged_model,
        &merged_tokens,
        merged_finalized,
        merged_successful,
        1,
    )
    .await?;

    sqlx::query("UPDATE usage_receipts SET endpoint_id = ?, model = ?, finalized = ?, successful = ?, input_tokens = ?, output_tokens = ?, total_tokens = ?, input_source = ?, output_source = ?, total_source = ? WHERE id = ?")
        .bind(merged_endpoint)
        .bind(merged_model)
        .bind(merged_finalized)
        .bind(merged_successful)
        .bind(merged_tokens.input.value.map(i64::from))
        .bind(merged_tokens.output.value.map(i64::from))
        .bind(merged_tokens.total.value.map(i64::from))
        .bind(merged_tokens.input.source.as_str())
        .bind(merged_tokens.output.source.as_str())
        .bind(merged_tokens.total.source.as_str())
        .bind(&id)
        .execute(&mut **transaction)
        .await
        .map_err(database_error)?;
    sqlx::query("DELETE FROM usage_daily WHERE request_count = 0")
        .execute(&mut **transaction)
        .await
        .map_err(database_error)?;
    Ok(inserted)
}

/// Before the listener starts, settle admissions whose process stopped before
/// terminal commit. Known observations survive; unreported fields stay missing.
/// The operation is idempotent and never recreates expired raw conversations.
pub async fn recover_interrupted(pool: &sqlx::SqlitePool) -> RouterResult<u64> {
    let mut recovered = 0u64;
    loop {
        let mut transaction = pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(database_error)?;
        let rows = sqlx::query_as::<_, StoredReceipt>("SELECT id, received_at, organization_id, account_id, api_key_id, auth_source, endpoint_id, model, finalized, successful, input_tokens, output_tokens, total_tokens, input_source, output_source, total_source FROM usage_receipts WHERE finalized = 0 LIMIT 100")
            .fetch_all(&mut *transaction).await.map_err(database_error)?;
        if rows.is_empty() {
            transaction.commit().await.map_err(database_error)?;
            return Ok(recovered);
        }
        for row in rows {
            let parse_id = |value: &str| {
                uuid::Uuid::parse_str(value)
                    .map_err(|_| LbError::Database("Invalid stored usage receipt identity".into()))
            };
            let attribution = UsageAttribution {
                request_id: parse_id(&row.id)?,
                received_at: chrono::DateTime::parse_from_rfc3339(&row.received_at)
                    .map_err(|_| LbError::Database("Invalid stored usage receipt time".into()))?
                    .with_timezone(&chrono::Utc),
                organization_id: row.organization_id.as_deref().map(parse_id).transpose()?,
                account_id: row.account_id.as_deref().map(parse_id).transpose()?,
                api_key_id: row.api_key_id.as_deref().map(parse_id).transpose()?,
                auth_source: row.auth_source.clone(),
            };
            save_in_transaction(
                &mut transaction,
                &attribution,
                &row.endpoint_id,
                &row.model,
                &row.tokens(),
                true,
                false,
            )
            .await?;
            sqlx::query("UPDATE request_history SET status = 'error', error_message = 'Server restarted before inference completion was committed', completed_at = ? WHERE id = ?")
                .bind(chrono::Utc::now().to_rfc3339()).bind(&row.id).execute(&mut *transaction).await.map_err(database_error)?;
            recovered = recovered.saturating_add(1);
        }
        transaction.commit().await.map_err(database_error)?;
    }
}

#[allow(clippy::too_many_arguments)]
async fn adjust_daily(
    transaction: &mut Transaction<'_, Sqlite>,
    date: &str,
    organization_id: &str,
    account_id: &str,
    endpoint_id: &str,
    model: &str,
    tokens: &UsageTokens,
    finalized: bool,
    successful: bool,
    direction: i64,
) -> RouterResult<()> {
    let (provider, estimated, missing) = tokens.coverage();
    sqlx::query("INSERT INTO usage_daily (date, organization_id, account_id, endpoint_id, model, request_count, successful_requests, failed_requests, input_tokens, output_tokens, total_tokens, provider_count, estimated_count, missing_count) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(date, organization_id, account_id, endpoint_id, model) DO UPDATE SET request_count = request_count + excluded.request_count, successful_requests = successful_requests + excluded.successful_requests, failed_requests = failed_requests + excluded.failed_requests, input_tokens = input_tokens + excluded.input_tokens, output_tokens = output_tokens + excluded.output_tokens, total_tokens = total_tokens + excluded.total_tokens, provider_count = provider_count + excluded.provider_count, estimated_count = estimated_count + excluded.estimated_count, missing_count = missing_count + excluded.missing_count")
        .bind(date)
        .bind(organization_id)
        .bind(account_id)
        .bind(endpoint_id)
        .bind(model)
        .bind(direction)
        .bind(i64::from(finalized && successful) * direction)
        .bind(i64::from(finalized && !successful) * direction)
        .bind(i64::from(tokens.input.value.unwrap_or(0)) * direction)
        .bind(i64::from(tokens.output.value.unwrap_or(0)) * direction)
        .bind(i64::from(tokens.total.value.unwrap_or(0)) * direction)
        .bind(provider * direction)
        .bind(estimated * direction)
        .bind(missing * direction)
        .execute(&mut **transaction)
        .await
        .map_err(database_error)?;
    Ok(())
}
