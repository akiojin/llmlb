//! Contract Test: POST /v1/images/generations (Image Generation)
//!
//! OpenAI互換の画像生成APIの契約テスト。
//! TDD Red Phase: エンドポイント実装前のテスト定義

use std::sync::Arc;

use crate::support::{
    http::{spawn_router, TestServer},
    router::{register_image_generation_endpoint, spawn_test_router},
};
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use reqwest::{Client, StatusCode as ReqStatusCode};
use serde_json::{json, Value};
use serial_test::serial;

/// 画像生成スタブサーバーの状態
#[derive(Clone)]
struct ImageGenStubState {
    expected_model: Option<String>,
    response: ImageGenStubResponse,
}

/// 画像生成スタブのレスポンス種別
#[derive(Clone)]
#[allow(dead_code)] // Error variant prepared for future TDD GREEN phase
enum ImageGenStubResponse {
    /// 成功レスポンス（URL形式）
    SuccessUrl(Vec<String>),
    /// 成功レスポンス（Base64形式）
    SuccessBase64(Vec<String>),
    /// エラーレスポンス
    Error(StatusCode, String),
}

/// 画像生成スタブサーバーを起動
async fn spawn_image_gen_stub(state: ImageGenStubState) -> TestServer {
    let router = Router::new()
        .route("/v1/images/generations", post(image_gen_handler))
        .route("/v1/models", get(models_handler))
        .route("/api/tags", get(tags_handler))
        .route("/api/health", post(|| async { StatusCode::OK }))
        .with_state(Arc::new(state));

    spawn_router(router).await
}

/// 画像生成エンドポイントハンドラ（スタブ）
async fn image_gen_handler(
    State(state): State<Arc<ImageGenStubState>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    // リクエスト検証
    let prompt = payload.get("prompt").and_then(|v| v.as_str());
    if prompt.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "message": "Missing required field: prompt",
                    "type": "invalid_request_error",
                    "code": "missing_prompt"
                }
            })),
        )
            .into_response();
    }

    match &state.response {
        ImageGenStubResponse::SuccessUrl(urls) => {
            let data: Vec<_> = urls.iter().map(|url| json!({"url": url})).collect();
            (
                StatusCode::OK,
                Json(json!({
                    "created": chrono::Utc::now().timestamp(),
                    "data": data
                })),
            )
                .into_response()
        }
        ImageGenStubResponse::SuccessBase64(b64s) => {
            let data: Vec<_> = b64s.iter().map(|b64| json!({"b64_json": b64})).collect();
            (
                StatusCode::OK,
                Json(json!({
                    "created": chrono::Utc::now().timestamp(),
                    "data": data
                })),
            )
                .into_response()
        }
        ImageGenStubResponse::Error(status, msg) => {
            (*status, Json(json!({"error": {"message": msg}}))).into_response()
        }
    }
}

/// モデル一覧ハンドラ（スタブ）
async fn models_handler(State(state): State<Arc<ImageGenStubState>>) -> impl IntoResponse {
    let models: Vec<_> = if let Some(model) = &state.expected_model {
        vec![json!({"id": model})]
    } else {
        vec![json!({"id": "stable-diffusion-xl"})]
    };
    (StatusCode::OK, Json(json!({"data": models}))).into_response()
}

/// タグ一覧ハンドラ（スタブ）
async fn tags_handler(State(state): State<Arc<ImageGenStubState>>) -> impl IntoResponse {
    let models: Vec<_> = if let Some(model) = &state.expected_model {
        vec![json!({"name": model, "size": 5_000_000_000i64})]
    } else {
        vec![json!({"name": "stable-diffusion-xl", "size": 5_000_000_000i64})]
    };
    (StatusCode::OK, Json(json!({"models": models}))).into_response()
}

// =============================================================================
// Contract Tests
// =============================================================================

