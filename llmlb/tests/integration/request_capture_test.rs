//! プロキシキャプチャの Integration Tests
//!
//! T011-T013: proxy.rs のキャプチャ機能をテスト

use axum::{
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use llmlb::common::protocol::{RecordStatus, RequestResponseRecord, RequestType};
use llmlb::db::request_history::RequestHistoryStorage;
use reqwest::Client;
use serde_json::{json, Value};
use serial_test::serial;
use sqlx::SqlitePool;
use std::net::SocketAddr;
use std::time::Duration;

use crate::support::{
    http::{spawn_lb, TestServer},
    lb::{register_endpoint_with_capabilities, spawn_test_lb_with_db},
};

async fn spawn_mock_openai_success() -> TestServer {
    async fn v1_models() -> impl axum::response::IntoResponse {
        Json(json!({
            "object": "list",
            "data": [{"id": "mock-model"}]
        }))
    }

    async fn chat_completion(Json(_payload): Json<Value>) -> impl axum::response::IntoResponse {
        Json(json!({
            "id": "chatcmpl-1",
            "object": "chat.completion",
            "model": "mock-model",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "hello"},
                "finish_reason": "stop"
            }]
        }))
    }

    async fn completions(Json(_payload): Json<Value>) -> impl axum::response::IntoResponse {
        Json(json!({
            "id": "cmpl-1",
            "object": "text_completion",
            "model": "mock-model",
            "choices": [{
                "index": 0,
                "text": "hello",
                "finish_reason": "stop"
            }]
        }))
    }

    let app = Router::new()
        .route("/v1/models", get(v1_models))
        .route("/v1/chat/completions", post(chat_completion))
        .route("/v1/completions", post(completions));

    spawn_lb(app).await
}

async fn spawn_mock_openai_error() -> TestServer {
    async fn v1_models() -> impl axum::response::IntoResponse {
        Json(json!({
            "object": "list",
            "data": [{"id": "mock-model"}]
        }))
    }

    async fn chat_completion(Json(_payload): Json<Value>) -> impl axum::response::IntoResponse {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "upstream error"})),
        )
    }

    async fn completions(Json(_payload): Json<Value>) -> impl axum::response::IntoResponse {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "upstream error"})),
        )
    }

    let app = Router::new()
        .route("/v1/models", get(v1_models))
        .route("/v1/chat/completions", post(chat_completion))
        .route("/v1/completions", post(completions));

    spawn_lb(app).await
}

async fn load_history(db_pool: &SqlitePool) -> Vec<RequestResponseRecord> {
    let storage = RequestHistoryStorage::new(db_pool.clone());
    storage.load_records().await.unwrap_or_default()
}

