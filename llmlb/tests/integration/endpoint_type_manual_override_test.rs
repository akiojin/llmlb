//! Integration Test: US11 - 手動タイプ指定
//!
//! SPEC-66555000: エンドポイントタイプ自動判別機能
//!
//! 管理者として、タイプを手動で指定・変更したい
//! （誤判別時の修正、またはオフラインエンドポイントの事前設定）。

use reqwest::Client;
use serde_json::{json, Value};

use crate::support::lb::spawn_test_lb;

/// US11-シナリオ1: 登録時に手動でタイプを指定
#[tokio::test]
#[ignore = "手動タイプ指定未実装 - T120で実装後に有効化"]
async fn test_manual_type_on_registration() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    // タイプを手動指定してエンドポイント登録
    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Manual xLLM",
            "base_url": "http://localhost:8080",
            "endpoint_type": "xllm"  // 手動指定
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 201);

    let body: Value = response.json().await.unwrap();

    // 手動指定したタイプが反映されていることを確認
    assert_eq!(
        body["endpoint_type"], "xllm",
        "Manual type should be applied"
    );
}

/// US11-シナリオ2: 既存エンドポイントのタイプを手動変更（PUT）
#[tokio::test]
#[ignore = "手動タイプ変更未実装 - T122で実装後に有効化"]
async fn test_manual_type_update() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    // エンドポイント登録（タイプはunknown）
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Test Endpoint",
            "base_url": "http://localhost:9999"
        }))
        .send()
        .await
        .unwrap();

    let body: Value = register_response.json().await.unwrap();
    let endpoint_id = body["id"].as_str().unwrap();

    // タイプをxLLMに手動変更
    let update_response = client
        .put(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Test Endpoint",
            "base_url": "http://localhost:9999",
            "endpoint_type": "xllm"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(update_response.status().as_u16(), 200);

    // 詳細取得で確認
    let detail_response = client
        .get(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let detail: Value = detail_response.json().await.unwrap();
    assert_eq!(
        detail["endpoint_type"], "xllm",
        "Type should be updated to xllm"
    );
}

/// US11-シナリオ3: 手動指定は自動判別より優先される
#[tokio::test]
#[ignore = "手動タイプ指定未実装 - T120, T122で実装後に有効化"]
async fn test_manual_type_overrides_auto_detection() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    // Ollamaエンドポイント（モック）を手動でxLLMとして登録
    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Manual Override",
            "base_url": "http://localhost:11434",  // Ollamaポート
            "endpoint_type": "xllm"  // 手動でxLLMを指定
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 201);

    let body: Value = response.json().await.unwrap();

    // 自動判別ではなく手動指定が優先される
    assert_eq!(
        body["endpoint_type"], "xllm",
        "Manual type should override auto-detection"
    );
}

/// US11-シナリオ4: 不正なタイプ指定はエラー
#[tokio::test]
#[ignore = "手動タイプ指定未実装 - T120で実装後に有効化"]
async fn test_invalid_type_specification() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    // 不正なタイプを指定
    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Invalid Type",
            "base_url": "http://localhost:8080",
            "endpoint_type": "invalid_type"  // 不正なタイプ
        }))
        .send()
        .await
        .unwrap();

    // 400 Bad Request または 422 Unprocessable Entity を期待
    let status = response.status().as_u16();
    assert!(
        status == 400 || status == 422,
        "invalid endpoint_type should be rejected with 400 or 422, got {status}"
    );
}

/// US11-シナリオ5: 全ての有効なタイプを手動指定可能
#[tokio::test]
#[ignore = "手動タイプ指定未実装 - T120で実装後に有効化"]
async fn test_all_valid_types_can_be_specified() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    let valid_types = ["xllm", "ollama", "vllm", "openai_compatible", "unknown"];

    for (i, endpoint_type) in valid_types.iter().enumerate() {
        let response = client
            .post(format!("http://{}/api/endpoints", server.addr()))
            .header("x-internal-token", "test-internal")
            .header("authorization", "Bearer sk_debug")
            .json(&json!({
                "name": format!("Endpoint Type {}", endpoint_type),
                "base_url": format!("http://localhost:{}", 8000 + i),
                "endpoint_type": endpoint_type
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(
            response.status().as_u16(),
            201,
            "Should accept type: {}",
            endpoint_type
        );

        let body: Value = response.json().await.unwrap();
        assert_eq!(
            body["endpoint_type"].as_str().unwrap(),
            *endpoint_type,
            "Type should match: {}",
            endpoint_type
        );
    }
}

/// US11-シナリオ6: タイプ変更後も他のフィールドは保持される
#[tokio::test]
#[ignore = "手動タイプ変更未実装 - T122で実装後に有効化"]
async fn test_type_update_preserves_other_fields() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    // エンドポイント登録（メモ付き）
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Test Endpoint",
            "base_url": "http://localhost:9999",
            "notes": "Important server"
        }))
        .send()
        .await
        .unwrap();

    let body: Value = register_response.json().await.unwrap();
    let endpoint_id = body["id"].as_str().unwrap();

    // タイプを変更（他のフィールドも指定）
    let update_response = client
        .put(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Test Endpoint",
            "base_url": "http://localhost:9999",
            "endpoint_type": "xllm",
            "notes": "Important server"  // メモを保持
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(update_response.status().as_u16(), 200);

    // 詳細取得でメモが保持されていることを確認
    let detail_response = client
        .get(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let detail: Value = detail_response.json().await.unwrap();
    assert_eq!(detail["endpoint_type"], "xllm");
    assert_eq!(
        detail["notes"], "Important server",
        "Notes should be preserved"
    );
}
