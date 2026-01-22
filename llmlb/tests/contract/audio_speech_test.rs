//! Contract Test: POST /v1/audio/speech (TTS)
//!
//! OpenAI互換の音声合成APIの契約テスト。
//! TDD Red Phase: エンドポイント実装前のテスト定義

use std::sync::Arc;

use crate::support::{
    http::{spawn_lb, TestServer},
    lb::{register_audio_speech_endpoint, spawn_test_lb},
};
use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use reqwest::{Client, StatusCode as ReqStatusCode};
use serde_json::Value;
use serial_test::serial;

/// TTSスタブサーバーの状態
#[derive(Clone)]
struct TtsStubState {
    expected_model: Option<String>,
    response: TtsStubResponse,
}

/// TTSスタブのレスポンス種別
#[derive(Clone)]
enum TtsStubResponse {
    /// 成功レスポンス（音声バイナリ）
    Success(Vec<u8>),
    /// エラーレスポンス
    #[allow(dead_code)]
    Error(StatusCode, String),
}

/// TTSスタブサーバーを起動
async fn spawn_tts_stub(state: TtsStubState) -> TestServer {
    let app = Router::new()
        .route("/v1/audio/speech", post(tts_handler))
        .route("/v1/models", get(models_handler))
        .route("/api/tags", get(tags_handler))
        .route("/api/health", post(|| async { StatusCode::OK }))
        .with_state(Arc::new(state));

    spawn_lb(app).await
}

/// TTSエンドポイントハンドラ（スタブ）
async fn tts_handler(State(state): State<Arc<TtsStubState>>) -> impl IntoResponse {
    match &state.response {
        TtsStubResponse::Success(audio_data) => {
            let body = Body::from(audio_data.clone());
            (StatusCode::OK, [(header::CONTENT_TYPE, "audio/mpeg")], body).into_response()
        }
        TtsStubResponse::Error(status, msg) => (*status, msg.clone()).into_response(),
    }
}

/// モデル一覧ハンドラ（スタブ）
async fn models_handler(State(state): State<Arc<TtsStubState>>) -> impl IntoResponse {
    let models: Vec<_> = if let Some(model) = &state.expected_model {
        vec![serde_json::json!({"id": model})]
    } else {
        vec![serde_json::json!({"id": "vibevoice-v1"})]
    };
    (StatusCode::OK, Json(serde_json::json!({"data": models}))).into_response()
}

/// タグ一覧ハンドラ（スタブ）
async fn tags_handler(State(state): State<Arc<TtsStubState>>) -> impl IntoResponse {
    let models: Vec<_> = if let Some(model) = &state.expected_model {
        vec![serde_json::json!({"name": model, "size": 500_000_000i64})]
    } else {
        vec![serde_json::json!({"name": "vibevoice-v1", "size": 500_000_000i64})]
    };
    (StatusCode::OK, Json(serde_json::json!({"models": models}))).into_response()
}

/// T007: POST /v1/audio/speech 正常系
///
/// 契約:
/// - application/json形式でリクエスト
/// - model (string) と input (string) が必須
/// - レスポンスは audio/mpeg バイナリ
#[tokio::test]
#[serial]
async fn speech_end_to_end_success() {
    // ダミーMP3データ（ID3タグの最小ヘッダ）
    let dummy_mp3 = vec![
        0x49, 0x44, 0x33, // "ID3"
        0x04, 0x00, // version
        0x00, // flags
        0x00, 0x00, 0x00, 0x00, // size
    ];

    let tts_stub = spawn_tts_stub(TtsStubState {
        expected_model: Some("vibevoice-v1".to_string()),
        response: TtsStubResponse::Success(dummy_mp3.clone()),
    })
    .await;

    let coordinator = spawn_test_lb().await;

    // EndpointRegistry経由でTTSエンドポイントを登録
    let _endpoint_id = register_audio_speech_endpoint(coordinator.addr(), tts_stub.addr())
        .await
        .expect("register endpoint must succeed");

    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/audio/speech", coordinator.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "model": "vibevoice-v1",
            "input": "こんにちは"
        }))
        .send()
        .await
        .expect("speech request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("audio/mpeg")
    );

    let body = response.bytes().await.expect("body should be readable");
    assert!(!body.is_empty());
}

