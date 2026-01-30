//! Integration Test: T024 - /api/system API統合テスト
//!
//! SPEC-f8e3a1b7: エンドポイント登録時の/api/system API呼び出し
//!
//! xLLMエンドポイントからデバイス情報を取得し、Endpoint.device_infoに保存する。
//! 非対応エンドポイント（Ollama、vLLM等）では無視される。

use axum::{http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::sleep;

use crate::support::http::spawn_lb;
use crate::support::lb::spawn_test_lb;

/// xLLM互換のモックエンドポイントを起動（/api/system対応）
async fn spawn_xllm_mock() -> crate::support::http::TestServer {
    let app = Router::new()
        .route("/api/system", get(system_handler))
        .route("/v1/models", get(models_handler))
        .route("/health", get(health_handler));
    spawn_lb(app).await
}

/// /api/system レスポンス（xLLM互換）
async fn system_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "device": {
                "device_type": "gpu",
                "gpu_devices": [
                    {
                        "name": "Apple M1 Max",
                        "total_memory_bytes": 34359738368_u64,
                        "used_memory_bytes": 8589934592_u64
                    }
                ]
            }
        })),
    )
}

/// /v1/models レスポンス
async fn models_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "object": "list",
            "data": [
                { "id": "llama-3.1-8b", "object": "model" }
            ]
        })),
    )
}

/// /health レスポンス
async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"status": "ok"})))
}

/// T024-1: xLLMエンドポイント登録時に/api/systemからデバイス情報を取得
#[tokio::test]
async fn test_v0_system_device_info_retrieved_on_registration() {
    let lb = spawn_test_lb().await;
    let mock_xllm = spawn_xllm_mock().await;
    let client = Client::new();

    // エンドポイント登録
    let response = client
        .post(format!("http://{}/api/endpoints", lb.addr()))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "xLLM Test",
            "base_url": format!("http://{}", mock_xllm.addr())
        }))
        .send()
        .await
        .expect("registration request failed");

    assert_eq!(response.status().as_u16(), 201);
    let body: Value = response.json().await.unwrap();
    let endpoint_id = body["id"].as_str().unwrap();

    // /api/systemは非同期で呼ばれるため、少し待つ
    sleep(Duration::from_millis(500)).await;

    // エンドポイント詳細を取得してdevice_infoを確認
    let detail_response = client
        .get(format!(
            "http://{}/api/endpoints/{}",
            lb.addr(),
            endpoint_id
        ))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("get endpoint request failed");

    assert_eq!(detail_response.status().as_u16(), 200);
    let detail: Value = detail_response.json().await.unwrap();

    // device_infoが取得されていることを確認
    assert!(
        detail.get("device_info").is_some() && !detail["device_info"].is_null(),
        "device_info should be retrieved from /api/system: {:?}",
        detail
    );

    let device_info = &detail["device_info"];
    assert_eq!(
        device_info["device_type"], "gpu",
        "device_type should be 'gpu'"
    );
    assert!(
        device_info["gpu_devices"].is_array(),
        "gpu_devices should be an array"
    );

    let gpu_devices = device_info["gpu_devices"].as_array().unwrap();
    assert_eq!(gpu_devices.len(), 1, "should have 1 GPU device");
    assert_eq!(gpu_devices[0]["name"], "Apple M1 Max");

    mock_xllm.stop().await;
}

/// /api/system非対応エンドポイント（404を返す）
async fn spawn_non_xllm_mock() -> crate::support::http::TestServer {
    let app = Router::new()
        .route("/v1/models", get(models_handler))
        .route("/health", get(health_handler));
    // /api/systemルートなし - 404を返す
    spawn_lb(app).await
}

/// T024-2: /api/system非対応エンドポイント（Ollama等）では無視される
#[tokio::test]
async fn test_v0_system_ignored_for_unsupported_endpoint() {
    let lb = spawn_test_lb().await;
    let mock_ollama = spawn_non_xllm_mock().await;
    let client = Client::new();

    // エンドポイント登録
    let response = client
        .post(format!("http://{}/api/endpoints", lb.addr()))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Ollama Test",
            "base_url": format!("http://{}", mock_ollama.addr())
        }))
        .send()
        .await
        .expect("registration request failed");

    assert_eq!(response.status().as_u16(), 201);
    let body: Value = response.json().await.unwrap();
    let endpoint_id = body["id"].as_str().unwrap();

    // /api/systemは非同期で呼ばれるため、少し待つ
    sleep(Duration::from_millis(500)).await;

    // エンドポイント詳細を取得
    let detail_response = client
        .get(format!(
            "http://{}/api/endpoints/{}",
            lb.addr(),
            endpoint_id
        ))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("get endpoint request failed");

    assert_eq!(detail_response.status().as_u16(), 200);
    let detail: Value = detail_response.json().await.unwrap();

    // device_infoがnullまたは存在しないことを確認
    // 非対応エンドポイントではdevice_infoは取得されない
    let device_info = detail.get("device_info");
    assert!(
        device_info.is_none() || device_info.unwrap().is_null(),
        "device_info should be null for unsupported endpoint: {:?}",
        detail
    );

    mock_ollama.stop().await;
}

/// CPU専用エンドポイントの/api/systemレスポンス
async fn cpu_system_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "device": {
                "device_type": "cpu",
                "gpu_devices": []
            }
        })),
    )
}

/// CPU専用のモックエンドポイントを起動
async fn spawn_cpu_mock() -> crate::support::http::TestServer {
    let app = Router::new()
        .route("/api/system", get(cpu_system_handler))
        .route("/v1/models", get(models_handler))
        .route("/health", get(health_handler));
    spawn_lb(app).await
}

/// T024-3: CPU専用エンドポイントのdevice_info取得
#[tokio::test]
async fn test_v0_system_cpu_device_info() {
    let lb = spawn_test_lb().await;
    let mock_cpu = spawn_cpu_mock().await;
    let client = Client::new();

    // エンドポイント登録
    let response = client
        .post(format!("http://{}/api/endpoints", lb.addr()))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "CPU Only",
            "base_url": format!("http://{}", mock_cpu.addr())
        }))
        .send()
        .await
        .expect("registration request failed");

    assert_eq!(response.status().as_u16(), 201);
    let body: Value = response.json().await.unwrap();
    let endpoint_id = body["id"].as_str().unwrap();

    // /api/systemは非同期で呼ばれるため、少し待つ
    sleep(Duration::from_millis(500)).await;

    // エンドポイント詳細を取得
    let detail_response = client
        .get(format!(
            "http://{}/api/endpoints/{}",
            lb.addr(),
            endpoint_id
        ))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("get endpoint request failed");

    assert_eq!(detail_response.status().as_u16(), 200);
    let detail: Value = detail_response.json().await.unwrap();

    // device_infoが取得されていることを確認
    assert!(
        detail.get("device_info").is_some() && !detail["device_info"].is_null(),
        "device_info should be retrieved: {:?}",
        detail
    );

    let device_info = &detail["device_info"];
    assert_eq!(
        device_info["device_type"], "cpu",
        "device_type should be 'cpu'"
    );
    assert!(
        device_info["gpu_devices"].as_array().unwrap().is_empty(),
        "gpu_devices should be empty for CPU-only endpoint"
    );

    mock_cpu.stop().await;
}
