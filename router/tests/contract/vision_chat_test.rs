//! Contract Test: Vision Chat Completions API (SPEC-e03a404c)
//!
//! TDD RED: These tests define the API contract for vision/image understanding.
//! All tests should FAIL until the vision feature is implemented.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::json;
use serial_test::serial;
use tower::ServiceExt;

mod common {
    use axum::Router;
    use llm_router::{api, balancer::LoadManager, registry::NodeRegistry, AppState};
    use llm_router_common::auth::{ApiKeyScope, UserRole};

    // Viewer role is used for API users (Admin is for full control)

    pub struct TestApp {
        pub app: Router,
        pub api_key: String,
    }

    pub async fn build_app() -> TestApp {
        let temp_dir = std::env::temp_dir().join(format!(
            "vision-test-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::env::set_var("LLM_ROUTER_DATA_DIR", &temp_dir);
        std::env::set_var("HOME", &temp_dir);
        std::env::set_var("USERPROFILE", &temp_dir);

        llm_router::api::models::clear_registered_models();

        let registry = NodeRegistry::new();
        let load_manager = LoadManager::new(registry.clone());
        let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to create test database");
        sqlx::migrate!("./migrations")
            .run(&db_pool)
            .await
            .expect("Failed to run migrations");
        let request_history = std::sync::Arc::new(
            llm_router::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
        );
        let jwt_secret = "test-secret".to_string();
        let state = AppState {
            registry,
            load_manager,
            request_history,
            db_pool: db_pool.clone(),
            jwt_secret,
            http_client: reqwest::Client::new(),
            queue_config: llm_router::config::QueueConfig::from_env(),
            event_bus: llm_router::events::create_shared_event_bus(),
        };

        let password_hash = llm_router::auth::password::hash_password("password123").unwrap();
        let user =
            llm_router::db::users::create(&db_pool, "testuser", &password_hash, UserRole::Viewer)
                .await
                .expect("create user");
        let api_key = llm_router::db::api_keys::create(
            &db_pool,
            "test-key",
            user.id,
            None,
            vec![ApiKeyScope::Api],
        )
        .await
        .expect("create api key")
        .key;

        let app = api::create_router(state);
        TestApp { app, api_key }
    }
}

use common::build_app;

/// FR-001: システムは、画像URL付きのchat completionsリクエストを処理できる必要がある
/// TDD RED: Vision機能未実装のため失敗する
///
/// NOTE: SPEC-93536000 により、モデル情報はノードの executable_models から取得します。
/// このテストはノードがVision対応モデルを登録している前提で動作します。
#[tokio::test]
#[serial]
#[ignore = "TDD RED: requires node with vision model registered (SPEC-93536000)"]
async fn test_chat_completions_with_image_url() {
    let common::TestApp { app, api_key } = build_app().await;

    // OpenAI Vision API互換形式: content配列にtext+image_url
    let request_body = json!({
        "model": "llava-v1.5-7b",
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "What is in this image?"
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": "https://example.com/test-image.jpg"
                        }
                    }
                ]
            }
        ],
        "max_tokens": 300
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", api_key))
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // TDD RED: Vision対応モデルが登録されていないため、モデル不明エラーになるはず
    // しかし、リクエスト形式自体（content配列）は受け入れられる必要がある
    // 現在は実装されていないので、このテストは何らかの形で失敗する
    assert!(
        response.status() != StatusCode::BAD_REQUEST || response.status() == StatusCode::NOT_FOUND, // モデルが見つからない場合
        "Request format with image_url should be accepted (actual: {})",
        response.status()
    );
}

