//! Contract Test: POST /v1/images/edits (Image Editing / Inpainting)
//!
//! OpenAI互換の画像編集APIの契約テスト。
//! TDD Red Phase: エンドポイント実装前のテスト定義

use std::sync::Arc;

use crate::support::{
    http::{spawn_lb, TestServer},
    lb::{register_image_generation_endpoint, spawn_test_lb},
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

/// 画像編集スタブサーバーの状態
#[derive(Clone)]
struct ImageEditStubState {
    expected_model: Option<String>,
    response: ImageEditStubResponse,
}

/// 画像編集スタブのレスポンス種別
#[derive(Clone)]
#[allow(dead_code)] // Error variant prepared for future TDD GREEN phase
enum ImageEditStubResponse {
    /// 成功レスポンス（URL形式）
    SuccessUrl(Vec<String>),
    /// エラーレスポンス
    Error(StatusCode, String),
}

/// 画像編集スタブサーバーを起動
async fn spawn_image_edit_stub(state: ImageEditStubState) -> TestServer {
    let app = Router::new()
        .route("/v1/images/edits", post(image_edit_handler))
        .route("/v1/models", get(models_handler))
        .route("/api/tags", get(tags_handler))
        .route("/api/health", post(|| async { StatusCode::OK }))
        .with_state(Arc::new(state));

    spawn_lb(app).await
}

/// 画像編集エンドポイントハンドラ（スタブ）
async fn image_edit_handler(
    State(state): State<Arc<ImageEditStubState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut has_image = false;
    let mut has_prompt = false;

    // multipartフィールドを解析
    while let Some(field) = multipart.next_field().await.ok().flatten() {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "image" => has_image = true,
            "prompt" => has_prompt = true,
            _ => {}
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

    if !has_prompt {
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
        ImageEditStubResponse::SuccessUrl(urls) => {
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
        ImageEditStubResponse::Error(status, msg) => {
            (*status, Json(json!({"error": {"message": msg}}))).into_response()
        }
    }
}

/// モデル一覧ハンドラ（スタブ）
async fn models_handler(State(state): State<Arc<ImageEditStubState>>) -> impl IntoResponse {
    let models: Vec<_> = if let Some(model) = &state.expected_model {
        vec![json!({"id": model})]
    } else {
        vec![json!({"id": "stable-diffusion-xl"})]
    };
    (StatusCode::OK, Json(json!({"data": models}))).into_response()
}

/// タグ一覧ハンドラ（スタブ）
async fn tags_handler(State(state): State<Arc<ImageEditStubState>>) -> impl IntoResponse {
    let models: Vec<_> = if let Some(model) = &state.expected_model {
        vec![json!({"name": model, "size": 5_000_000_000i64})]
    } else {
        vec![json!({"name": "stable-diffusion-xl", "size": 5_000_000_000i64})]
    };
    (StatusCode::OK, Json(json!({"models": models}))).into_response()
}

/// ダミーPNG画像データを生成（1x1ピクセル）
fn create_dummy_png() -> Vec<u8> {
    // 最小の有効なPNGファイル（1x1透明ピクセル）
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

/// IE001: POST /v1/images/edits 正常系
///
/// 契約:
/// - multipart/form-data形式でリクエスト
/// - image (file) と prompt (string) が必須
/// - mask (file) はオプション
/// - レスポンスは created (timestamp) と data (array of image objects)
#[tokio::test]
#[serial]
async fn images_edits_success() {
    let stub_state = ImageEditStubState {
        expected_model: Some("stable-diffusion-xl".to_string()),
        response: ImageEditStubResponse::SuccessUrl(vec![
            "https://example.com/edited-image.png".to_string()
        ]),
    };

    let stub = spawn_image_edit_stub(stub_state).await;
    let lb = spawn_test_lb().await;

    // EndpointRegistry経由で画像生成エンドポイントを登録
    let _endpoint_id = register_image_generation_endpoint(lb.addr(), stub.addr())
        .await
        .expect("register endpoint must succeed");

    let client = Client::new();
    let form = Form::new()
        .part(
            "image",
            Part::bytes(create_dummy_png())
                .file_name("image.png")
                .mime_str("image/png")
                .unwrap(),
        )
        .text("prompt", "A sunlit indoor lounge area")
        .text("model", "stable-diffusion-xl");

    let res = client
        .post(format!("http://{}/v1/images/edits", lb.addr()))
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

/// IE002: POST /v1/images/edits マスク付き
#[tokio::test]
#[serial]
async fn images_edits_with_mask() {
    let stub_state = ImageEditStubState {
        expected_model: Some("stable-diffusion-xl".to_string()),
        response: ImageEditStubResponse::SuccessUrl(vec![
            "https://example.com/edited-image.png".to_string()
        ]),
    };

    let stub = spawn_image_edit_stub(stub_state).await;
    let lb = spawn_test_lb().await;

    // EndpointRegistry経由で画像生成エンドポイントを登録
    let _endpoint_id = register_image_generation_endpoint(lb.addr(), stub.addr())
        .await
        .expect("register endpoint must succeed");

    let client = Client::new();
    let form = Form::new()
        .part(
            "image",
            Part::bytes(create_dummy_png())
                .file_name("image.png")
                .mime_str("image/png")
                .unwrap(),
        )
        .part(
            "mask",
            Part::bytes(create_dummy_png())
                .file_name("mask.png")
                .mime_str("image/png")
                .unwrap(),
        )
        .text("prompt", "A sunlit indoor lounge area")
        .text("model", "stable-diffusion-xl");

    let res = client
        .post(format!("http://{}/v1/images/edits", lb.addr()))
        .header("x-api-key", "sk_debug")
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), ReqStatusCode::OK);
}

/// IE003: POST /v1/images/edits 画像ファイル欠落
#[tokio::test]
#[serial]
async fn images_edits_missing_image() {
    let stub_state = ImageEditStubState {
        expected_model: None,
        response: ImageEditStubResponse::SuccessUrl(vec![]),
    };

    let stub = spawn_image_edit_stub(stub_state).await;
    let lb = spawn_test_lb().await;

    // EndpointRegistry経由で画像生成エンドポイントを登録
    let _endpoint_id = register_image_generation_endpoint(lb.addr(), stub.addr())
        .await
        .expect("register endpoint must succeed");

    let client = Client::new();
    let form = Form::new()
        .text("prompt", "A sunlit indoor lounge area")
        .text("model", "stable-diffusion-xl");
    // image missing

    let res = client
        .post(format!("http://{}/v1/images/edits", lb.addr()))
        .header("x-api-key", "sk_debug")
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), ReqStatusCode::BAD_REQUEST);

    let body: Value = res.json().await.unwrap();
    assert!(body.get("error").is_some());
}

/// IE004: POST /v1/images/edits プロンプト欠落
#[tokio::test]
#[serial]
async fn images_edits_missing_prompt() {
    let stub_state = ImageEditStubState {
        expected_model: None,
        response: ImageEditStubResponse::SuccessUrl(vec![]),
    };

    let stub = spawn_image_edit_stub(stub_state).await;
    let lb = spawn_test_lb().await;

    // EndpointRegistry経由で画像生成エンドポイントを登録
    let _endpoint_id = register_image_generation_endpoint(lb.addr(), stub.addr())
        .await
        .expect("register endpoint must succeed");

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
    // prompt missing

    let res = client
        .post(format!("http://{}/v1/images/edits", lb.addr()))
        .header("x-api-key", "sk_debug")
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), ReqStatusCode::BAD_REQUEST);
}

