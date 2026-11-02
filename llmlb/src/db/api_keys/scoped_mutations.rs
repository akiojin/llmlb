//! Mutations intersect the current authorization scope with the original issuer.

use super::{map_write_error, ApiKeyMetadata, ApiKeyMetadataRow};
use crate::auth::scope::Scope;
use crate::common::error::LbError;
use chrono::{DateTime, Utc};
use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use uuid::Uuid;

const RETURNING_METADATA: &str = " RETURNING id,key_prefix,name,created_by,created_at,expires_at,permissions,usage_account_id,organization_id";

fn require_assigned(scope: &Scope) -> Result<(), LbError> {
    if matches!(scope, Scope::Unassigned(_)) {
        return Err(LbError::Authorization(
            "An active organization membership is required".to_string(),
        ));
    }
    Ok(())
}

fn restrict_mutation(
    query: &mut QueryBuilder<'_, Sqlite>,
    id: Uuid,
    created_by: Uuid,
    scope: &Scope,
) {
    query
        .push(" WHERE id = ")
        .push_bind(id.to_string())
        .push(" AND created_by = ")
        .push_bind(created_by.to_string())
        .push(" AND (");
    scope.push_sql(query, "organization_id", "usage_account_id");
    query.push(")");
}

/// Update only a key both visible in the current scope and issued by this actor.
/// Returned metadata is selected atomically by the mutation, without key hashes.
pub async fn update_scoped_by_creator(
    pool: &SqlitePool,
    id: Uuid,
    created_by: Uuid,
    scope: &Scope,
    name: &str,
    expires_at: Option<DateTime<Utc>>,
) -> Result<Option<ApiKeyMetadata>, LbError> {
    require_assigned(scope)?;
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| map_write_error(error, name, "begin scoped API key update"))?;
    let expiry = expires_at.map(|time| time.to_rfc3339());

    // Old databases can contain duplicate names and an older name-update trigger.
    // Do not touch the name column when only expiry changes. Serialize both paths.
    let mut expiry_update = QueryBuilder::new("UPDATE api_keys SET expires_at = ");
    expiry_update.push_bind(expiry.clone());
    restrict_mutation(&mut expiry_update, id, created_by, scope);
    expiry_update.push(" AND name = ").push_bind(name);
    expiry_update.push(RETURNING_METADATA);
    let same_name = expiry_update
        .build_query_as::<ApiKeyMetadataRow>()
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| map_write_error(error, name, "update scoped API key expiry"))?;
    if let Some(row) = same_name {
        let metadata = row.into_metadata()?;
        transaction
            .commit()
            .await
            .map_err(|error| map_write_error(error, name, "commit scoped API key update"))?;
        return Ok(Some(metadata));
    }

    let mut rename = QueryBuilder::new("UPDATE api_keys SET name = ");
    rename
        .push_bind(name)
        .push(", expires_at = ")
        .push_bind(expiry);
    restrict_mutation(&mut rename, id, created_by, scope);
    rename.push(" AND name <> ").push_bind(name);
    rename.push(RETURNING_METADATA);
    let renamed = rename
        .build_query_as::<ApiKeyMetadataRow>()
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| map_write_error(error, name, "update scoped API key name"))?
        .map(ApiKeyMetadataRow::into_metadata)
        .transpose()?;
    transaction
        .commit()
        .await
        .map_err(|error| map_write_error(error, name, "commit scoped API key update"))?;
    Ok(renamed)
}

/// Delete only a key both visible in the current scope and issued by this actor.
/// Hidden IDs and nonexistent IDs both affect zero rows and disclose no metadata.
pub async fn delete_scoped_by_creator(
    pool: &SqlitePool,
    id: Uuid,
    created_by: Uuid,
    scope: &Scope,
) -> Result<bool, LbError> {
    require_assigned(scope)?;
    let mut query = QueryBuilder::new("DELETE FROM api_keys");
    restrict_mutation(&mut query, id, created_by, scope);
    let deleted = query.build().execute(pool).await.map_err(|error| {
        LbError::Database(format!(
            "Failed to delete scoped API key by creator: {error}"
        ))
    })?;
    Ok(deleted.rows_affected() > 0)
}
