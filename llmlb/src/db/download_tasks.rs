//! Model Download Tasks Database Operations
//!
//! SPEC-e8e9326e: xLLM Model Download Task Management

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::types::endpoint::ModelDownloadTask;

// Note: DownloadStatus is used in tests
#[cfg(test)]
use crate::types::endpoint::DownloadStatus;

/// Create a new download task
pub async fn create_download_task(
    pool: &SqlitePool,
    task: &ModelDownloadTask,
) -> Result<(), sqlx::Error> {
    let started_at = task.started_at.to_rfc3339();
    let completed_at = task.completed_at.map(|dt| dt.to_rfc3339());

    sqlx::query(
        r#"
        INSERT INTO model_download_tasks (
            id, endpoint_id, model, filename, status, progress,
            speed_mbps, eta_seconds, error_message, started_at, completed_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&task.id)
    .bind(task.endpoint_id.to_string())
    .bind(&task.model)
    .bind(&task.filename)
    .bind(task.status.as_str())
    .bind(task.progress)
    .bind(task.speed_mbps)
    .bind(task.eta_seconds.map(|v| v as i32))
    .bind(&task.error_message)
    .bind(&started_at)
    .bind(&completed_at)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get a download task by ID
pub async fn get_download_task(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<Option<ModelDownloadTask>, sqlx::Error> {
    let row = sqlx::query_as::<_, DownloadTaskRow>(
        r#"
        SELECT id, endpoint_id, model, filename, status, progress,
               speed_mbps, eta_seconds, error_message, started_at, completed_at
        FROM model_download_tasks
        WHERE id = ?
        "#,
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into()))
}

/// List download tasks for an endpoint
pub async fn list_download_tasks(
    pool: &SqlitePool,
    endpoint_id: Uuid,
) -> Result<Vec<ModelDownloadTask>, sqlx::Error> {
    let rows = sqlx::query_as::<_, DownloadTaskRow>(
        r#"
        SELECT id, endpoint_id, model, filename, status, progress,
               speed_mbps, eta_seconds, error_message, started_at, completed_at
        FROM model_download_tasks
        WHERE endpoint_id = ?
        ORDER BY started_at DESC
        "#,
    )
    .bind(endpoint_id.to_string())
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

/// List active (pending/downloading) download tasks for an endpoint
pub async fn list_active_download_tasks(
    pool: &SqlitePool,
    endpoint_id: Uuid,
) -> Result<Vec<ModelDownloadTask>, sqlx::Error> {
    let rows = sqlx::query_as::<_, DownloadTaskRow>(
        r#"
        SELECT id, endpoint_id, model, filename, status, progress,
               speed_mbps, eta_seconds, error_message, started_at, completed_at
        FROM model_download_tasks
        WHERE endpoint_id = ? AND status IN ('pending', 'downloading')
        ORDER BY started_at ASC
        "#,
    )
    .bind(endpoint_id.to_string())
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

/// Update download task progress
pub async fn update_download_progress(
    pool: &SqlitePool,
    task_id: &str,
    progress: f64,
    speed_mbps: Option<f64>,
    eta_seconds: Option<u32>,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE model_download_tasks SET
            status = 'downloading',
            progress = ?,
            speed_mbps = ?,
            eta_seconds = ?
        WHERE id = ?
        "#,
    )
    .bind(progress)
    .bind(speed_mbps)
    .bind(eta_seconds.map(|v| v as i32))
    .bind(task_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Mark download task as completed
pub async fn complete_download_task(
    pool: &SqlitePool,
    task_id: &str,
    filename: Option<&str>,
) -> Result<bool, sqlx::Error> {
    let completed_at = Utc::now().to_rfc3339();

    let result = sqlx::query(
        r#"
        UPDATE model_download_tasks SET
            status = 'completed',
            progress = 100.0,
            filename = COALESCE(?, filename),
            completed_at = ?
        WHERE id = ?
        "#,
    )
    .bind(filename)
    .bind(&completed_at)
    .bind(task_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Mark download task as failed
pub async fn fail_download_task(
    pool: &SqlitePool,
    task_id: &str,
    error_message: &str,
) -> Result<bool, sqlx::Error> {
    let completed_at = Utc::now().to_rfc3339();

    let result = sqlx::query(
        r#"
        UPDATE model_download_tasks SET
            status = 'failed',
            error_message = ?,
            completed_at = ?
        WHERE id = ?
        "#,
    )
    .bind(error_message)
    .bind(&completed_at)
    .bind(task_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Cancel a download task
pub async fn cancel_download_task(pool: &SqlitePool, task_id: &str) -> Result<bool, sqlx::Error> {
    let completed_at = Utc::now().to_rfc3339();

    let result = sqlx::query(
        r#"
        UPDATE model_download_tasks SET
            status = 'cancelled',
            completed_at = ?
        WHERE id = ? AND status IN ('pending', 'downloading')
        "#,
    )
    .bind(&completed_at)
    .bind(task_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Delete a download task
pub async fn delete_download_task(pool: &SqlitePool, task_id: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM model_download_tasks WHERE id = ?")
        .bind(task_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

/// Cleanup old completed/failed/cancelled tasks (older than 7 days)
pub async fn cleanup_old_download_tasks(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let cutoff = (Utc::now() - chrono::Duration::days(7)).to_rfc3339();
    let result = sqlx::query(
        r#"
        DELETE FROM model_download_tasks
        WHERE status IN ('completed', 'failed', 'cancelled')
        AND completed_at < ?
        "#,
    )
    .bind(&cutoff)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

// --- Internal Row Type ---

#[derive(sqlx::FromRow)]
struct DownloadTaskRow {
    id: String,
    endpoint_id: String,
    model: String,
    filename: Option<String>,
    status: String,
    progress: f64,
    speed_mbps: Option<f64>,
    eta_seconds: Option<i32>,
    error_message: Option<String>,
    started_at: String,
    completed_at: Option<String>,
}

impl From<DownloadTaskRow> for ModelDownloadTask {
    fn from(row: DownloadTaskRow) -> Self {
        ModelDownloadTask {
            id: row.id,
            endpoint_id: Uuid::parse_str(&row.endpoint_id).unwrap_or_default(),
            model: row.model,
            filename: row.filename,
            status: row.status.parse().unwrap_or_default(),
            progress: row.progress,
            speed_mbps: row.speed_mbps,
            eta_seconds: row.eta_seconds.map(|v| v as u32),
            error_message: row.error_message,
            started_at: DateTime::parse_from_rfc3339(&row.started_at)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            completed_at: row
                .completed_at
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_test_db() -> SqlitePool {
        crate::db::test_utils::test_db_pool().await
    }

    #[tokio::test]
    async fn test_download_task_crud() {
        let pool = setup_test_db().await;

        // Create endpoint first (needed for foreign key)
        let endpoint = crate::types::endpoint::Endpoint::new(
            "Test Endpoint".to_string(),
            "http://localhost:8080".to_string(),
            crate::types::endpoint::EndpointType::OpenaiCompatible,
        );
        crate::db::endpoints::create_endpoint(&pool, &endpoint)
            .await
            .unwrap();

        // Create download task
        let task = ModelDownloadTask::new(endpoint.id, "llama-3.2-1b".to_string());
        create_download_task(&pool, &task).await.unwrap();

        // Get
        let fetched = get_download_task(&pool, &task.id).await.unwrap().unwrap();
        assert_eq!(fetched.model, "llama-3.2-1b");
        assert_eq!(fetched.status, DownloadStatus::Pending);
        assert_eq!(fetched.progress, 0.0);

        // Update progress
        update_download_progress(&pool, &task.id, 50.0, Some(10.5), Some(30))
            .await
            .unwrap();

        let updated = get_download_task(&pool, &task.id).await.unwrap().unwrap();
        assert_eq!(updated.status, DownloadStatus::Downloading);
        assert_eq!(updated.progress, 50.0);
        assert_eq!(updated.speed_mbps, Some(10.5));

        // Complete
        complete_download_task(&pool, &task.id, Some("model.gguf"))
            .await
            .unwrap();

        let completed = get_download_task(&pool, &task.id).await.unwrap().unwrap();
        assert_eq!(completed.status, DownloadStatus::Completed);
        assert_eq!(completed.progress, 100.0);
        assert_eq!(completed.filename, Some("model.gguf".to_string()));

        // Delete
        delete_download_task(&pool, &task.id).await.unwrap();
        let deleted = get_download_task(&pool, &task.id).await.unwrap();
        assert!(deleted.is_none());
    }

    #[tokio::test]
    async fn test_list_active_tasks() {
        let pool = setup_test_db().await;

        // Create endpoint first
        let endpoint = crate::types::endpoint::Endpoint::new(
            "Test Endpoint".to_string(),
            "http://localhost:8080".to_string(),
            crate::types::endpoint::EndpointType::OpenaiCompatible,
        );
        crate::db::endpoints::create_endpoint(&pool, &endpoint)
            .await
            .unwrap();

        // Create multiple tasks
        let task1 = ModelDownloadTask::new(endpoint.id, "model-1".to_string());
        let task2 = ModelDownloadTask::new(endpoint.id, "model-2".to_string());
        let task3 = ModelDownloadTask::new(endpoint.id, "model-3".to_string());

        create_download_task(&pool, &task1).await.unwrap();
        create_download_task(&pool, &task2).await.unwrap();
        create_download_task(&pool, &task3).await.unwrap();

        // Complete one
        complete_download_task(&pool, &task1.id, None)
            .await
            .unwrap();

        // List active
        let active = list_active_download_tasks(&pool, endpoint.id)
            .await
            .unwrap();
        assert_eq!(active.len(), 2);
    }

    #[tokio::test]
    async fn test_cancel_task() {
        let pool = setup_test_db().await;

        // Create endpoint first
        let endpoint = crate::types::endpoint::Endpoint::new(
            "Test Endpoint".to_string(),
            "http://localhost:8080".to_string(),
            crate::types::endpoint::EndpointType::OpenaiCompatible,
        );
        crate::db::endpoints::create_endpoint(&pool, &endpoint)
            .await
            .unwrap();

        // Create task
        let task = ModelDownloadTask::new(endpoint.id, "model".to_string());
        create_download_task(&pool, &task).await.unwrap();

        // Cancel
        let cancelled = cancel_download_task(&pool, &task.id).await.unwrap();
        assert!(cancelled);

        let fetched = get_download_task(&pool, &task.id).await.unwrap().unwrap();
        assert_eq!(fetched.status, DownloadStatus::Cancelled);
    }

    /// Helper to create an endpoint for tests
    async fn create_test_endpoint(pool: &SqlitePool) -> crate::types::endpoint::Endpoint {
        let endpoint = crate::types::endpoint::Endpoint::new(
            "Test EP".to_string(),
            "http://localhost:9090".to_string(),
            crate::types::endpoint::EndpointType::OpenaiCompatible,
        );
        crate::db::endpoints::create_endpoint(pool, &endpoint)
            .await
            .unwrap();
        endpoint
    }

    #[tokio::test]
    async fn test_get_nonexistent_task() {
        let pool = setup_test_db().await;
        let result = get_download_task(&pool, "no-such-id").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_task() {
        let pool = setup_test_db().await;
        let deleted = delete_download_task(&pool, "no-such-id").await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_fail_download_task() {
        let pool = setup_test_db().await;
        let ep = create_test_endpoint(&pool).await;

        let task = ModelDownloadTask::new(ep.id, "fail-model".to_string());
        create_download_task(&pool, &task).await.unwrap();

        let failed = fail_download_task(&pool, &task.id, "network error")
            .await
            .unwrap();
        assert!(failed);

        let fetched = get_download_task(&pool, &task.id).await.unwrap().unwrap();
        assert_eq!(fetched.status, DownloadStatus::Failed);
        assert_eq!(fetched.error_message, Some("network error".to_string()));
        assert!(fetched.completed_at.is_some());
    }

    #[tokio::test]
    async fn test_cancel_completed_task_no_effect() {
        let pool = setup_test_db().await;
        let ep = create_test_endpoint(&pool).await;

        let task = ModelDownloadTask::new(ep.id, "completed-model".to_string());
        create_download_task(&pool, &task).await.unwrap();

        complete_download_task(&pool, &task.id, None).await.unwrap();

        // Cancelling a completed task should have no effect
        let cancelled = cancel_download_task(&pool, &task.id).await.unwrap();
        assert!(!cancelled);

        let fetched = get_download_task(&pool, &task.id).await.unwrap().unwrap();
        assert_eq!(fetched.status, DownloadStatus::Completed);
    }

    #[tokio::test]
    async fn test_list_download_tasks_for_endpoint() {
        let pool = setup_test_db().await;
        let ep = create_test_endpoint(&pool).await;

        let t1 = ModelDownloadTask::new(ep.id, "m1".to_string());
        let t2 = ModelDownloadTask::new(ep.id, "m2".to_string());
        create_download_task(&pool, &t1).await.unwrap();
        create_download_task(&pool, &t2).await.unwrap();

        let tasks = list_download_tasks(&pool, ep.id).await.unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[tokio::test]
    async fn test_list_download_tasks_empty_for_other_endpoint() {
        let pool = setup_test_db().await;
        let ep = create_test_endpoint(&pool).await;

        let task = ModelDownloadTask::new(ep.id, "m1".to_string());
        create_download_task(&pool, &task).await.unwrap();

        // Different endpoint should have no tasks
        let other_id = Uuid::new_v4();
        let tasks = list_download_tasks(&pool, other_id).await.unwrap();
        assert!(tasks.is_empty());
    }

    #[tokio::test]
    async fn test_update_progress_nonexistent_returns_false() {
        let pool = setup_test_db().await;
        let updated = update_download_progress(&pool, "ghost", 50.0, None, None)
            .await
            .unwrap();
        assert!(!updated);
    }

    #[tokio::test]
    async fn test_complete_with_filename_override() {
        let pool = setup_test_db().await;
        let ep = create_test_endpoint(&pool).await;

        let task = ModelDownloadTask::new(ep.id, "dl-model".to_string());
        create_download_task(&pool, &task).await.unwrap();

        complete_download_task(&pool, &task.id, Some("final-name.gguf"))
            .await
            .unwrap();

        let fetched = get_download_task(&pool, &task.id).await.unwrap().unwrap();
        assert_eq!(fetched.filename, Some("final-name.gguf".to_string()));
        assert_eq!(fetched.progress, 100.0);
    }

    #[tokio::test]
    async fn test_fail_nonexistent_task_returns_false() {
        let pool = setup_test_db().await;
        let result = fail_download_task(&pool, "ghost-task", "error")
            .await
            .unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_cancel_nonexistent_task_returns_false() {
        let pool = setup_test_db().await;
        let result = cancel_download_task(&pool, "ghost-task").await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_complete_nonexistent_task_returns_false() {
        let pool = setup_test_db().await;
        let result = complete_download_task(&pool, "ghost-task", None)
            .await
            .unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_cleanup_old_tasks_removes_old_completed() {
        let pool = setup_test_db().await;
        let ep = create_test_endpoint(&pool).await;

        let task = ModelDownloadTask::new(ep.id, "old-task".to_string());
        create_download_task(&pool, &task).await.unwrap();

        // Manually set completed_at to 10 days ago
        let old_time = (chrono::Utc::now() - chrono::Duration::days(10)).to_rfc3339();
        sqlx::query(
            "UPDATE model_download_tasks SET status = 'completed', completed_at = ? WHERE id = ?",
        )
        .bind(&old_time)
        .bind(&task.id)
        .execute(&pool)
        .await
        .unwrap();

        let cleaned = cleanup_old_download_tasks(&pool).await.unwrap();
        assert_eq!(cleaned, 1);

        let fetched = get_download_task(&pool, &task.id).await.unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn test_cleanup_does_not_remove_active_tasks() {
        let pool = setup_test_db().await;
        let ep = create_test_endpoint(&pool).await;

        let task = ModelDownloadTask::new(ep.id, "active-task".to_string());
        create_download_task(&pool, &task).await.unwrap();

        // Active (pending) tasks should not be cleaned up
        let cleaned = cleanup_old_download_tasks(&pool).await.unwrap();
        assert_eq!(cleaned, 0);

        let fetched = get_download_task(&pool, &task.id).await.unwrap();
        assert!(fetched.is_some());
    }

    #[tokio::test]
    async fn test_download_task_row_conversion() {
        let pool = setup_test_db().await;
        let ep = create_test_endpoint(&pool).await;

        let mut task = ModelDownloadTask::new(ep.id, "conv-model".to_string());
        task.speed_mbps = Some(25.5);
        task.eta_seconds = Some(120);
        create_download_task(&pool, &task).await.unwrap();

        let fetched = get_download_task(&pool, &task.id).await.unwrap().unwrap();
        assert_eq!(fetched.endpoint_id, ep.id);
        assert_eq!(fetched.model, "conv-model");
        assert_eq!(fetched.status, DownloadStatus::Pending);
        assert_eq!(fetched.progress, 0.0);
    }

    #[tokio::test]
    async fn test_cancel_failed_task_no_effect() {
        let pool = setup_test_db().await;
        let ep = create_test_endpoint(&pool).await;

        let task = ModelDownloadTask::new(ep.id, "fail-then-cancel".to_string());
        create_download_task(&pool, &task).await.unwrap();

        fail_download_task(&pool, &task.id, "broken").await.unwrap();

        // Failed tasks cannot be cancelled
        let cancelled = cancel_download_task(&pool, &task.id).await.unwrap();
        assert!(!cancelled);

        let fetched = get_download_task(&pool, &task.id).await.unwrap().unwrap();
        assert_eq!(fetched.status, DownloadStatus::Failed);
    }
}