/// T007: POST /v1/audio/speech オプションパラメータ
///
/// 契約:
/// - voice, response_format, speed はオプション
/// - デフォルト値: voice=nova, response_format=mp3, speed=1.0
#[tokio::test]
#[serial]
async fn speech_with_optional_params() {
    let dummy_wav = vec![
        0x52, 0x49, 0x46, 0x46, // "RIFF"
        0x00, 0x00, 0x00, 0x00, // size
        0x57, 0x41, 0x56, 0x45, // "WAVE"
    ];

    let tts_stub = spawn_tts_stub(TtsStubState {
        expected_model: Some("vibevoice-v1".to_string()),
        response: TtsStubResponse::Success(dummy_wav),
    })
    .await;

    let coordinator = spawn_test_lb().await;

    // EndpointRegistry経由でTTSエンドポイントを登録
    let _endpoint_id = register_audio_speech_endpoint(coordinator.addr(), tts_stub.addr())
        .await
        .expect("register endpoint must succeed");

    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/audio/speech", coordinator.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "model": "vibevoice-v1",
            "input": "テスト",
            "voice": "echo",
            "response_format": "wav",
            "speed": 1.5
        }))
        .send()
        .await
        .expect("speech request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
}

/// T007: POST /v1/audio/speech エラー系（空の入力）
///
/// 契約:
/// - input が空の場合は 400 Bad Request を返す
#[tokio::test]
#[serial]
async fn speech_empty_input_returns_400() {
    // ノードを登録しなくても400を返すことを検証（入力検証はルーター側で行う）
    let coordinator = spawn_test_lb().await;

    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/audio/speech", coordinator.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "model": "vibevoice-v1",
            "input": ""
        }))
        .send()
        .await
        .expect("request should complete");

    assert_eq!(response.status(), ReqStatusCode::BAD_REQUEST);
    let body: Value = response.json().await.expect("valid json response");
    assert!(body["error"]["message"].as_str().is_some());
}

/// T007: POST /v1/audio/speech 認証エラー
///
/// 契約:
/// - APIキーなしの場合は 401 Unauthorized を返す
#[tokio::test]
#[serial]
async fn speech_without_auth_returns_401() {
    let coordinator = spawn_test_lb().await;

    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/audio/speech", coordinator.addr()))
        // x-api-key ヘッダなし
        .json(&serde_json::json!({
            "model": "vibevoice-v1",
            "input": "テスト"
        }))
        .send()
        .await
        .expect("request should complete");

    assert_eq!(response.status(), ReqStatusCode::UNAUTHORIZED);
}

/// T007: POST /v1/audio/speech 利用可能ノードなし
///
/// 契約:
/// - TTS対応ノードがない場合は 503 Service Unavailable を返す
#[tokio::test]
#[serial]
async fn speech_no_available_node_returns_503() {
    // ノードを登録しない
    let coordinator = spawn_test_lb().await;

    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/audio/speech", coordinator.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "model": "vibevoice-v1",
            "input": "テスト"
        }))
        .send()
        .await
        .expect("request should complete");

    assert_eq!(response.status(), ReqStatusCode::SERVICE_UNAVAILABLE);
}

/// T007: POST /v1/audio/speech 入力サイズ制限
///
/// 契約:
/// - input が 4096 文字を超える場合は 400 Bad Request を返す
#[tokio::test]
#[serial]
async fn speech_input_too_long_returns_400() {
    // ノードを登録しなくても400を返すことを検証（入力検証はルーター側で行う）
    let coordinator = spawn_test_lb().await;

    // 4097文字のテキスト
    let long_input = "あ".repeat(4097);

    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/audio/speech", coordinator.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "model": "vibevoice-v1",
            "input": long_input
        }))
        .send()
        .await
        .expect("request should complete");

    assert_eq!(response.status(), ReqStatusCode::BAD_REQUEST);
}
