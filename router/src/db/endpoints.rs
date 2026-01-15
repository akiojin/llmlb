//! エンドポイントデータベース操作
//!
//! SPEC-66555000: ルーター主導エンドポイント登録システム

use crate::types::endpoint::{Endpoint, EndpointHealthCheck, EndpointModel, EndpointStatus};
use sqlx::SqlitePool;
use uuid::Uuid;

/// エンドポイントを登録
pub async fn create_endpoint(pool: &SqlitePool, endpoint: &Endpoint) -> Result<(), sqlx::Error> {
    let id = endpoint.id.to_string();
    let status = endpoint.status.as_str();
    let registered_at = endpoint.registered_at.to_rfc3339();
    let last_seen = endpoint.last_seen.map(|dt| dt.to_rfc3339());

    sqlx::query(
        r#"
        INSERT INTO endpoints (
            id, name, base_url, api_key_encrypted, status,
            health_check_interval_secs, inference_timeout_secs,
            latency_ms, last_seen, last_error, error_count,
            registered_at, notes
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(&endpoint.name)
    .bind(&endpoint.base_url)
    .bind(&endpoint.api_key)
    .bind(status)
    .bind(endpoint.health_check_interval_secs as i32)
    .bind(endpoint.inference_timeout_secs as i32)
    .bind(endpoint.latency_ms.map(|v| v as i32))
    .bind(&last_seen)
    .bind(&endpoint.last_error)
    .bind(endpoint.error_count as i32)
    .bind(&registered_at)
    .bind(&endpoint.notes)
    .execute(pool)
    .await?;

    Ok(())
}

/// エンドポイント一覧を取得
pub async fn list_endpoints(pool: &SqlitePool) -> Result<Vec<Endpoint>, sqlx::Error> {
    let rows = sqlx::query_as::<_, EndpointRow>(
        r#"
        SELECT id, name, base_url, api_key_encrypted, status,
               health_check_interval_secs, inference_timeout_secs,
               latency_ms, last_seen, last_error, error_count,
               registered_at, notes
        FROM endpoints
        ORDER BY registered_at DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

/// IDでエンドポイントを取得
pub async fn get_endpoint(pool: &SqlitePool, id: Uuid) -> Result<Option<Endpoint>, sqlx::Error> {
    let row = sqlx::query_as::<_, EndpointRow>(
        r#"
        SELECT id, name, base_url, api_key_encrypted, status,
               health_check_interval_secs, inference_timeout_secs,
               latency_ms, last_seen, last_error, error_count,
               registered_at, notes
        FROM endpoints
        WHERE id = ?
        "#,
    )
    .bind(id.to_string())
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into()))
}

/// エンドポイントを更新
pub async fn update_endpoint(pool: &SqlitePool, endpoint: &Endpoint) -> Result<bool, sqlx::Error> {
    let id = endpoint.id.to_string();
    let status = endpoint.status.as_str();
    let last_seen = endpoint.last_seen.map(|dt| dt.to_rfc3339());

    let result = sqlx::query(
        r#"
        UPDATE endpoints SET
            name = ?, base_url = ?, api_key_encrypted = ?, status = ?,
            health_check_interval_secs = ?, inference_timeout_secs = ?,
            latency_ms = ?, last_seen = ?, last_error = ?, error_count = ?,
            notes = ?
        WHERE id = ?
        "#,
    )
    .bind(&endpoint.name)
    .bind(&endpoint.base_url)
    .bind(&endpoint.api_key)
    .bind(status)
    .bind(endpoint.health_check_interval_secs as i32)
    .bind(endpoint.inference_timeout_secs as i32)
    .bind(endpoint.latency_ms.map(|v| v as i32))
    .bind(&last_seen)
    .bind(&endpoint.last_error)
    .bind(endpoint.error_count as i32)
    .bind(&endpoint.notes)
    .bind(&id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// エンドポイントを削除
pub async fn delete_endpoint(pool: &SqlitePool, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM endpoints WHERE id = ?")
        .bind(id.to_string())
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

/// ステータスでフィルタしてエンドポイント一覧を取得
pub async fn list_endpoints_by_status(
    pool: &SqlitePool,
    status: EndpointStatus,
) -> Result<Vec<Endpoint>, sqlx::Error> {
    let rows = sqlx::query_as::<_, EndpointRow>(
        r#"
        SELECT id, name, base_url, api_key_encrypted, status,
               health_check_interval_secs, inference_timeout_secs,
               latency_ms, last_seen, last_error, error_count,
               registered_at, notes
        FROM endpoints
        WHERE status = ?
        ORDER BY registered_at DESC
        "#,
    )
    .bind(status.as_str())
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

/// エンドポイントのステータスを更新
pub async fn update_endpoint_status(
    pool: &SqlitePool,
    id: Uuid,
    status: EndpointStatus,
    latency_ms: Option<u32>,
    last_error: Option<&str>,
) -> Result<bool, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let result = sqlx::query(
        r#"
        UPDATE endpoints SET
            status = ?,
            latency_ms = ?,
            last_seen = ?,
            last_error = ?,
            error_count = CASE WHEN ? = 'error' THEN error_count + 1 ELSE 0 END
        WHERE id = ?
        "#,
    )
    .bind(status.as_str())
    .bind(latency_ms.map(|v| v as i32))
    .bind(&now)
    .bind(last_error)
    .bind(status.as_str())
    .bind(id.to_string())
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

// --- EndpointModel CRUD ---

/// エンドポイントにモデルを追加
pub async fn add_endpoint_model(
    pool: &SqlitePool,
    model: &EndpointModel,
) -> Result<(), sqlx::Error> {
    let capabilities_json = model
        .capabilities
        .as_ref()
        .map(|c| serde_json::to_string(c).unwrap_or_default());
    let last_checked = model.last_checked.map(|dt| dt.to_rfc3339());

    sqlx::query(
        r#"
        INSERT OR REPLACE INTO endpoint_models (endpoint_id, model_id, capabilities, last_checked)
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(model.endpoint_id.to_string())
    .bind(&model.model_id)
    .bind(&capabilities_json)
    .bind(&last_checked)
    .execute(pool)
    .await?;

    Ok(())
}

/// エンドポイントのモデル一覧を取得
pub async fn list_endpoint_models(
    pool: &SqlitePool,
    endpoint_id: Uuid,
) -> Result<Vec<EndpointModel>, sqlx::Error> {
    let rows = sqlx::query_as::<_, EndpointModelRow>(
        r#"
        SELECT endpoint_id, model_id, capabilities, last_checked
        FROM endpoint_models
        WHERE endpoint_id = ?
        "#,
    )
    .bind(endpoint_id.to_string())
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

/// エンドポイントからモデルを削除
pub async fn delete_endpoint_model(
    pool: &SqlitePool,
    endpoint_id: Uuid,
    model_id: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        DELETE FROM endpoint_models
        WHERE endpoint_id = ? AND model_id = ?
        "#,
    )
    .bind(endpoint_id.to_string())
    .bind(model_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// エンドポイントの全モデルを削除
pub async fn delete_all_endpoint_models(
    pool: &SqlitePool,
    endpoint_id: Uuid,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM endpoint_models WHERE endpoint_id = ?")
        .bind(endpoint_id.to_string())
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

// --- EndpointHealthCheck CRUD ---

/// ヘルスチェック結果を記録
pub async fn record_health_check(
    pool: &SqlitePool,
    check: &EndpointHealthCheck,
) -> Result<i64, sqlx::Error> {
    let checked_at = check.checked_at.to_rfc3339();

    let result = sqlx::query(
        r#"
        INSERT INTO endpoint_health_checks (
            endpoint_id, checked_at, success, latency_ms,
            error_message, status_before, status_after
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(check.endpoint_id.to_string())
    .bind(&checked_at)
    .bind(check.success)
    .bind(check.latency_ms.map(|v| v as i32))
    .bind(&check.error_message)
    .bind(check.status_before.as_str())
    .bind(check.status_after.as_str())
    .execute(pool)
    .await?;

    Ok(result.last_insert_rowid())
}

/// エンドポイントのヘルスチェック履歴を取得
pub async fn list_health_checks(
    pool: &SqlitePool,
    endpoint_id: Uuid,
    limit: i32,
) -> Result<Vec<EndpointHealthCheck>, sqlx::Error> {
    let rows = sqlx::query_as::<_, EndpointHealthCheckRow>(
        r#"
        SELECT id, endpoint_id, checked_at, success, latency_ms,
               error_message, status_before, status_after
        FROM endpoint_health_checks
        WHERE endpoint_id = ?
        ORDER BY checked_at DESC
        LIMIT ?
        "#,
    )
    .bind(endpoint_id.to_string())
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

/// 古いヘルスチェック履歴を削除（30日以上前）
pub async fn cleanup_old_health_checks(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(30)).to_rfc3339();
    let result = sqlx::query("DELETE FROM endpoint_health_checks WHERE checked_at < ?")
        .bind(&cutoff)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

// --- Internal Row Types ---

#[derive(sqlx::FromRow)]
struct EndpointRow {
    id: String,
    name: String,
    base_url: String,
    api_key_encrypted: Option<String>,
    status: String,
    health_check_interval_secs: i32,
    inference_timeout_secs: i32,
    latency_ms: Option<i32>,
    last_seen: Option<String>,
    last_error: Option<String>,
    error_count: i32,
    registered_at: String,
    notes: Option<String>,
}

impl From<EndpointRow> for Endpoint {
    fn from(row: EndpointRow) -> Self {
        Endpoint {
            id: Uuid::parse_str(&row.id).unwrap_or_default(),
            name: row.name,
            base_url: row.base_url,
            api_key: row.api_key_encrypted,
            status: row.status.parse().unwrap_or_default(),
            health_check_interval_secs: row.health_check_interval_secs as u32,
            inference_timeout_secs: row.inference_timeout_secs as u32,
            latency_ms: row.latency_ms.map(|v| v as u32),
            last_seen: row
                .last_seen
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc)),
            last_error: row.last_error,
            error_count: row.error_count as u32,
            registered_at: chrono::DateTime::parse_from_rfc3339(&row.registered_at)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
            notes: row.notes,
        }
    }
}

#[derive(sqlx::FromRow)]
struct EndpointModelRow {
    endpoint_id: String,
    model_id: String,
    capabilities: Option<String>,
    last_checked: Option<String>,
}

impl From<EndpointModelRow> for EndpointModel {
    fn from(row: EndpointModelRow) -> Self {
        EndpointModel {
            endpoint_id: Uuid::parse_str(&row.endpoint_id).unwrap_or_default(),
            model_id: row.model_id,
            capabilities: row.capabilities.and_then(|s| serde_json::from_str(&s).ok()),
            last_checked: row
                .last_checked
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc)),
        }
    }
}

#[derive(sqlx::FromRow)]
struct EndpointHealthCheckRow {
    id: i64,
    endpoint_id: String,
    checked_at: String,
    success: bool,
    latency_ms: Option<i32>,
    error_message: Option<String>,
    status_before: String,
    status_after: String,
}

impl From<EndpointHealthCheckRow> for EndpointHealthCheck {
    fn from(row: EndpointHealthCheckRow) -> Self {
        EndpointHealthCheck {
            id: row.id,
            endpoint_id: Uuid::parse_str(&row.endpoint_id).unwrap_or_default(),
            checked_at: chrono::DateTime::parse_from_rfc3339(&row.checked_at)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
            success: row.success,
            latency_ms: row.latency_ms.map(|v| v as u32),
            error_message: row.error_message,
            status_before: row.status_before.parse().unwrap_or_default(),
            status_after: row.status_after.parse().unwrap_or_default(),
        }
    }
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
    async fn test_endpoint_crud() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;

        // Create
        let endpoint = Endpoint::new(
            "Test Endpoint".to_string(),
            "http://localhost:8080".to_string(),
        );
        create_endpoint(&pool, &endpoint).await.unwrap();

        // Read
        let fetched = get_endpoint(&pool, endpoint.id).await.unwrap().unwrap();
        assert_eq!(fetched.name, "Test Endpoint");
        assert_eq!(fetched.base_url, "http://localhost:8080");
        assert_eq!(fetched.status, EndpointStatus::Pending);

        // List
        let all = list_endpoints(&pool).await.unwrap();
        assert_eq!(all.len(), 1);

        // Update
        let mut updated = fetched;
        updated.status = EndpointStatus::Online;
        updated.latency_ms = Some(50);
        update_endpoint(&pool, &updated).await.unwrap();

        let fetched_again = get_endpoint(&pool, endpoint.id).await.unwrap().unwrap();
        assert_eq!(fetched_again.status, EndpointStatus::Online);
        assert_eq!(fetched_again.latency_ms, Some(50));

        // Delete
        let deleted = delete_endpoint(&pool, endpoint.id).await.unwrap();
        assert!(deleted);

        let not_found = get_endpoint(&pool, endpoint.id).await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_endpoint_model_crud() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;

        // Create endpoint first
        let endpoint = Endpoint::new(
            "Model Test".to_string(),
            "http://localhost:8081".to_string(),
        );
        create_endpoint(&pool, &endpoint).await.unwrap();

        // Add model
        let model = EndpointModel {
            endpoint_id: endpoint.id,
            model_id: "llama3:8b".to_string(),
            capabilities: Some(vec!["chat".to_string(), "embeddings".to_string()]),
            last_checked: Some(chrono::Utc::now()),
        };
        add_endpoint_model(&pool, &model).await.unwrap();

        // List models
        let models = list_endpoint_models(&pool, endpoint.id).await.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].model_id, "llama3:8b");
        assert_eq!(
            models[0].capabilities,
            Some(vec!["chat".to_string(), "embeddings".to_string()])
        );

        // Delete model
        let deleted = delete_endpoint_model(&pool, endpoint.id, "llama3:8b")
            .await
            .unwrap();
        assert!(deleted);

        let models_after = list_endpoint_models(&pool, endpoint.id).await.unwrap();
        assert!(models_after.is_empty());
    }

    #[tokio::test]
    async fn test_health_check_crud() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;

        // Create endpoint first
        let endpoint = Endpoint::new(
            "Health Test".to_string(),
            "http://localhost:8082".to_string(),
        );
        create_endpoint(&pool, &endpoint).await.unwrap();

        // Record health check
        let check = EndpointHealthCheck {
            id: 0,
            endpoint_id: endpoint.id,
            checked_at: chrono::Utc::now(),
            success: true,
            latency_ms: Some(25),
            error_message: None,
            status_before: EndpointStatus::Pending,
            status_after: EndpointStatus::Online,
        };
        let inserted_id = record_health_check(&pool, &check).await.unwrap();
        assert!(inserted_id > 0);

        // List health checks
        let checks = list_health_checks(&pool, endpoint.id, 10).await.unwrap();
        assert_eq!(checks.len(), 1);
        assert!(checks[0].success);
        assert_eq!(checks[0].latency_ms, Some(25));
    }

    #[tokio::test]
    async fn test_list_endpoints_by_status() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;

        // Create endpoints with different statuses
        let mut ep1 = Endpoint::new("Online EP".to_string(), "http://localhost:8083".to_string());
        ep1.status = EndpointStatus::Online;
        create_endpoint(&pool, &ep1).await.unwrap();

        let mut ep2 = Endpoint::new(
            "Offline EP".to_string(),
            "http://localhost:8084".to_string(),
        );
        ep2.status = EndpointStatus::Offline;
        create_endpoint(&pool, &ep2).await.unwrap();

        let ep3 = Endpoint::new(
            "Pending EP".to_string(),
            "http://localhost:8085".to_string(),
        );
        create_endpoint(&pool, &ep3).await.unwrap();

        // Filter by status
        let online = list_endpoints_by_status(&pool, EndpointStatus::Online)
            .await
            .unwrap();
        assert_eq!(online.len(), 1);
        assert_eq!(online[0].name, "Online EP");

        let pending = list_endpoints_by_status(&pool, EndpointStatus::Pending)
            .await
            .unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].name, "Pending EP");
    }
}
