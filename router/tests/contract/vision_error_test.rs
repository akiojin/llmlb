//! Contract Test: Vision Error Handling (SPEC-e03a404c)
//!
//! TDD RED: These tests define error handling behavior for vision API.
//! All tests should FAIL until the vision feature is implemented.

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::json;
use serial_test::serial;
use tower::ServiceExt;

mod common {
    use axum::Router;
    use llm_router::registry::models::ModelInfo;
    use llm_router::{api, balancer::LoadManager, registry::NodeRegistry, AppState};
    use llm_router_common::auth::{ApiKeyScope, UserRole};

    pub struct TestApp {
        pub app: Router,
        pub api_key: String,
        #[allow(dead_code)]
        pub db_pool: sqlx::SqlitePool,
    }

    pub async fn build_app() -> TestApp {
        let temp_dir = std::env::temp_dir().join(format!(
            "vision-error-test-{}-{}",
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
        let convert_manager = llm_router::convert::ConvertTaskManager::new(1, db_pool.clone());
        let jwt_secret = "test-secret".to_string();
        let state = AppState {
            registry,
            load_manager,
            request_history,
            convert_manager,
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
        TestApp {
            app,
            api_key,
            db_pool,
        }
    }

    /// テスト用のテキストのみ対応モデルを登録する
    pub fn register_text_only_model(name: &str) {
        let model = ModelInfo::new(name.to_string(), 0, "test".to_string(), 0, vec![]);
        // Vision capabilityを持たないモデルとして登録
        // capabilities は空のまま (image_understandingなし)
        llm_router::api::models::upsert_registered_model(model);
    }
}

use common::{build_app, register_text_only_model};

/// FR-004: Vision非対応モデルへの画像付きリクエストを400エラーで拒否
/// TDD RED: capabilities検証が未実装のため失敗する
#[tokio::test]
#[serial]
#[ignore = "TDD RED: Vision capability validation not yet implemented"]
async fn test_image_request_to_non_vision_model_returns_400() {
    let common::TestApp { app, api_key, .. } = build_app().await;

    // Vision非対応のテキストモデルを登録
    register_text_only_model("llama-3.1-8b");

    let request_body = json!({
        "model": "llama-3.1-8b",
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

    // TDD RED: Vision非対応モデルへの画像リクエストは400で拒否される必要がある
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "Image request to non-vision model should return 400 (actual: {})",
        response.status()
    );

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap_or(json!({}));

    // エラーメッセージに「image understanding」または「vision」が含まれる
    let error_msg = body["error"]["message"]
        .as_str()
        .unwrap_or("")
        .to_lowercase();
    assert!(
        error_msg.contains("image") || error_msg.contains("vision"),
        "Error message should mention image/vision capability: {:?}",
        body
    );
}

/// FR-008: 画像サイズ制限 (最大10MB) を超えた場合のエラー
/// TDD RED: サイズ検証が未実装のため失敗する
#[tokio::test]
#[serial]
#[ignore = "TDD RED: Vision image size validation not yet implemented"]
async fn test_image_size_limit_exceeded() {
    let common::TestApp { app, api_key, .. } = build_app().await;

    // 10MBを超えるサイズを指定（実際のデータではなくヘッダーで判断される想定）
    // テスト用に大きなBase64文字列を生成（約15MB相当）
    let large_base64 = "A".repeat(15 * 1024 * 1024);

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
                            "url": format!("data:image/png;base64,{}", large_base64)
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

    // TDD RED: 10MBを超える画像は400で拒否される必要がある
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "Oversized image should be rejected with 400 (actual: {})",
        response.status()
    );

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap_or(json!({}));

    let error_msg = body["error"]["message"]
        .as_str()
        .unwrap_or("")
        .to_lowercase();
    assert!(
        error_msg.contains("size") || error_msg.contains("10mb") || error_msg.contains("limit"),
        "Error message should mention size limit: {:?}",
        body
    );
}

/// FR-009: 1リクエストあたりの画像枚数制限 (最大10枚)
/// TDD RED: 枚数制限検証が未実装のため失敗する
#[tokio::test]
#[serial]
#[ignore = "TDD RED: Vision image count validation not yet implemented"]
async fn test_image_count_limit_exceeded() {
    let common::TestApp { app, api_key, .. } = build_app().await;

    // 11枚の画像を含むリクエストを作成
    let mut content = vec![json!({
        "type": "text",
        "text": "Describe all these images"
    })];

    for i in 0..11 {
        content.push(json!({
            "type": "image_url",
            "image_url": {
                "url": format!("https://example.com/image{}.jpg", i)
            }
        }));
    }

    let request_body = json!({
        "model": "llava-v1.5-7b",
        "messages": [
            {
                "role": "user",
                "content": content
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

    // TDD RED: 10枚を超える画像は400で拒否される必要がある
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "More than 10 images should be rejected with 400 (actual: {})",
        response.status()
    );

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap_or(json!({}));

    let error_msg = body["error"]["message"]
        .as_str()
        .unwrap_or("")
        .to_lowercase();
    assert!(
        error_msg.contains("10") || error_msg.contains("limit") || error_msg.contains("maximum"),
        "Error message should mention image count limit: {:?}",
        body
    );
}

/// エッジケース: 不正なBase64エンコード
/// TDD RED: Base64検証が未実装のため失敗する
#[tokio::test]
#[serial]
#[ignore = "TDD RED: Vision base64 validation not yet implemented"]
async fn test_invalid_base64_encoding() {
    let common::TestApp { app, api_key, .. } = build_app().await;

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
                            "url": "data:image/png;base64,!!!INVALID_BASE64!!!"
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

    // TDD RED: 不正なBase64は400で拒否される必要がある
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "Invalid base64 should be rejected with 400 (actual: {})",
        response.status()
    );

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap_or(json!({}));

    let error_msg = body["error"]["message"]
        .as_str()
        .unwrap_or("")
        .to_lowercase();
    assert!(
        error_msg.contains("base64")
            || error_msg.contains("decode")
            || error_msg.contains("invalid"),
        "Error message should mention base64 decoding error: {:?}",
        body
    );
}

/// エッジケース: サポートされていない画像形式 (TIFF)
/// TDD RED: 形式検証が未実装のため失敗する
#[tokio::test]
#[serial]
#[ignore = "TDD RED: Vision image format validation not yet implemented"]
async fn test_unsupported_image_format() {
    let common::TestApp { app, api_key, .. } = build_app().await;

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
                            "url": "data:image/tiff;base64,AAAA"
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

    // TDD RED: サポートされていない形式は400で拒否される必要がある
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "Unsupported format (TIFF) should be rejected with 400 (actual: {})",
        response.status()
    );

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap_or(json!({}));

    let error_msg = body["error"]["message"]
        .as_str()
        .unwrap_or("")
        .to_lowercase();
    assert!(
        error_msg.contains("format")
            || error_msg.contains("tiff")
            || error_msg.contains("supported"),
        "Error message should mention unsupported format: {:?}",
        body
    );
}
