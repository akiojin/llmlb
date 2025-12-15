//! Contract Test: POST /v1/audio/transcriptions (ASR)
//!
//! OpenAI互換の音声認識APIの契約テスト。
//! TDD Red Phase: エンドポイント実装前のテスト定義

use std::sync::Arc;

use crate::support::{
    http::{spawn_router, TestServer},
    router::{register_node, spawn_test_router},
};
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use reqwest::{multipart, Client, StatusCode as ReqStatusCode};
use serde_json::Value;
use serial_test::serial;

/// ASRスタブサーバーの状態
#[derive(Clone)]
struct AsrStubState {
    expected_model: Option<String>,
    response: AsrStubResponse,
}

/// ASRスタブのレスポンス種別
#[derive(Clone)]
enum AsrStubResponse {
    /// 成功レスポンス（認識テキスト）
    Success(String),
    /// エラーレスポンス
    Error(StatusCode, String),
}

/// ASRスタブサーバーを起動
async fn spawn_asr_stub(state: AsrStubState) -> TestServer {
    let router = Router::new()
        .route("/v1/audio/transcriptions", post(asr_handler))
        .route("/v1/models", get(models_handler))
        .route("/api/tags", get(tags_handler))
        .route("/api/health", post(|| async { StatusCode::OK }))
        .with_state(Arc::new(state));

    spawn_router(router).await
}

/// ASRエンドポイントハンドラ（スタブ）
async fn asr_handler(State(state): State<Arc<AsrStubState>>) -> impl IntoResponse {
    match &state.response {
        AsrStubResponse::Success(text) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "text": text
            })),
        )
            .into_response(),
        AsrStubResponse::Error(status, msg) => (*status, msg.clone()).into_response(),
    }
}

/// モデル一覧ハンドラ（スタブ）
async fn models_handler(State(state): State<Arc<AsrStubState>>) -> impl IntoResponse {
    let models: Vec<_> = if let Some(model) = &state.expected_model {
        vec![serde_json::json!({"id": model})]
    } else {
        vec![serde_json::json!({"id": "whisper-large-v3"})]
    };
    (StatusCode::OK, Json(serde_json::json!({"data": models}))).into_response()
}

/// タグ一覧ハンドラ（スタブ）
async fn tags_handler(State(state): State<Arc<AsrStubState>>) -> impl IntoResponse {
    let models: Vec<_> = if let Some(model) = &state.expected_model {
        vec![serde_json::json!({"name": model, "size": 3_000_000_000i64})]
    } else {
        vec![serde_json::json!({"name": "whisper-large-v3", "size": 3_000_000_000i64})]
    };
    (StatusCode::OK, Json(serde_json::json!({"models": models}))).into_response()
}

/// T006: POST /v1/audio/transcriptions 正常系
///
/// 契約:
/// - multipart/form-data形式でリクエスト
/// - file (binary) と model (string) が必須
/// - レスポンスは { "text": "..." } 形式
#[tokio::test]
#[serial]
#[ignore = "TDD RED: /v1/audio/transcriptions endpoint not implemented yet"]
async fn transcriptions_end_to_end_success() {
    let asr_stub = spawn_asr_stub(AsrStubState {
        expected_model: Some("whisper-large-v3".to_string()),
        response: AsrStubResponse::Success("こんにちは".to_string()),
    })
    .await;

    let coordinator = spawn_test_router().await;

    let register_response = register_node(coordinator.addr(), asr_stub.addr())
        .await
        .expect("register agent must succeed");
    assert_eq!(register_response.status(), ReqStatusCode::CREATED);

    // WAV形式のダミー音声データ（最小限のヘッダ）
    let dummy_wav = vec![
        0x52, 0x49, 0x46, 0x46, // "RIFF"
        0x24, 0x00, 0x00, 0x00, // Chunk size
        0x57, 0x41, 0x56, 0x45, // "WAVE"
        0x66, 0x6d, 0x74, 0x20, // "fmt "
        0x10, 0x00, 0x00, 0x00, // Subchunk1 size
        0x01, 0x00, // Audio format (PCM)
        0x01, 0x00, // Num channels
        0x22, 0x56, 0x00, 0x00, // Sample rate (22050)
        0x44, 0xAC, 0x00, 0x00, // Byte rate
        0x02, 0x00, // Block align
        0x10, 0x00, // Bits per sample
        0x64, 0x61, 0x74, 0x61, // "data"
        0x00, 0x00, 0x00, 0x00, // Subchunk2 size
    ];

    let client = Client::new();
    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(dummy_wav)
                .file_name("test.wav")
                .mime_str("audio/wav")
                .unwrap(),
        )
        .text("model", "whisper-large-v3");

    let response = client
        .post(format!(
            "http://{}/v1/audio/transcriptions",
            coordinator.addr()
        ))
        .header("x-api-key", "sk_debug")
        .multipart(form)
        .send()
        .await
        .expect("transcription request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    let body: Value = response.json().await.expect("valid json response");
    assert_eq!(body["text"], "こんにちは");
}