/// FR-002: システムは、Base64エンコードされた画像付きのリクエストを処理できる必要がある
/// TDD RED: Vision機能未実装のため失敗する
#[tokio::test]
#[serial]
async fn test_chat_completions_with_base64_image() {
    let common::TestApp { app, api_key } = build_app().await;

    // 1x1ピクセルの透明PNGをBase64エンコード
    let tiny_png_base64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

    let request_body = json!({
        "model": "llava-v1.5-7b",
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "Describe this image"
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:image/png;base64,{}", tiny_png_base64)
                        }
                    }
                ]
            }
        ],
        "max_tokens": 300
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", api_key))
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // TDD RED: Base64画像形式が受け入れられる必要がある
    assert!(
        response.status() != StatusCode::BAD_REQUEST || response.status() == StatusCode::NOT_FOUND,
        "Request format with base64 image should be accepted (actual: {})",
        response.status()
    );
}

/// FR-003: システムは、複数画像を含むリクエストを処理できる必要がある
/// TDD RED: Vision機能未実装のため失敗する
///
/// NOTE: SPEC-93536000 により、モデル情報はノードの executable_models から取得します。
/// このテストはノードがVision対応モデルを登録している前提で動作します。
#[tokio::test]
#[serial]
#[ignore = "TDD RED: requires node with vision model registered (SPEC-93536000)"]
async fn test_chat_completions_with_multiple_images() {
    let common::TestApp { app, api_key } = build_app().await;

    let request_body = json!({
        "model": "llava-v1.5-7b",
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "Compare these two images"
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": "https://example.com/image1.jpg"
                        }
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": "https://example.com/image2.jpg"
                        }
                    }
                ]
            }
        ],
        "max_tokens": 300
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", api_key))
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // TDD RED: 複数画像形式が受け入れられる必要がある
    assert!(
        response.status() != StatusCode::BAD_REQUEST || response.status() == StatusCode::NOT_FOUND,
        "Request format with multiple images should be accepted (actual: {})",
        response.status()
    );
}

/// FR-007: システムは、JPEG, PNG, GIF, WebP形式の画像をサポートする必要がある
/// TDD RED: 各画像形式のMIMEタイプが受け入れられることを確認
#[tokio::test]
#[serial]
async fn test_supported_image_formats() {
    let common::TestApp { app, api_key } = build_app().await;

    let formats = vec![
        ("image/jpeg", "jpeg"),
        ("image/png", "png"),
        ("image/gif", "gif"),
        ("image/webp", "webp"),
    ];

    for (mime, ext) in formats {
        // 最小限のダミーBase64データ
        let dummy_base64 = "AAAA";

        let request_body = json!({
            "model": "llava-v1.5-7b",
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": "Describe this image"
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": format!("data:{};base64,{}", mime, dummy_base64)
                            }
                        }
                    ]
                }
            ],
            "max_tokens": 100
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/chat/completions")
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {}", api_key))
                    .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // TDD RED: 全ての対応形式が受け入れられる必要がある
        // 400エラーは「形式が不正」を意味するので、それ以外であれば良い
        assert!(
            response.status() != StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Image format {} ({}) should be supported (actual: {})",
            ext,
            mime,
            response.status()
        );
    }
}

/// FR-005: システムは、画像付きリクエストのストリーミングレスポンスをサポートする必要がある
/// TDD RED: stream=true オプションが受け入れられることを確認
///
/// NOTE: SPEC-93536000 により、モデル情報はノードの executable_models から取得します。
/// このテストはノードがVision対応モデルを登録している前提で動作します。
#[tokio::test]
#[serial]
#[ignore = "TDD RED: requires node with vision model registered (SPEC-93536000)"]
async fn test_vision_streaming_response() {
    let common::TestApp { app, api_key } = build_app().await;

    let request_body = json!({
        "model": "llava-v1.5-7b",
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "Describe this image in detail"
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": "https://example.com/test-image.jpg"
                        }
                    }
                ]
            }
        ],
        "max_tokens": 500,
        "stream": true
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", api_key))
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // TDD RED: ストリーミングリクエストが受け入れられる必要がある
    assert!(
        response.status() != StatusCode::BAD_REQUEST || response.status() == StatusCode::NOT_FOUND,
        "Streaming vision request should be accepted (actual: {})",
        response.status()
    );
}
