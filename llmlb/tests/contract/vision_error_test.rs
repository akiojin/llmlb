//! Contract Test: Vision Error Handling (SPEC-e03a404c)
//!
//! Vision APIのエラーハンドリング契約テスト。
//! - Vision非対応モデルへの画像リクエスト拒否
//! - 画像サイズ/枚数制限
//! - Base64/フォーマット検証

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::json;
use serial_test::serial;
use tower::ServiceExt;

mod common {
    use axum::Router;
    use llmlb::common::auth::{ApiKeyScope, UserRole};
    use llmlb::db::models::ModelStorage;
    use llmlb::registry::models::ModelInfo;
    use llmlb::{api, balancer::LoadManager, registry::endpoints::EndpointRegistry, AppState};
    use sqlx::SqlitePool;
    use std::sync::Arc;

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
        std::env::set_var("LLMLB_DATA_DIR", &temp_dir);
        std::env::set_var("HOME", &temp_dir);
        std::env::set_var("USERPROFILE", &temp_dir);

        let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to create test database");
        sqlx::migrate!("./migrations")
            .run(&db_pool)
            .await
            .expect("Failed to run migrations");
        let endpoint_registry = EndpointRegistry::new(db_pool.clone())
            .await
            .expect("Failed to create endpoint registry");
        let load_manager = LoadManager::new(Arc::new(endpoint_registry.clone()));
        llmlb::api::models::clear_registered_models(&db_pool)
            .await
            .expect("clear registered models");
        let request_history = std::sync::Arc::new(
            llmlb::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
        );
        let jwt_secret = "test-secret".to_string();
        let state = AppState {
            load_manager,
            request_history,
            db_pool: db_pool.clone(),
            jwt_secret,
            http_client: reqwest::Client::new(),
            queue_config: llmlb::config::QueueConfig::from_env(),
            event_bus: llmlb::events::create_shared_event_bus(),
            endpoint_registry,
        };

        let password_hash = llmlb::auth::password::hash_password("password123").unwrap();
        let user = llmlb::db::users::create(&db_pool, "testuser", &password_hash, UserRole::Viewer)
            .await
            .expect("create user");
        let api_key = llmlb::db::api_keys::create(
            &db_pool,
            "test-key",
            user.id,
            None,
            vec![ApiKeyScope::Api],
        )
        .await
        .expect("create api key")
        .key;

        let app = api::create_app(state);
        TestApp {
            app,
            api_key,
            db_pool,
        }
    }

    /// テスト用のテキストのみ対応モデルを登録する
    pub async fn register_text_only_model(db_pool: &SqlitePool, name: &str) {
        let model = ModelInfo::new(name.to_string(), 0, "test".to_string(), 0, vec![]);
        // Vision capabilityを持たないモデルとして登録
        // capabilities は空のまま (image_understandingなし)
        let storage = ModelStorage::new(db_pool.clone());
        storage.save_model(&model).await.unwrap();
    }

    /// テスト用のVision対応モデルを登録する
    pub async fn register_vision_model(db_pool: &SqlitePool, name: &str) {
        use llmlb::common::types::ModelCapability;
        let model = ModelInfo::with_capabilities(
            name.to_string(),
            0,
            "test".to_string(),
            0,
            vec![],
            vec![ModelCapability::Vision, ModelCapability::TextGeneration],
        );
        let storage = ModelStorage::new(db_pool.clone());
        storage.save_model(&model).await.unwrap();
    }
}

use common::{build_app, register_text_only_model, register_vision_model};

/// FR-004: Vision非対応モデルへの画像付きリクエストを400エラーで拒否
#[tokio::test]
#[serial]
async fn test_image_request_to_non_vision_model_returns_400() {
    let common::TestApp {
        app,
        api_key,
        db_pool,
    } = build_app().await;

    // Vision非対応のテキストモデルを登録
    register_text_only_model(&db_pool, "llama-3.1-8b").await;

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
/// 注: 非常に大きな画像（15MB Base64）はAxumのペイロード制限により413が返される場合がある
#[tokio::test]
#[serial]
async fn test_image_size_limit_exceeded() {
    let common::TestApp {
        app,
        api_key,
        db_pool,
    } = build_app().await;

    // Vision対応モデルを登録
    register_vision_model(&db_pool, "llava-v1.5-7b").await;

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

    // 400 (アプリケーションレベルのサイズ制限) または
    // 413 (Axumのペイロードサイズ制限) のいずれかを受け入れる
    let status = response.status();
    assert!(
        status == StatusCode::BAD_REQUEST || status == StatusCode::PAYLOAD_TOO_LARGE,
        "Oversized image should be rejected with 400 or 413 (actual: {})",
        status
    );
}

/// FR-009: 1リクエストあたりの画像枚数制限 (最大10枚)
#[tokio::test]
#[serial]
async fn test_image_count_limit_exceeded() {
    let common::TestApp {
        app,
        api_key,
        db_pool,
    } = build_app().await;

    // Vision対応モデルを登録
    register_vision_model(&db_pool, "llava-v1.5-7b").await;

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
#[tokio::test]
#[serial]
async fn test_invalid_base64_encoding() {
    let common::TestApp {
        app,
        api_key,
        db_pool,
    } = build_app().await;

    // Vision対応モデルを登録
    register_vision_model(&db_pool, "llava-v1.5-7b").await;

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
#[tokio::test]
#[serial]
async fn test_unsupported_image_format() {
    let common::TestApp {
        app,
        api_key,
        db_pool,
    } = build_app().await;

    // Vision対応モデルを登録
    register_vision_model(&db_pool, "llava-v1.5-7b").await;

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
