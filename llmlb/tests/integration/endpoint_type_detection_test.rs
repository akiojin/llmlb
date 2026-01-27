//! Integration Test: US6 - エンドポイントタイプ自動判別
//!
//! SPEC-66555000: エンドポイントタイプ自動判別機能
//!
//! 管理者として、エンドポイント登録時に自動的にタイプ
//! （xLLM/Ollama/vLLM/OpenAI互換）を判別してほしい。

use reqwest::Client;
use serde_json::{json, Value};

use crate::support::lb::spawn_test_lb;

/// US6-シナリオ1: エンドポイント登録時にタイプが自動判別される
/// NOTE: 実際のエンドポイントがないとタイプ判別できないため、unknownになる
#[tokio::test]
async fn test_endpoint_type_auto_detection_offline() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    // エンドポイント登録（接続先がないのでタイプはunknownになる）
    let response = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Unknown Endpoint",
            "base_url": "http://localhost:9999"
        }))
        .send()
        .await
        .expect("registration request failed");

    assert_eq!(response.status().as_u16(), 201);

    let body: Value = response.json().await.unwrap();

    // endpoint_typeフィールドが存在し、オフラインの場合はunknownになる
    assert!(
        body["endpoint_type"].is_string(),
        "endpoint_type should be present in response"
    );
    assert_eq!(
        body["endpoint_type"], "unknown",
        "Offline endpoint should have unknown type"
    );
}

/// US6-シナリオ2: 判別の優先順位（xLLM > Ollama > vLLM > OpenAI互換）
/// NOTE: モックサーバーを使用したテストが必要（実際の判別ロジック検証）
#[tokio::test]
#[ignore = "モックサーバーが必要 - T112-T116で実装後に有効化"]
async fn test_endpoint_type_detection_priority() {
    let server = spawn_test_lb().await;
    let _client = Client::new();

    // xLLMエンドポイント判別テスト
    // GET /v0/system が xllm_version を返す場合 → xLLMタイプ

    // Ollamaエンドポイント判別テスト
    // GET /api/tags が成功する場合 → Ollamaタイプ

    // vLLMエンドポイント判別テスト
    // Serverヘッダーに vllm が含まれる場合 → vLLMタイプ

    // OpenAI互換判別テスト
    // 上記すべてに該当しない場合 → OpenAI互換タイプ

    // テストはモックサーバーを用いて実装
    let _ = server;
}

/// US6-シナリオ3: xLLM判別（/v0/systemエンドポイント）
#[tokio::test]
#[ignore = "モックサーバーが必要 - T113で実装後に有効化"]
async fn test_endpoint_type_detection_xllm() {
    let server = spawn_test_lb().await;
    let _client = Client::new();

    // xLLM判別テスト
    // モックサーバーが GET /v0/system に対して
    // { "xllm_version": "0.1.0" } を返す場合
    // → endpoint_type が "xllm" になることを確認

    let _ = server;
}

/// US6-シナリオ4: Ollama判別（/api/tagsエンドポイント）
#[tokio::test]
#[ignore = "モックサーバーが必要 - T114で実装後に有効化"]
async fn test_endpoint_type_detection_ollama() {
    let server = spawn_test_lb().await;
    let _client = Client::new();

    // Ollama判別テスト
    // モックサーバーが GET /api/tags に対して成功レスポンスを返す場合
    // → endpoint_type が "ollama" になることを確認

    let _ = server;
}

/// US6-シナリオ5: vLLM判別（Serverヘッダー）
#[tokio::test]
#[ignore = "モックサーバーが必要 - T115で実装後に有効化"]
async fn test_endpoint_type_detection_vllm() {
    let server = spawn_test_lb().await;
    let _client = Client::new();

    // vLLM判別テスト
    // モックサーバーが Server: vllm ヘッダーを返す場合
    // → endpoint_type が "vllm" になることを確認

    let _ = server;
}

/// US6-シナリオ6: OpenAI互換判別（フォールバック）
#[tokio::test]
#[ignore = "モックサーバーが必要 - T116で実装後に有効化"]
async fn test_endpoint_type_detection_openai_compatible() {
    let server = spawn_test_lb().await;
    let _client = Client::new();

    // OpenAI互換判別テスト
    // モックサーバーが /v1/models に成功レスポンスを返すが
    // xLLM/Ollama/vLLMのいずれにも該当しない場合
    // → endpoint_type が "openai_compatible" になることを確認

    let _ = server;
}

/// US6-シナリオ7: オンライン復帰時のタイプ再判別
#[tokio::test]
#[ignore = "T132で実装後に有効化"]
async fn test_endpoint_type_redetection_on_online() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    // 最初はオフラインでunknownタイプ
    let response = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Test Endpoint",
            "base_url": "http://localhost:9999"
        }))
        .send()
        .await
        .expect("registration request failed");

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["endpoint_type"], "unknown");

    // TODO: エンドポイントをオンラインにして
    // ヘルスチェックでタイプが再判別されることを確認
}