/// T006: POST /v1/audio/transcriptions エラー系（不正なフォーマット）
///
/// 契約:
/// - 未サポートのファイル形式は 400 Bad Request を返す
/// - エラーレスポンスは OpenAI API形式
#[tokio::test]
#[serial]
#[ignore = "TDD RED: /v1/audio/transcriptions endpoint not implemented yet"]
async fn transcriptions_unsupported_format_returns_400() {
    let asr_stub = spawn_asr_stub(AsrStubState {
        expected_model: None,
        response: AsrStubResponse::Error(
            StatusCode::BAD_REQUEST,
            r#"{"error":{"message":"Unsupported audio format","type":"invalid_request_error"}}"#
                .to_string(),
        ),
    })
    .await;

    let coordinator = spawn_test_router().await;

    let register_response = register_node(coordinator.addr(), asr_stub.addr())
        .await
        .expect("register agent must succeed");
    assert_eq!(register_response.status(), ReqStatusCode::CREATED);

    // 不正なファイルデータ
    let invalid_data = vec![0x00, 0x01, 0x02, 0x03];

    let client = Client::new();
    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(invalid_data)
                .file_name("test.xyz")
                .mime_str("application/octet-stream")
                .unwrap(),
        )
        .text("model", "whisper-large-v3");

    let response = client
        .post(format!(
            "http://{}/v1/audio/transcriptions",
            coordinator.addr()
        ))
        .header("x-api-key", "sk_debug")
        .multipart(form)
        .send()
        .await
        .expect("request should complete");

    assert_eq!(response.status(), ReqStatusCode::BAD_REQUEST);
    let body: Value = response.json().await.expect("valid json response");
    assert!(body["error"]["message"].as_str().is_some());
    assert_eq!(body["error"]["type"], "invalid_request_error");
}

/// T006: POST /v1/audio/transcriptions 認証エラー
///
/// 契約:
/// - APIキーなしの場合は 401 Unauthorized を返す
#[tokio::test]
#[serial]
#[ignore = "TDD RED: /v1/audio/transcriptions endpoint not implemented yet"]
async fn transcriptions_without_auth_returns_401() {
    let coordinator = spawn_test_router().await;

    let dummy_wav = vec![0x52, 0x49, 0x46, 0x46]; // 最小限のWAVヘッダ

    let client = Client::new();
    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(dummy_wav)
                .file_name("test.wav")
                .mime_str("audio/wav")
                .unwrap(),
        )
        .text("model", "whisper-large-v3");

    let response = client
        .post(format!(
            "http://{}/v1/audio/transcriptions",
            coordinator.addr()
        ))
        // x-api-key ヘッダなし
        .multipart(form)
        .send()
        .await
        .expect("request should complete");

    assert_eq!(response.status(), ReqStatusCode::UNAUTHORIZED);
}

/// T006: POST /v1/audio/transcriptions 利用可能ノードなし
///
/// 契約:
/// - ASR対応ノードがない場合は 503 Service Unavailable を返す
#[tokio::test]
#[serial]
#[ignore = "TDD RED: /v1/audio/transcriptions endpoint not implemented yet"]
async fn transcriptions_no_available_node_returns_503() {
    // ノードを登録しない
    let coordinator = spawn_test_router().await;

    let dummy_wav = vec![0x52, 0x49, 0x46, 0x46];

    let client = Client::new();
    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(dummy_wav)
                .file_name("test.wav")
                .mime_str("audio/wav")
                .unwrap(),
        )
        .text("model", "whisper-large-v3");

    let response = client
        .post(format!(
            "http://{}/v1/audio/transcriptions",
            coordinator.addr()
        ))
        .header("x-api-key", "sk_debug")
        .multipart(form)
        .send()
        .await
        .expect("request should complete");

    assert_eq!(response.status(), ReqStatusCode::SERVICE_UNAVAILABLE);
}
