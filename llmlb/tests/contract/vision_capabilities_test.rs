//! Contract Test: Vision Capabilities in /v1/models (SPEC-e03a404c)
//!
//! TDD RED: These tests define the capability reporting for vision models.
//! All tests should FAIL until the vision feature is implemented.
//!
//! NOTE: NodeRegistry廃止（SPEC-66555000）に伴い、EndpointRegistryベースに更新済み。

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
        pub db_pool: sqlx::SqlitePool,
    }

    pub async fn build_app() -> TestApp {
        let temp_dir = std::env::temp_dir().join(format!(
            "vision-cap-test-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::env::set_var("LLMLB_DATA_DIR", &temp_dir);
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

    /// テスト用のVision対応モデルを登録する
    /// Vision capability を設定し、image_understanding が true になるようにする
    pub async fn register_vision_model(db_pool: &SqlitePool, name: &str) {
        use llmlb::common::types::ModelCapability;
        let model = ModelInfo::with_capabilities(
            name.to_string(),
            4,
            "test".to_string(),
            0,
            vec![],
            vec![ModelCapability::TextGeneration, ModelCapability::Vision],
        );
        let storage = ModelStorage::new(db_pool.clone());
        storage.save_model(&model).await.unwrap();
    }

    /// テスト用のテキストのみ対応モデルを登録する
    pub async fn register_text_only_model(db_pool: &SqlitePool, name: &str) {
        let model = ModelInfo::new(name.to_string(), 4, "test".to_string(), 0, vec![]);
        // Vision capability なし
        let storage = ModelStorage::new(db_pool.clone());
        storage.save_model(&model).await.unwrap();
    }
}

use common::{build_app, register_text_only_model, register_vision_model};

/// FR-006: /v1/models レスポンスに image_understanding capability を含める
#[tokio::test]
#[serial]
async fn test_vision_model_has_image_understanding_capability() {
    let common::TestApp {
        app,
        api_key,
        db_pool,
    } = build_app().await;

    // Vision対応モデルを登録
    register_vision_model(&db_pool, "llava-v1.5-7b").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/models")
                .header("authorization", format!("Bearer {}", api_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");

    let models = body["data"].as_array().expect("data should be an array");
    let vision_model = models
        .iter()
        .find(|m| m["id"] == "llava-v1.5-7b")
        .expect("llava-v1.5-7b should be in the list");

    // TDD RED: capabilities.image_understanding が true である必要がある
    assert_eq!(
        vision_model["capabilities"]["image_understanding"],
        json!(true),
        "Vision model should have image_understanding capability: {:?}",
        vision_model
    );
}

/// FR-006: テキストのみ対応モデルは image_understanding が false
#[tokio::test]
#[serial]
async fn test_text_model_has_no_image_understanding_capability() {
    let common::TestApp {
        app,
        api_key,
        db_pool,
    } = build_app().await;

    // テキストのみ対応モデルを登録
    register_text_only_model(&db_pool, "llama-3.1-8b").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/models")
                .header("authorization", format!("Bearer {}", api_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");

    let models = body["data"].as_array().expect("data should be an array");
    let text_model = models
        .iter()
        .find(|m| m["id"] == "llama-3.1-8b")
        .expect("llama-3.1-8b should be in the list");

    // TDD RED: capabilities.image_understanding が false または存在しない
    let has_vision = text_model["capabilities"]["image_understanding"]
        .as_bool()
        .unwrap_or(false);
    assert!(
        !has_vision,
        "Text-only model should NOT have image_understanding capability: {:?}",
        text_model
    );
}

/// FR-006: 複数モデルの capabilities が正しく区別される
#[tokio::test]
#[serial]
async fn test_mixed_models_capabilities() {
    let common::TestApp {
        app,
        api_key,
        db_pool,
    } = build_app().await;

    // 両方のモデルを登録
    register_vision_model(&db_pool, "llava-v1.5-7b").await;
    register_vision_model(&db_pool, "qwen-vl-7b").await;
    register_text_only_model(&db_pool, "llama-3.1-8b").await;
    register_text_only_model(&db_pool, "mistral-7b").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/models")
                .header("authorization", format!("Bearer {}", api_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");

    let models = body["data"].as_array().expect("data should be an array");

    // Visionモデルをチェック
    for vision_name in ["llava-v1.5-7b", "qwen-vl-7b"] {
        let vision_model = models
            .iter()
            .find(|m| m["id"] == vision_name)
            .unwrap_or_else(|| panic!("{} should be in the list", vision_name));

        assert_eq!(
            vision_model["capabilities"]["image_understanding"],
            json!(true),
            "{} should have image_understanding capability",
            vision_name
        );
    }

    // テキストモデルをチェック
    for text_name in ["llama-3.1-8b", "mistral-7b"] {
        let text_model = models
            .iter()
            .find(|m| m["id"] == text_name)
            .unwrap_or_else(|| panic!("{} should be in the list", text_name));

        let has_vision = text_model["capabilities"]["image_understanding"]
            .as_bool()
            .unwrap_or(false);
        assert!(
            !has_vision,
            "{} should NOT have image_understanding capability",
            text_name
        );
    }
}

/// capabilities オブジェクトが /v1/models レスポンスに含まれる
#[tokio::test]
#[serial]
async fn test_models_response_includes_capabilities_field() {
    let common::TestApp {
        app,
        api_key,
        db_pool,
    } = build_app().await;

    register_vision_model(&db_pool, "test-model").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/models")
                .header("authorization", format!("Bearer {}", api_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");

    let models = body["data"].as_array().expect("data should be an array");
    let model = models
        .iter()
        .find(|m| m["id"] == "test-model")
        .expect("test-model should be in the list");

    // TDD RED: capabilities フィールドがオブジェクトとして存在する必要がある
    assert!(
        model["capabilities"].is_object(),
        "Model should have a 'capabilities' object: {:?}",
        model
    );
}
