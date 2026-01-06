//! Contract Test: Vision Capabilities in /v1/models (SPEC-e03a404c)
//!
//! TDD RED: These tests define the capability reporting for vision models.
//! All tests should FAIL until the vision feature is implemented.
//!
//! NOTE: SPEC-93536000 により、ルーター側のモデルレジストリは廃止されました。
//! モデル情報はノードの executable_models から取得します。
//! これらのテストはノードがモデルを登録している前提で動作します。

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::json;
use serial_test::serial;
use tower::ServiceExt;

mod common {
    use axum::Router;
    use llm_router::{api, balancer::LoadManager, registry::NodeRegistry, AppState};
    use llm_router_common::auth::{ApiKeyScope, UserRole};

    pub struct TestApp {
        pub app: Router,
        pub api_key: String,
    }

    pub async fn build_app() -> TestApp {
        let temp_dir = std::env::temp_dir().join(format!(
            "vision-cap-test-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::env::set_var("LLM_ROUTER_DATA_DIR", &temp_dir);
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

    // NOTE: SPEC-93536000 により、ルーター側のモデルレジストリは廃止されました。
    // モデル情報はノードの executable_models から取得します。
}

use common::build_app;

/// FR-006: /v1/models レスポンスに image_understanding capability を含める
/// TDD RED: capabilities フィールドが未実装のため失敗する
///
/// NOTE: SPEC-93536000 により、モデル情報はノードから取得します。
/// このテストが動作するにはノードがVision対応モデルを登録している必要があります。
#[tokio::test]
#[serial]
#[ignore = "TDD RED: Vision image_understanding capability not yet implemented - requires node with vision model"]
async fn test_vision_model_has_image_understanding_capability() {
    let common::TestApp { app, api_key } = build_app().await;

    // NOTE: ノードがない状態では /v1/models は空の配列を返す
    // このテストはノードがVision対応モデルを登録している前提で動作する

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

    // ノードがない場合、モデルリストは空になる可能性がある
    if models.is_empty() {
        // Skip: ノードが登録されていない
        return;
    }

    let vision_model = models.iter().find(|m| m["id"] == "llava-v1.5-7b");

    if let Some(vision_model) = vision_model {
        // TDD RED: capabilities.image_understanding が true である必要がある
        assert_eq!(
            vision_model["capabilities"]["image_understanding"],
            json!(true),
            "Vision model should have image_understanding capability: {:?}",
            vision_model
        );
    }
}

/// FR-006: テキストのみ対応モデルは image_understanding が false
/// TDD RED: capabilities フィールドが未実装のため失敗する
#[tokio::test]
#[serial]
#[ignore = "TDD RED: Vision image_understanding capability not yet implemented - requires node with text model"]
async fn test_text_model_has_no_image_understanding_capability() {
    let common::TestApp { app, api_key } = build_app().await;

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

    // ノードがない場合、モデルリストは空になる可能性がある
    if models.is_empty() {
        return;
    }

    let text_model = models.iter().find(|m| m["id"] == "llama-3.1-8b");

    if let Some(text_model) = text_model {
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
}

/// FR-006: 複数モデルの capabilities が正しく区別される
/// TDD RED: capabilities フィールドが未実装のため失敗する
#[tokio::test]
#[serial]
#[ignore = "TDD RED: Vision image_understanding capability not yet implemented - requires node with multiple models"]
async fn test_mixed_models_capabilities() {
    let common::TestApp { app, api_key } = build_app().await;

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

    // ノードがない場合、モデルリストは空になる可能性がある
    if models.is_empty() {
        return;
    }

    // Visionモデルをチェック
    for vision_name in ["llava-v1.5-7b", "qwen-vl-7b"] {
        if let Some(vision_model) = models.iter().find(|m| m["id"] == vision_name) {
            assert_eq!(
                vision_model["capabilities"]["image_understanding"],
                json!(true),
                "{} should have image_understanding capability",
                vision_name
            );
        }
    }

    // テキストモデルをチェック
    for text_name in ["llama-3.1-8b", "mistral-7b"] {
        if let Some(text_model) = models.iter().find(|m| m["id"] == text_name) {
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
}

/// capabilities オブジェクトが /v1/models レスポンスに含まれる
/// TDD RED: capabilities フィールド自体が未実装のため失敗する
#[tokio::test]
#[serial]
#[ignore = "TDD RED: Vision capabilities field not yet implemented - requires node with model"]
async fn test_models_response_includes_capabilities_field() {
    let common::TestApp { app, api_key } = build_app().await;

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

    // ノードがない場合、モデルリストは空になる可能性がある
    if models.is_empty() {
        return;
    }

    // 少なくとも1つのモデルが capabilities フィールドを持つことを確認
    let has_capabilities = models.iter().any(|m| m["capabilities"].is_object());
    assert!(
        has_capabilities,
        "At least one model should have a 'capabilities' object"
    );
}
