//! Integration Test: US7 - タイプフィルタリング
//!
//! SPEC-66555000: エンドポイントタイプ自動判別機能
//!
//! 管理者として、特定タイプのエンドポイントのみを
//! フィルタリングして一覧表示したい。

use reqwest::Client;
use serde_json::{json, Value};

use crate::support::lb::spawn_test_lb;

/// US7-シナリオ1: タイプパラメータなしの場合、全エンドポイントを返す
#[tokio::test]
async fn test_list_endpoints_without_type_filter() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    // 複数エンドポイントを登録
    for i in 1..=3 {
        let _ = client
            .post(format!("http://{}/api/endpoints", server.addr()))
            .header("x-internal-token", "test-internal")
            .header("authorization", "Bearer sk_debug")
            .json(&json!({
                "name": format!("Endpoint {}", i),
                "base_url": format!("http://localhost:{}", 9000 + i)
            }))
            .send()
            .await
            .unwrap();
    }

    // フィルタなしで取得
    let response = client
        .get(format!("http://{}/api/endpoints", server.addr()))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("list request failed");

    assert_eq!(response.status().as_u16(), 200);

    let body: Value = response.json().await.unwrap();
    let endpoints = body["endpoints"].as_array().unwrap();

    assert_eq!(endpoints.len(), 3, "Should return all endpoints");
}

/// US7-シナリオ2: type=xllmでフィルタリング
#[tokio::test]
#[ignore = "タイプフィルタ未実装 - T121で実装後に有効化"]
async fn test_list_endpoints_filter_by_xllm() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    // xLLMタイプでフィルタ
    let response = client
        .get(format!("http://{}/api/endpoints?type=xllm", server.addr()))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("list request failed");

    assert_eq!(response.status().as_u16(), 200);

    let body: Value = response.json().await.unwrap();
    let endpoints = body["endpoints"].as_array().unwrap();

    // すべてのエンドポイントがxLLMタイプであることを確認
    for endpoint in endpoints {
        assert_eq!(
            endpoint["endpoint_type"], "xllm",
            "All filtered endpoints should be xLLM type"
        );
    }
}

/// US7-シナリオ3: type=ollamaでフィルタリング
#[tokio::test]
#[ignore = "タイプフィルタ未実装 - T121で実装後に有効化"]
async fn test_list_endpoints_filter_by_ollama() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    // Ollamaタイプでフィルタ
    let response = client
        .get(format!(
            "http://{}/api/endpoints?type=ollama",
            server.addr()
        ))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("list request failed");

    assert_eq!(response.status().as_u16(), 200);

    let body: Value = response.json().await.unwrap();
    let endpoints = body["endpoints"].as_array().unwrap();

    for endpoint in endpoints {
        assert_eq!(endpoint["endpoint_type"], "ollama");
    }
}

/// US7-シナリオ4: type=vllmでフィルタリング
#[tokio::test]
#[ignore = "タイプフィルタ未実装 - T121で実装後に有効化"]
async fn test_list_endpoints_filter_by_vllm() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    let response = client
        .get(format!("http://{}/api/endpoints?type=vllm", server.addr()))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("list request failed");

    assert_eq!(response.status().as_u16(), 200);

    let body: Value = response.json().await.unwrap();
    let endpoints = body["endpoints"].as_array().unwrap();

    for endpoint in endpoints {
        assert_eq!(endpoint["endpoint_type"], "vllm");
    }
}

/// US7-シナリオ5: type=openai_compatibleでフィルタリング
#[tokio::test]
#[ignore = "タイプフィルタ未実装 - T121で実装後に有効化"]
async fn test_list_endpoints_filter_by_openai_compatible() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    let response = client
        .get(format!(
            "http://{}/api/endpoints?type=openai_compatible",
            server.addr()
        ))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("list request failed");

    assert_eq!(response.status().as_u16(), 200);

    let body: Value = response.json().await.unwrap();
    let endpoints = body["endpoints"].as_array().unwrap();

    for endpoint in endpoints {
        assert_eq!(endpoint["endpoint_type"], "openai_compatible");
    }
}

/// US7-シナリオ6: type=unknownでフィルタリング
#[tokio::test]
#[ignore = "タイプフィルタ未実装 - T121で実装後に有効化"]
async fn test_list_endpoints_filter_by_unknown() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    // オフラインエンドポイントを登録（unknownタイプになる）
    let _ = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Offline Endpoint",
            "base_url": "http://localhost:9999"
        }))
        .send()
        .await
        .unwrap();

    // unknownタイプでフィルタ
    let response = client
        .get(format!(
            "http://{}/api/endpoints?type=unknown",
            server.addr()
        ))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("list request failed");

    assert_eq!(response.status().as_u16(), 200);

    let body: Value = response.json().await.unwrap();
    let endpoints = body["endpoints"].as_array().unwrap();

    // オフラインエンドポイントが含まれることを確認
    assert!(
        !endpoints.is_empty(),
        "Should have at least one unknown endpoint"
    );
    for endpoint in endpoints {
        assert_eq!(endpoint["endpoint_type"], "unknown");
    }
}

/// US7-シナリオ7: 複数フィルタの組み合わせ（type + status）
#[tokio::test]
#[ignore = "タイプフィルタ未実装 - T121で実装後に有効化"]
async fn test_list_endpoints_combined_filters() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    // type=xllm かつ status=pending でフィルタ
    let response = client
        .get(format!(
            "http://{}/api/endpoints?type=xllm&status=pending",
            server.addr()
        ))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("list request failed");

    assert_eq!(response.status().as_u16(), 200);

    let body: Value = response.json().await.unwrap();
    let endpoints = body["endpoints"].as_array().unwrap();

    for endpoint in endpoints {
        assert_eq!(endpoint["endpoint_type"], "xllm");
        assert_eq!(endpoint["status"], "pending");
    }
}
