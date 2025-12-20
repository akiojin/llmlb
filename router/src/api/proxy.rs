//! LLM runtimeプロキシ APIハンドラー

use crate::AppState;
use axum::{
    body::Body,
    http::{HeaderName, HeaderValue, StatusCode},
    response::Response,
};
use futures::TryStreamExt;
use llm_router_common::{error::RouterError, protocol::RequestResponseRecord};
use std::{io, sync::Arc};

pub(crate) async fn select_available_node(
    state: &AppState,
) -> Result<llm_router_common::types::Node, RouterError> {
    let mode = std::env::var("LOAD_BALANCER_MODE").unwrap_or_else(|_| "auto".to_string());

    match mode.as_str() {
        "metrics" => {
            // メトリクスベース選択（T014-T015で実装）
            state.load_manager.select_node_by_metrics().await
        }
        _ => {
            // デフォルト: 既存の高度なロードバランシング
            let node = state.load_manager.select_node().await?;
            if node.initializing {
                return Err(RouterError::ServiceUnavailable(
                    "All nodes are warming up models".into(),
                ));
            }
            Ok(node)
        }
    }
}

pub(crate) fn forward_streaming_response(
    response: reqwest::Response,
) -> Result<Response, RouterError> {
    let status = response.status();
    let headers = response.headers().clone();
    let stream = response.bytes_stream().map_err(io::Error::other);
    let body = Body::from_stream(stream);
    let mut axum_response = Response::new(body);
    *axum_response.status_mut() = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK);
    {
        let response_headers = axum_response.headers_mut();
        for (name, value) in headers.iter() {
            if let (Ok(header_name), Ok(header_value)) = (
                HeaderName::from_bytes(name.as_str().as_bytes()),
                HeaderValue::from_bytes(value.as_bytes()),
            ) {
                response_headers.insert(header_name, header_value);
            }
        }
    }
    use axum::http::header;
    if !axum_response
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap_or("").starts_with("text/event-stream"))
        .unwrap_or(false)
    {
        axum_response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
    }
    Ok(axum_response)
}

/// リクエスト/レスポンスレコードを保存（Fire-and-forget）
pub(crate) fn save_request_record(
    storage: Arc<crate::db::request_history::RequestHistoryStorage>,
    record: RequestResponseRecord,
) {
    tokio::spawn(async move {
        if let Err(e) = storage.save_record(&record).await {
            tracing::error!("Failed to save request record: {}", e);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        balancer::{LoadManager, MetricsUpdate},
        registry::NodeRegistry,
    };
    use llm_router_common::{protocol::RegisterRequest, types::GpuDeviceInfo};
    use std::net::IpAddr;
    use uuid::Uuid;

    async fn create_test_state() -> AppState {
        let registry = NodeRegistry::new();
        let load_manager = LoadManager::new(registry.clone());
        let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to create test database");
        sqlx::migrate!("./migrations")
            .run(&db_pool)
            .await
            .expect("Failed to run migrations");
        let request_history = std::sync::Arc::new(
            crate::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
        );
        let convert_manager = crate::convert::ConvertTaskManager::new(1, db_pool.clone());
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

    async fn mark_ready(state: &AppState, node_id: Uuid) {
        // レジストリ側のフラグも更新し、ロードバランサが初期化完了と判断できるようにする
        state
            .registry
            .update_last_seen(
                node_id,
                None,
                None,
                None,
                None,
                None,
                Some(false),
                Some((4, 4)),
            )
            .await
            .ok();

        state
            .load_manager
            .record_metrics(MetricsUpdate {
                node_id,
                cpu_usage: 0.0,
                memory_usage: 0.0,
                gpu_usage: None,
                gpu_memory_usage: None,
                gpu_memory_total_mb: None,
                gpu_memory_used_mb: None,
                gpu_temperature: None,
                gpu_model_name: None,
                gpu_compute_capability: None,
                gpu_capability_score: None,
                active_requests: 0,
                average_response_time_ms: Some(1.0),
                initializing: false,
                ready_models: Some((4, 4)),
            })
            .await
            .ok();
    }

    #[tokio::test]
    async fn test_select_available_node_no_nodes() {
        let state = create_test_state().await;
        let result = select_available_node(&state).await;
        assert!(matches!(result, Err(RouterError::NoNodesAvailable)));
    }

    #[tokio::test]
    async fn test_select_available_node_success() {
        let state = create_test_state().await;

        // ノードを登録
        let register_req = RegisterRequest {
            machine_name: "test-machine".to_string(),
            ip_address: "192.168.1.100".parse::<IpAddr>().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 11434,
            gpu_available: true,
            gpu_devices: vec![GpuDeviceInfo {
                model: "Test GPU".to_string(),
                count: 1,
                memory: None,
            }],
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        };
        let response = state.registry.register(register_req).await.unwrap();
        state.registry.approve(response.node_id).await.unwrap();

        // mark as ready so load balancer can pick
        mark_ready(&state, response.node_id).await;

        let result = select_available_node(&state).await;
        assert!(result.is_ok());

        let node = result.unwrap();
        assert_eq!(node.machine_name, "test-machine");
    }

    #[tokio::test]
    async fn test_select_available_node_skips_offline() {
        let state = create_test_state().await;

        // ノード1を登録
        let register_req1 = RegisterRequest {
            machine_name: "machine1".to_string(),
            ip_address: "192.168.1.100".parse().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 11434,
            gpu_available: true,
            gpu_devices: vec![GpuDeviceInfo {
                model: "Test GPU".to_string(),
                count: 1,
                memory: None,
            }],
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        };
        let response1 = state.registry.register(register_req1).await.unwrap();
        state.registry.approve(response1.node_id).await.unwrap();

        // ノード1をオフラインにする
        state
            .registry
            .mark_offline(response1.node_id)
            .await
            .unwrap();

        // ノード2を登録
        let register_req2 = RegisterRequest {
            machine_name: "machine2".to_string(),
            ip_address: "192.168.1.101".parse().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 11434,
            gpu_available: true,
            gpu_devices: vec![GpuDeviceInfo {
                model: "Test GPU".to_string(),
                count: 1,
                memory: None,
            }],
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        };
        let response2 = state.registry.register(register_req2).await.unwrap();
        state.registry.approve(response2.node_id).await.unwrap();

        // mark second node ready
        mark_ready(&state, response2.node_id).await;

        let result = select_available_node(&state).await;
        assert!(result.is_ok());

        let node = result.unwrap();
        assert_eq!(node.machine_name, "machine2");
    }
}