/// I001: POST /v1/images/generations 正常系
///
/// 契約:
/// - application/json形式でリクエスト
/// - model (string) と prompt (string) が必須
/// - レスポンスは created (timestamp) と data (array of image objects)
#[tokio::test]
#[serial]
async fn images_generations_success() {
    let stub_state = ImageGenStubState {
        expected_model: Some("stable-diffusion-xl".to_string()),
        response: ImageGenStubResponse::SuccessUrl(vec![
            "https://example.com/generated-image.png".to_string()
        ]),
    };

    let stub = spawn_image_gen_stub(stub_state).await;
    let router = spawn_test_router().await;

    // EndpointRegistry経由で画像生成エンドポイントを登録
    let _endpoint_id = register_image_generation_endpoint(router.addr(), stub.addr())
        .await
        .expect("register endpoint must succeed");

    let client = Client::new();
    let res = client
        .post(format!("http://{}/v1/images/generations", router.addr()))
        .header("x-api-key", "sk_debug")
        .json(&json!({
            "model": "stable-diffusion-xl",
            "prompt": "A white cat sitting on a windowsill",
            "n": 1,
            "size": "1024x1024"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), ReqStatusCode::OK);

    let body: Value = res.json().await.unwrap();
    assert!(body.get("created").is_some());
    assert!(body.get("data").is_some());

    let data = body["data"].as_array().unwrap();
    assert_eq!(data.len(), 1);
    assert!(data[0].get("url").is_some());
}

/// I002: POST /v1/images/generations Base64形式レスポンス
#[tokio::test]
#[serial]
async fn images_generations_base64_response() {
    // 1x1ピクセルの透明PNGをBase64エンコード
    let dummy_png_b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==".to_string();

    let stub_state = ImageGenStubState {
        expected_model: Some("stable-diffusion-xl".to_string()),
        response: ImageGenStubResponse::SuccessBase64(vec![dummy_png_b64.clone()]),
    };

    let stub = spawn_image_gen_stub(stub_state).await;
    let router = spawn_test_router().await;

    // EndpointRegistry経由で画像生成エンドポイントを登録
    let _endpoint_id = register_image_generation_endpoint(router.addr(), stub.addr())
        .await
        .expect("register endpoint must succeed");

    let client = Client::new();
    let res = client
        .post(format!("http://{}/v1/images/generations", router.addr()))
        .header("x-api-key", "sk_debug")
        .json(&json!({
            "model": "stable-diffusion-xl",
            "prompt": "A white cat",
            "response_format": "b64_json"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), ReqStatusCode::OK);

    let body: Value = res.json().await.unwrap();
    let data = body["data"].as_array().unwrap();
    assert!(data[0].get("b64_json").is_some());
}

/// I003: POST /v1/images/generations 複数画像生成 (n > 1)
#[tokio::test]
#[serial]
async fn images_generations_multiple() {
    let stub_state = ImageGenStubState {
        expected_model: Some("stable-diffusion-xl".to_string()),
        response: ImageGenStubResponse::SuccessUrl(vec![
            "https://example.com/image1.png".to_string(),
            "https://example.com/image2.png".to_string(),
            "https://example.com/image3.png".to_string(),
        ]),
    };

    let stub = spawn_image_gen_stub(stub_state).await;
    let router = spawn_test_router().await;

    // EndpointRegistry経由で画像生成エンドポイントを登録
    let _endpoint_id = register_image_generation_endpoint(router.addr(), stub.addr())
        .await
        .expect("register endpoint must succeed");

    let client = Client::new();
    let res = client
        .post(format!("http://{}/v1/images/generations", router.addr()))
        .header("x-api-key", "sk_debug")
        .json(&json!({
            "model": "stable-diffusion-xl",
            "prompt": "A beautiful sunset",
            "n": 3
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), ReqStatusCode::OK);

    let body: Value = res.json().await.unwrap();
    let data = body["data"].as_array().unwrap();
    assert_eq!(data.len(), 3);
}

/// I004: POST /v1/images/generations 必須フィールド欠落
#[tokio::test]
#[serial]
async fn images_generations_missing_prompt() {
    let stub_state = ImageGenStubState {
        expected_model: None,
        response: ImageGenStubResponse::SuccessUrl(vec![]),
    };

    let stub = spawn_image_gen_stub(stub_state).await;
    let router = spawn_test_router().await;

    // EndpointRegistry経由で画像生成エンドポイントを登録
    let _endpoint_id = register_image_generation_endpoint(router.addr(), stub.addr())
        .await
        .expect("register endpoint must succeed");

    let client = Client::new();
    let res = client
        .post(format!("http://{}/v1/images/generations", router.addr()))
        .header("x-api-key", "sk_debug")
        .json(&json!({
            "model": "stable-diffusion-xl",
            "prompt": ""
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), ReqStatusCode::BAD_REQUEST);

    let body: Value = res.json().await.unwrap();
    assert!(body.get("error").is_some());
}

/// I005: POST /v1/images/generations 認証なし
#[tokio::test]
#[serial]
async fn images_generations_unauthorized() {
    let router = spawn_test_router().await;

    let client = Client::new();
    let res = client
        .post(format!("http://{}/v1/images/generations", router.addr()))
        // No Authorization header
        .json(&json!({
            "model": "stable-diffusion-xl",
            "prompt": "A cat"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), ReqStatusCode::UNAUTHORIZED);
}

/// I006: POST /v1/images/generations 利用可能なノードなし
#[tokio::test]
#[serial]
async fn images_generations_no_node_available() {
    let router = spawn_test_router().await;

    // ノードを登録しない

    let client = Client::new();
    let res = client
        .post(format!("http://{}/v1/images/generations", router.addr()))
        .header("x-api-key", "sk_debug")
        .json(&json!({
            "model": "stable-diffusion-xl",
            "prompt": "A cat"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), ReqStatusCode::SERVICE_UNAVAILABLE);
}

/// I007: POST /v1/images/generations 各種オプションパラメータ
#[tokio::test]
#[serial]
async fn images_generations_with_options() {
    let stub_state = ImageGenStubState {
        expected_model: Some("stable-diffusion-xl".to_string()),
        response: ImageGenStubResponse::SuccessUrl(vec![
            "https://example.com/image.png".to_string()
        ]),
    };

    let stub = spawn_image_gen_stub(stub_state).await;
    let router = spawn_test_router().await;

    // EndpointRegistry経由で画像生成エンドポイントを登録
    let _endpoint_id = register_image_generation_endpoint(router.addr(), stub.addr())
        .await
        .expect("register endpoint must succeed");

    let client = Client::new();
    let res = client
        .post(format!("http://{}/v1/images/generations", router.addr()))
        .header("x-api-key", "sk_debug")
        .json(&json!({
            "model": "stable-diffusion-xl",
            "prompt": "A white cat",
            "n": 1,
            "size": "1024x1024",
            "quality": "hd",
            "style": "vivid",
            "response_format": "url"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), ReqStatusCode::OK);
}
