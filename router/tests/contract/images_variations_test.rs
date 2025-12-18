//! Contract Test: POST /v1/images/variations (Image Variations)
//!
//! OpenAI互換の画像バリエーションAPIの契約テスト。
//! TDD Red Phase: エンドポイント実装前のテスト定義

use std::sync::Arc;

use crate::support::{
    http::{spawn_router, TestServer},
    router::{register_node_with_runtimes, spawn_test_router},
};
use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use reqwest::{
    multipart::{Form, Part},
    Client, StatusCode as ReqStatusCode,
};
use serde_json::{json, Value};
use serial_test::serial;

/// 画像バリエーションスタブサーバーの状態
#[derive(Clone)]
struct ImageVarStubState {
    expected_model: Option<String>,
    response: ImageVarStubResponse,
}

/// 画像バリエーションスタブのレスポンス種別
#[derive(Clone)]
#[allow(dead_code)] // Error variant prepared for future TDD GREEN phase
enum ImageVarStubResponse {
    /// 成功レスポンス（URL形式）
    SuccessUrl(Vec<String>),
    /// エラーレスポンス
    Error(StatusCode, String),
}

/// 画像バリエーションスタブサーバーを起動
async fn spawn_image_var_stub(state: ImageVarStubState) -> TestServer {
    let router = Router::new()
        .route("/v1/images/variations", post(image_var_handler))
        .route("/v1/models", get(models_handler))
        .route("/api/tags", get(tags_handler))
        .route("/api/health", post(|| async { StatusCode::OK }))
        .with_state(Arc::new(state));

    spawn_router(router).await
}

/// 画像バリエーションエンドポイントハンドラ（スタブ）
async fn image_var_handler(
    State(state): State<Arc<ImageVarStubState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut has_image = false;

    // multipartフィールドを解析
    while let Some(field) = multipart.next_field().await.ok().flatten() {
        let name = field.name().unwrap_or("").to_string();
        if name == "image" {
            has_image = true;
        }
    }

    if !has_image {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "message": "Missing required field: image",
                    "type": "invalid_request_error",
                    "code": "missing_image"
                }
            })),
        )
            .into_response();
    }

    match &state.response {
        ImageVarStubResponse::SuccessUrl(urls) => {
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
        ImageVarStubResponse::Error(status, msg) => {
            (*status, Json(json!({"error": {"message": msg}}))).into_response()
        }
    }
}

/// モデル一覧ハンドラ（スタブ）
async fn models_handler(State(state): State<Arc<ImageVarStubState>>) -> impl IntoResponse {
    let models: Vec<_> = if let Some(model) = &state.expected_model {
        vec![json!({"id": model})]
    } else {
        vec![json!({"id": "stable-diffusion-xl"})]
    };
    (StatusCode::OK, Json(json!({"data": models}))).into_response()
}

/// タグ一覧ハンドラ（スタブ）
async fn tags_handler(State(state): State<Arc<ImageVarStubState>>) -> impl IntoResponse {
    let models: Vec<_> = if let Some(model) = &state.expected_model {
        vec![json!({"name": model, "size": 5_000_000_000i64})]
    } else {
        vec![json!({"name": "stable-diffusion-xl", "size": 5_000_000_000i64})]
    };
    (StatusCode::OK, Json(json!({"models": models}))).into_response()
}

/// ダミーPNG画像データを生成（1x1ピクセル）
fn create_dummy_png() -> Vec<u8> {
    vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, // IHDR chunk length
        0x49, 0x48, 0x44, 0x52, // "IHDR"
        0x00, 0x00, 0x00, 0x01, // width: 1
        0x00, 0x00, 0x00, 0x01, // height: 1
        0x08, 0x06, // bit depth: 8, color type: RGBA
        0x00, 0x00, 0x00, // compression, filter, interlace
        0x1F, 0x15, 0xC4, 0x89, // CRC
        0x00, 0x00, 0x00, 0x0A, // IDAT chunk length
        0x49, 0x44, 0x41, 0x54, // "IDAT"
        0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, // compressed data
        0x0D, 0x0A, 0x2D, 0xB4, // CRC
        0x00, 0x00, 0x00, 0x00, // IEND chunk length
        0x49, 0x45, 0x4E, 0x44, // "IEND"
        0xAE, 0x42, 0x60, 0x82, // CRC
    ]
}

// =============================================================================
// Contract Tests
// =============================================================================