async fn sync_endpoint(lb_addr: SocketAddr, endpoint_id: &str) {
    let client = Client::new();
    let response = client
        .post(format!(
            "http://{}/api/endpoints/{}/sync",
            lb_addr, endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("sync endpoint");

    assert_eq!(response.status(), reqwest::StatusCode::OK);
}

/// T011: プロキシキャプチャ機能の integration test
#[tokio::test]
#[serial]
async fn test_chat_request_is_captured() {
    let mock = spawn_mock_openai_success().await;
    let (lb, db_pool) = spawn_test_lb_with_db().await;

    let endpoint_id = register_endpoint_with_capabilities(
        lb.addr(),
        mock.addr(),
        "Chat Endpoint",
        &["chat_completion"],
    )
    .await
    .expect("register endpoint");
    sync_endpoint(lb.addr(), &endpoint_id).await;

    let client = Client::new();
    let payload = json!({
        "model": "mock-model",
        "messages": [{"role": "user", "content": "hello"}]
    });

    let response = client
        .post(format!("http://{}/v1/chat/completions", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&payload)
        .send()
        .await
        .expect("send chat request");

    assert_eq!(response.status(), reqwest::StatusCode::OK);

    tokio::time::sleep(Duration::from_millis(100)).await;

    let records = load_history(&db_pool).await;
    assert!(records.iter().any(|r| {
        r.model == "mock-model"
            && r.request_type == RequestType::Chat
            && matches!(&r.status, RecordStatus::Success)
    }));
}

/// T011: Generate リクエストのキャプチャ
#[tokio::test]
#[serial]
async fn test_generate_request_is_captured() {
    let mock = spawn_mock_openai_success().await;
    let (lb, db_pool) = spawn_test_lb_with_db().await;

    let endpoint_id = register_endpoint_with_capabilities(
        lb.addr(),
        mock.addr(),
        "Generate Endpoint",
        &["chat_completion"],
    )
    .await
    .expect("register endpoint");
    sync_endpoint(lb.addr(), &endpoint_id).await;

    let client = Client::new();
    let payload = json!({
        "model": "mock-model",
        "prompt": "hello"
    });

    let response = client
        .post(format!("http://{}/v1/completions", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&payload)
        .send()
        .await
        .expect("send completion request");

    assert_eq!(response.status(), reqwest::StatusCode::OK);

    tokio::time::sleep(Duration::from_millis(100)).await;

    let records = load_history(&db_pool).await;
    assert!(records.iter().any(|r| {
        r.model == "mock-model"
            && r.request_type == RequestType::Generate
            && matches!(&r.status, RecordStatus::Success)
    }));
}

/// T011: レコード内容の検証
#[tokio::test]
#[serial]
async fn test_captured_record_contents() {
    let mock = spawn_mock_openai_success().await;
    let (lb, db_pool) = spawn_test_lb_with_db().await;

    let endpoint_id = register_endpoint_with_capabilities(
        lb.addr(),
        mock.addr(),
        "Content Endpoint",
        &["chat_completion"],
    )
    .await
    .expect("register endpoint");
    sync_endpoint(lb.addr(), &endpoint_id).await;

    let client = Client::new();
    let payload = json!({
        "model": "mock-model",
        "messages": [{"role": "user", "content": "content-check"}]
    });

    let response = client
        .post(format!("http://{}/v1/chat/completions", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&payload)
        .send()
        .await
        .expect("send chat request");

    assert_eq!(response.status(), reqwest::StatusCode::OK);

    tokio::time::sleep(Duration::from_millis(100)).await;

    let records = load_history(&db_pool).await;
    let record = records
        .iter()
        .find(|r| r.model == "mock-model" && r.request_type == RequestType::Chat)
        .expect("record should exist");

    assert_eq!(record.model, "mock-model");
    assert_eq!(record.node_id.to_string(), endpoint_id);
    assert!(record.completed_at >= record.timestamp);
    assert!(record.response_body.is_some());
    assert!(record.request_body.get("messages").is_some());
}

/// T012: エラーリクエストのキャプチャ integration test
#[tokio::test]
#[serial]
async fn test_error_request_is_captured() {
    let mock = spawn_mock_openai_error().await;
    let (lb, db_pool) = spawn_test_lb_with_db().await;

    let endpoint_id = register_endpoint_with_capabilities(
        lb.addr(),
        mock.addr(),
        "Error Endpoint",
        &["chat_completion"],
    )
    .await
    .expect("register endpoint");
    sync_endpoint(lb.addr(), &endpoint_id).await;

    let client = Client::new();
    let payload = json!({
        "model": "mock-model",
        "messages": [{"role": "user", "content": "error"}]
    });

    let response = client
        .post(format!("http://{}/v1/chat/completions", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&payload)
        .send()
        .await
        .expect("send error request");

    assert_eq!(response.status(), reqwest::StatusCode::BAD_GATEWAY);

    tokio::time::sleep(Duration::from_millis(100)).await;

    let records = load_history(&db_pool).await;
    let record = records
        .iter()
        .find(|r| r.model == "mock-model" && matches!(&r.status, RecordStatus::Error { .. }))
        .expect("error record should exist");
    assert!(record.response_body.is_none());
}

/// T012: ノード接続失敗のキャプチャ
#[tokio::test]
#[serial]
async fn test_node_connection_failure_capture() {
    let mock = spawn_mock_openai_success().await;
    let (lb, db_pool) = spawn_test_lb_with_db().await;

    let endpoint_id = register_endpoint_with_capabilities(
        lb.addr(),
        mock.addr(),
        "Offline Endpoint",
        &["chat_completion"],
    )
    .await
    .expect("register endpoint");
    sync_endpoint(lb.addr(), &endpoint_id).await;

    // モックサーバーを停止して接続失敗を再現
    mock.stop().await;

    let client = Client::new();
    let payload = json!({
        "model": "mock-model",
        "messages": [{"role": "user", "content": "fail"}]
    });

    let response = client
        .post(format!("http://{}/v1/chat/completions", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&payload)
        .send()
        .await
        .expect("send request");

    assert_eq!(response.status(), reqwest::StatusCode::BAD_GATEWAY);

    tokio::time::sleep(Duration::from_millis(100)).await;

    let records = load_history(&db_pool).await;
    let record = records
        .iter()
        .find(|r| r.model == "mock-model" && matches!(&r.status, RecordStatus::Error { .. }))
        .expect("connection failure record should exist");
    assert!(record.response_body.is_none());
}

/// T013: ストリーミングレスポンスのキャプチャ integration test
#[tokio::test]
#[ignore = "streaming capture test harness not implemented yet"]
async fn test_streaming_response_capture() {}

/// T013: ストリーミングエラーのキャプチャ
#[tokio::test]
#[ignore = "streaming error capture test harness not implemented yet"]
async fn test_streaming_error_capture() {}

/// T011-T013: プロキシのパフォーマンスへの影響テスト
#[tokio::test]
#[ignore = "performance benchmark not implemented yet"]
async fn test_capture_performance_impact() {}