/// IE005: POST /v1/images/edits 認証なし
#[tokio::test]
#[serial]
async fn images_edits_unauthorized() {
    let lb = spawn_test_lb().await;

    let client = Client::new();
    let form = Form::new()
        .part(
            "image",
            Part::bytes(create_dummy_png())
                .file_name("image.png")
                .mime_str("image/png")
                .unwrap(),
        )
        .text("prompt", "A cat")
        .text("model", "stable-diffusion-xl");

    let res = client
        .post(format!("http://{}/v1/images/edits", lb.addr()))
        // No Authorization header
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), ReqStatusCode::UNAUTHORIZED);
}

/// IE006: POST /v1/images/edits 利用可能なノードなし
#[tokio::test]
#[serial]
async fn images_edits_no_node_available() {
    let lb = spawn_test_lb().await;

    let client = Client::new();
    let form = Form::new()
        .part(
            "image",
            Part::bytes(create_dummy_png())
                .file_name("image.png")
                .mime_str("image/png")
                .unwrap(),
        )
        .text("prompt", "A cat")
        .text("model", "stable-diffusion-xl");

    let res = client
        .post(format!("http://{}/v1/images/edits", lb.addr()))
        .header("x-api-key", "sk_debug")
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), ReqStatusCode::SERVICE_UNAVAILABLE);
}
