//! ログ閲覧API
//!
//! `/v0/dashboard/logs/*` エンドポイントを提供する。

use super::nodes::AppError;
use crate::{logging, AppState};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use llm_router_common::{
    error::{RouterError, RouterResult},
    log::{tail_json_logs, LogEntry},
    types::NodeStatus,
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
    /// ログソース（router / node:NAME）
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

/// GET /v0/dashboard/logs/router
pub async fn get_router_logs(Query(query): Query<LogQuery>) -> Result<Json<LogResponse>, AppError> {
    let log_path = logging::log_file_path().map_err(|err| {
        RouterError::Internal(format!("Failed to resolve router log path: {err}"))
    })?;
    let entries = read_logs(log_path.clone(), clamp_limit(query.limit)).await?;

    Ok(Json(LogResponse {
        source: "router".to_string(),
        entries,
        path: Some(log_path.display().to_string()),
    }))
}

/// GET /v0/nodes/:node_id/logs
pub async fn get_node_logs(
    Path(node_id): Path<Uuid>,
    Query(query): Query<LogQuery>,
    State(state): State<AppState>,
) -> Result<Json<LogResponse>, AppError> {
    let node = state.registry.get(node_id).await?;
    // Pending/Registering 状態でもログ取得は許可（Offline のみ拒否）
    if node.status == NodeStatus::Offline {
        return Err(RouterError::NodeOffline(node_id).into());
    }

    let limit = clamp_limit(query.limit);
    let node_api_port = node.runtime_port.saturating_add(1); // APIポートはLLM runtimeポート+1
    let url = format!(
        "http://{}:{}/v0/logs?tail={}",
        node.ip_address, node_api_port, limit
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|err| RouterError::Internal(err.to_string()))?;

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
        .map_err(|err| RouterError::Internal(err.to_string()))?
        .into();

    Ok(Json(LogResponse {
        source: format!("node:{}", node.machine_name),
        entries: node_logs.entries,
        path: node_logs.path,
    }))
}

fn map_reqwest_error(err: reqwest::Error) -> AppError {
    if err.is_timeout() {
        RouterError::Timeout(err.to_string()).into()
    } else {
        RouterError::Http(err.to_string()).into()
    }
}

async fn read_logs(path: PathBuf, limit: usize) -> RouterResult<Vec<LogEntry>> {
    task::spawn_blocking(move || tail_json_logs(&path, limit))
        .await
        .map_err(|err| RouterError::Internal(format!("Failed to join log reader: {err}")))?
        .map_err(|err| RouterError::Internal(format!("Failed to read logs: {err}")))
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
    use crate::{balancer::LoadManager, db::test_utils::TEST_LOCK, registry::NodeRegistry};
    use axum::extract::State as AxumState;
    use llm_router_common::{protocol::RegisterRequest, types::GpuDeviceInfo};
    use std::{net::IpAddr, sync::Arc};
    use tempfile::tempdir;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn sample_gpu_devices() -> Vec<GpuDeviceInfo> {
        vec![GpuDeviceInfo {
            model: "Test GPU".to_string(),
            count: 1,
            memory: Some(8_000_000_000),
        }]
    }

    async fn router_state() -> AppState {
        let registry = NodeRegistry::new();
        let load_manager = LoadManager::new(registry.clone());
        let request_history = Arc::new(
            crate::db::request_history::RequestHistoryStorage::new().expect("history init"),
        );
        let convert_manager = crate::convert::ConvertTaskManager::new(1);
        let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to create test database");
        sqlx::migrate!("./migrations")
            .run(&db_pool)
            .await
            .expect("Failed to run migrations");
        let jwt_secret = "test-secret".to_string();
        AppState {
            registry,
            load_manager,
            request_history,
            convert_manager,
            db_pool,
            jwt_secret,
            http_client: reqwest::Client::new(),
        }
    }

    #[tokio::test]
    async fn router_logs_endpoint_returns_entries() {
        let _guard = TEST_LOCK.lock().await;
        let temp = tempdir().unwrap();
        std::env::set_var("LLM_ROUTER_DATA_DIR", temp.path());
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
        let response = get_router_logs(Query(LogQuery { limit: 100 }))
            .await
            .unwrap()
            .0;

        assert_eq!(response.source, "router");
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

        std::env::remove_var("LLM_ROUTER_DATA_DIR");
    }

    #[tokio::test]
    async fn node_logs_endpoint_fetches_remote_entries() {
        let _guard = TEST_LOCK.lock().await;
        let mock = MockServer::start().await;
        let node_port = mock.address().port();
        let node_ip: IpAddr = mock.address().ip();

        Mock::given(method("GET"))
            .and(path("/v0/logs"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{"entries":[{"timestamp":"2025-11-14T00:00:00Z","level":"INFO","target":"node","message":"remote","fields":{}}],"path":"/var/log/node.log"}"#,
                "application/json",
            ))
            .mount(&mock)
            .await;

        let state = router_state().await;
        let register_req = RegisterRequest {
            machine_name: "node-1".to_string(),
            ip_address: node_ip,
            runtime_version: "0.1.0".to_string(),
            runtime_port: node_port.saturating_sub(1),
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        };
        let register_res = state.registry.register(register_req).await.unwrap();
        let node_id = register_res.node_id;

        let response = get_node_logs(
            Path(node_id),
            Query(LogQuery { limit: 50 }),
            AxumState(state),
        )
        .await
        .unwrap()
        .0;

        assert_eq!(response.entries.len(), 1);
        assert_eq!(response.entries[0].message.as_deref(), Some("remote"));
        assert_eq!(response.source, "node:node-1");
    }
}