/// IV001: POST /v1/images/variations 正常系
///
/// 契約:
/// - multipart/form-data形式でリクエスト
/// - image (file) が必須
/// - レスポンスは created (timestamp) と data (array of image objects)
#[tokio::test]
#[serial]
async fn images_variations_success() {
    let stub_state = ImageVarStubState {
        expected_model: Some("stable-diffusion-xl".to_string()),
        response: ImageVarStubResponse::SuccessUrl(vec![
            "https://example.com/variation1.png".to_string()
        ]),
    };

    let stub = spawn_image_var_stub(stub_state).await;
    let router = spawn_test_router().await;

    let register_response =
        register_node_with_runtimes(router.addr(), stub.addr(), vec!["stable_diffusion"])
            .await
            .expect("register node must succeed");
    assert_eq!(register_response.status(), ReqStatusCode::CREATED);

    let client = Client::new();
    let form = Form::new()
        .part(
            "image",
            Part::bytes(create_dummy_png())
                .file_name("image.png")
                .mime_str("image/png")
                .unwrap(),
        )
        .text("model", "stable-diffusion-xl");

    let res = client
        .post(format!("http://{}/v1/images/variations", router.addr()))
        .header("x-api-key", "sk_debug")
        .multipart(form)
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

/// IV002: POST /v1/images/variations 複数バリエーション (n > 1)
#[tokio::test]
#[serial]
async fn images_variations_multiple() {
    let stub_state = ImageVarStubState {
        expected_model: Some("stable-diffusion-xl".to_string()),
        response: ImageVarStubResponse::SuccessUrl(vec![
            "https://example.com/var1.png".to_string(),
            "https://example.com/var2.png".to_string(),
            "https://example.com/var3.png".to_string(),
        ]),
    };

    let stub = spawn_image_var_stub(stub_state).await;
    let router = spawn_test_router().await;

    let register_response =
        register_node_with_runtimes(router.addr(), stub.addr(), vec!["stable_diffusion"])
            .await
            .expect("register node must succeed");
    assert_eq!(register_response.status(), ReqStatusCode::CREATED);

    let client = Client::new();
    let form = Form::new()
        .part(
            "image",
            Part::bytes(create_dummy_png())
                .file_name("image.png")
                .mime_str("image/png")
                .unwrap(),
        )
        .text("model", "stable-diffusion-xl")
        .text("n", "3");

    let res = client
        .post(format!("http://{}/v1/images/variations", router.addr()))
        .header("x-api-key", "sk_debug")
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), ReqStatusCode::OK);

    let body: Value = res.json().await.unwrap();
    let data = body["data"].as_array().unwrap();
    assert_eq!(data.len(), 3);
}

/// IV003: POST /v1/images/variations 画像ファイル欠落
#[tokio::test]
#[serial]
async fn images_variations_missing_image() {
    let stub_state = ImageVarStubState {
        expected_model: None,
        response: ImageVarStubResponse::SuccessUrl(vec![]),
    };

    let stub = spawn_image_var_stub(stub_state).await;
    let router = spawn_test_router().await;

    let register_response =
        register_node_with_runtimes(router.addr(), stub.addr(), vec!["stable_diffusion"])
            .await
            .expect("register node must succeed");
    assert_eq!(register_response.status(), ReqStatusCode::CREATED);

    let client = Client::new();
    let form = Form::new().text("model", "stable-diffusion-xl");
    // image missing

    let res = client
        .post(format!("http://{}/v1/images/variations", router.addr()))
        .header("x-api-key", "sk_debug")
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), ReqStatusCode::BAD_REQUEST);

    let body: Value = res.json().await.unwrap();
    assert!(body.get("error").is_some());
}

/// IV004: POST /v1/images/variations 認証なし
#[tokio::test]
#[serial]
async fn images_variations_unauthorized() {
    let router = spawn_test_router().await;

    let client = Client::new();
    let form = Form::new()
        .part(
            "image",
            Part::bytes(create_dummy_png())
                .file_name("image.png")
                .mime_str("image/png")
                .unwrap(),
        )
        .text("model", "stable-diffusion-xl");

    let res = client
        .post(format!("http://{}/v1/images/variations", router.addr()))
        // No Authorization header
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), ReqStatusCode::UNAUTHORIZED);
}

/// IV005: POST /v1/images/variations 利用可能なノードなし
#[tokio::test]
#[serial]
async fn images_variations_no_node_available() {
    let router = spawn_test_router().await;

    let client = Client::new();
    let form = Form::new()
        .part(
            "image",
            Part::bytes(create_dummy_png())
                .file_name("image.png")
                .mime_str("image/png")
                .unwrap(),
        )
        .text("model", "stable-diffusion-xl");

    let res = client
        .post(format!("http://{}/v1/images/variations", router.addr()))
        .header("x-api-key", "sk_debug")
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), ReqStatusCode::SERVICE_UNAVAILABLE);
}

/// IV006: POST /v1/images/variations サイズ指定
#[tokio::test]
#[serial]
async fn images_variations_with_size() {
    let stub_state = ImageVarStubState {
        expected_model: Some("stable-diffusion-xl".to_string()),
        response: ImageVarStubResponse::SuccessUrl(vec![
            "https://example.com/variation.png".to_string()
        ]),
    };

    let stub = spawn_image_var_stub(stub_state).await;
    let router = spawn_test_router().await;

    let register_response =
        register_node_with_runtimes(router.addr(), stub.addr(), vec!["stable_diffusion"])
            .await
            .expect("register node must succeed");
    assert_eq!(register_response.status(), ReqStatusCode::CREATED);

    let client = Client::new();
    let form = Form::new()
        .part(
            "image",
            Part::bytes(create_dummy_png())
                .file_name("image.png")
                .mime_str("image/png")
                .unwrap(),
        )
        .text("model", "stable-diffusion-xl")
        .text("size", "512x512");

    let res = client
        .post(format!("http://{}/v1/images/variations", router.addr()))
        .header("x-api-key", "sk_debug")
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), ReqStatusCode::OK);
}
