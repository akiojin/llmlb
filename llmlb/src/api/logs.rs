//! ログ閲覧API
//!
//! `/api/dashboard/logs/*` エンドポイントを提供する。

use super::error::AppError;
use crate::common::{
    error::{LbError, RouterResult},
    log::{tail_json_logs, LogEntry},
};
use crate::{logging, AppState};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};
use tokio::task;
use uuid::Uuid;

const DEFAULT_LIMIT: usize = 200;
const MAX_LIMIT: usize = 1000;

/// ログ取得クエリパラメーター
#[derive(Debug, Clone, Deserialize)]
pub struct LogQuery {
    /// 取得件数（1-1000）
    #[serde(default = "default_limit")]
    pub limit: usize,
}

/// ログレスポンス
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LogResponse {
    /// ログソース（load balancer / node:NAME）
    pub source: String,
    /// ログエントリ一覧
    pub entries: Vec<LogEntry>,
    /// ログファイルパス
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

fn default_limit() -> usize {
    DEFAULT_LIMIT
}

fn clamp_limit(limit: usize) -> usize {
    limit.clamp(1, MAX_LIMIT)
}

/// GET /api/dashboard/logs/load balancer
pub async fn get_lb_logs(Query(query): Query<LogQuery>) -> Result<Json<LogResponse>, AppError> {
    let log_path = logging::log_file_path().map_err(|err| {
        LbError::Internal(format!("Failed to resolve load balancer log path: {err}"))
    })?;
    let entries = read_logs(log_path.clone(), clamp_limit(query.limit)).await?;

    Ok(Json(LogResponse {
        source: "load balancer".to_string(),
        entries,
        path: Some(log_path.display().to_string()),
    }))
}

/// GET /api/nodes/:node_id/logs
///
/// # 廃止予定
///
/// このAPIは廃止予定です。ノードベースのログ取得はエンドポイントベースに移行されます。
/// エンドポイントが `/api/logs` を提供している場合、llmlbはそこにリクエストを転送します。
#[deprecated(note = "Use endpoint-based log fetching instead. Node-based routing is deprecated.")]
#[allow(deprecated)] // NodeRegistry migration in progress
pub async fn get_node_logs(
    Path(endpoint_id): Path<Uuid>,
    Query(query): Query<LogQuery>,
    State(state): State<AppState>,
) -> Result<Json<LogResponse>, AppError> {
    use crate::types::endpoint::EndpointStatus;

    let endpoint = state
        .endpoint_registry
        .get(endpoint_id)
        .await
        .ok_or(LbError::NodeNotFound(endpoint_id))?;

    // Pending/Error 状態でもログ取得は許可（Offline のみ拒否）
    if endpoint.status == EndpointStatus::Offline {
        return Err(LbError::NodeOffline(endpoint_id).into());
    }

    let limit = clamp_limit(query.limit);
    // エンドポイントのbase_urlからログ取得
    let url = format!(
        "{}/api/logs?tail={}",
        endpoint.base_url.trim_end_matches('/'),
        limit
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|err| LbError::Internal(err.to_string()))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(map_reqwest_error)?
        .error_for_status()
        .map_err(map_reqwest_error)?;

    let node_logs: LogResponse = response
        .json::<NodeLogPayload>()
        .await
        .map_err(|err| LbError::Internal(err.to_string()))?
        .into();

    Ok(Json(LogResponse {
        source: format!("endpoint:{}", endpoint.name),
        entries: node_logs.entries,
        path: node_logs.path,
    }))
}

fn map_reqwest_error(err: reqwest::Error) -> AppError {
    if err.is_timeout() {
        LbError::Timeout(err.to_string()).into()
    } else {
        LbError::Http(err.to_string()).into()
    }
}

async fn read_logs(path: PathBuf, limit: usize) -> RouterResult<Vec<LogEntry>> {
    task::spawn_blocking(move || tail_json_logs(&path, limit))
        .await
        .map_err(|err| LbError::Internal(format!("Failed to join log reader: {err}")))?
        .map_err(|err| LbError::Internal(format!("Failed to read logs: {err}")))
}

#[derive(Debug, Deserialize)]
struct NodeLogPayload {
    entries: Vec<LogEntry>,
    path: Option<String>,
}

impl From<NodeLogPayload> for LogResponse {
    fn from(value: NodeLogPayload) -> Self {
        Self {
            source: "node".to_string(),
            entries: value.entries,
            path: value.path,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{balancer::LoadManager, db::test_utils::TEST_LOCK};
    use axum::extract::State as AxumState;
    use std::sync::Arc;
    use tempfile::tempdir;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn lb_state() -> AppState {
        let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to create test database");
        sqlx::migrate!("./migrations")
            .run(&db_pool)
            .await
            .expect("Failed to run migrations");
        let request_history = Arc::new(crate::db::request_history::RequestHistoryStorage::new(
            db_pool.clone(),
        ));
        let endpoint_registry = crate::registry::endpoints::EndpointRegistry::new(db_pool.clone())
            .await
            .expect("Failed to create endpoint registry");
        let endpoint_registry_arc = Arc::new(endpoint_registry.clone());
        let load_manager = LoadManager::new(endpoint_registry_arc);
        let jwt_secret = "test-secret".to_string();
        AppState {
            load_manager,
            request_history,
            db_pool,
            jwt_secret,
            http_client: reqwest::Client::new(),
            queue_config: crate::config::QueueConfig::from_env(),
            event_bus: crate::events::create_shared_event_bus(),
            endpoint_registry,
        }
    }

    #[tokio::test]
    async fn lb_logs_endpoint_returns_entries() {
        let _guard = TEST_LOCK.lock().await;
        let temp = tempdir().unwrap();
        std::env::set_var("LLMLB_DATA_DIR", temp.path());
        let log_path = logging::log_file_path().unwrap();
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        // 既存のログファイルを削除してクリーンな状態から開始
        let _ = std::fs::remove_file(&log_path);
        std::fs::write(
            &log_path,
            "{\"timestamp\":\"2025-11-14T00:00:00Z\",\"level\":\"INFO\",\"target\":\"test\",\"fields\":{\"message\":\"hello\"}}\n{\"timestamp\":\"2025-11-14T00:01:00Z\",\"level\":\"ERROR\",\"target\":\"test\",\"fields\":{\"message\":\"world\"}}\n",
        )
        .unwrap();

        // limitを十分大きく設定し、バックグラウンドプロセスによるログ追加を考慮
        let response = get_lb_logs(Query(LogQuery { limit: 100 })).await.unwrap().0;

        assert_eq!(response.source, "load balancer");
        // インデックスベースの検証ではなく、特定のメッセージが存在するかどうかを確認
        // （バックグラウンドプロセスがログに追加すると、インデックスがずれる可能性があるため）
        let has_hello = response
            .entries
            .iter()
            .any(|e| e.message.as_deref() == Some("hello"));
        let has_world = response
            .entries
            .iter()
            .any(|e| e.message.as_deref() == Some("world"));
        assert!(has_hello, "Expected 'hello' message in log entries");
        assert!(has_world, "Expected 'world' message in log entries");

        std::env::remove_var("LLMLB_DATA_DIR");
    }

    #[tokio::test]
    #[allow(deprecated)] // get_node_logs is deprecated
    async fn node_logs_endpoint_fetches_remote_entries() {
        use crate::types::endpoint::{Endpoint, EndpointStatus};

        let _guard = TEST_LOCK.lock().await;
        let mock = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/logs"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{"entries":[{"timestamp":"2025-11-14T00:00:00Z","level":"INFO","target":"node","message":"remote","fields":{}}],"path":"/var/log/node.log"}"#,
                "application/json",
            ))
            .mount(&mock)
            .await;

        let state = lb_state().await;

        // EndpointRegistryにエンドポイントを追加
        let mut endpoint = Endpoint::new("endpoint-1".to_string(), mock.uri());
        endpoint.status = EndpointStatus::Online;
        endpoint.gpu_device_count = Some(1);
        endpoint.gpu_total_memory_bytes = Some(8_000_000_000);
        let endpoint_id = endpoint.id;
        state
            .endpoint_registry
            .add(endpoint)
            .await
            .expect("Failed to add endpoint");

        let response = get_node_logs(
            Path(endpoint_id),
            Query(LogQuery { limit: 50 }),
            AxumState(state),
        )
        .await
        .unwrap()
        .0;

        assert_eq!(response.entries.len(), 1);
        assert_eq!(response.entries[0].message.as_deref(), Some("remote"));
        assert_eq!(response.source, "endpoint:endpoint-1");
    }
}
