//! モデル情報表示統合テスト
//!
//! TDD RED: モデル一覧とエンドポイント別インストール状況の表示
//!
//! NOTE: NodeRegistry廃止（SPEC-66555000）に伴い、EndpointRegistryベースに更新済み。

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use llmlb::{api, balancer::LoadManager, registry::endpoints::EndpointRegistry, AppState};
use llmlb_common::auth::{ApiKeyScope, UserRole};
use std::sync::Arc;
use tower::ServiceExt;

async fn build_app() -> (Router, String, sqlx::SqlitePool) {
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
    let admin_user =
        llmlb::db::users::create(&state.db_pool, "admin", &password_hash, UserRole::Admin)
            .await
            .expect("create admin user");
    let admin_key = llmlb::db::api_keys::create(
        &state.db_pool,
        "admin-key",
        admin_user.id,
        None,
        vec![ApiKeyScope::Admin],
    )
    .await
    .expect("create admin api key")
    .key;

    (api::create_router(state), admin_key, db_pool)
}

/// T018: /v0/models/available は廃止され、/v0/models に統合
/// NOTE: HuggingFaceカタログ参照は廃止。登録済みモデル一覧は /v0/models で取得
#[tokio::test]
async fn test_available_models_endpoint_is_removed() {
    let (app, admin_key, _db_pool) = build_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/models/available")
                .header("authorization", format!("Bearer {}", admin_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // エンドポイントは削除済み
    // NOTE: 405 (Method Not Allowed) は /v0/models/*model_name (DELETE用) にマッチするため
    assert!(
        response.status() == StatusCode::NOT_FOUND
            || response.status() == StatusCode::METHOD_NOT_ALLOWED,
        "/v0/models/available GET endpoint should be removed (got {})",
        response.status()
    );
}

// NOTE: T019 (test_list_installed_models_on_endpoint) はSPEC-66555000（NodeRegistry廃止）により削除。
// EndpointRegistryベースのモデル同期テストは router/tests/contract/endpoints_sync_test.rs で実装済み。

/// T020: 複数エンドポイントのロード済みモデルの反映テスト
#[tokio::test]
#[ignore = "TODO: Requires multiple mock servers for proper health check testing"]
async fn test_model_matrix_view_multiple_endpoints() {
    let (_app, _admin_key, _db_pool) = build_app().await;
    // TODO: EndpointRegistryベースのテストを実装
}

/// T021: /v1/models は対応モデル一覧を返す（APIキー認証必須）
#[tokio::test]
async fn test_v1_models_returns_fixed_list() {
    // テスト用のDBを作成
    let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run migrations");

    // テストユーザーとAPIキーを作成
    let password_hash = llmlb::auth::password::hash_password("testpassword").unwrap();
    let test_user = llmlb::db::users::create(
        &db_pool,
        "test-admin",
        &password_hash,
        llmlb_common::auth::UserRole::Admin,
    )
    .await
    .expect("Failed to create test user");
    let api_key = llmlb::db::api_keys::create(
        &db_pool,
        "test-key",
        test_user.id,
        None,
        vec![llmlb_common::auth::ApiKeyScope::Api],
    )
    .await
    .expect("Failed to create test API key");

    let endpoint_registry = EndpointRegistry::new(db_pool.clone())
        .await
        .expect("Failed to create endpoint registry");
    let load_manager = LoadManager::new(Arc::new(endpoint_registry.clone()));
    let request_history = std::sync::Arc::new(
        llmlb::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let jwt_secret = "test-secret".to_string();
    let state = AppState {
        load_manager,
        request_history,
        db_pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
        queue_config: llmlb::config::QueueConfig::from_env(),
        event_bus: llmlb::events::create_shared_event_bus(),
        endpoint_registry,
    };

    let app = api::create_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/models")
                .header("Authorization", format!("Bearer {}", api_key.key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let data = json["data"]
        .as_array()
        .expect("data must be an array of models");

    // ローカルモデルのみをフィルタ（クラウドプロバイダープレフィックスを除外）
    // SPEC-82491000でクラウドモデルが追加されたため、ローカルモデルのみを検証
    let cloud_prefixes = ["openai:", "google:", "anthropic:"];
    let local_ids: Vec<String> = data
        .iter()
        .filter_map(|m| m.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .filter(|id| !cloud_prefixes.iter().any(|prefix| id.starts_with(prefix)))
        .collect();

    let expected: Vec<String> = vec![];

    assert_eq!(
        local_ids.len(),
        expected.len(),
        "should return only downloaded local models (cloud models are filtered out)"
    );
}
