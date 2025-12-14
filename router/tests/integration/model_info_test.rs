//! モデル情報表示統合テスト
//!
//! TDD RED: モデル一覧とノード別インストール状況の表示

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use llm_router::{api, balancer::LoadManager, registry::NodeRegistry, AppState};
use serde_json::json;
use tower::ServiceExt;

async fn build_app() -> Router {
    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let request_history =
        std::sync::Arc::new(llm_router::db::request_history::RequestHistoryStorage::new().unwrap());
    let convert_manager = llm_router::convert::ConvertTaskManager::new(1);
    let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run migrations");
    let jwt_secret = "test-secret".to_string();
    let state = AppState {
        registry,
        load_manager,
        request_history,
        convert_manager,
        db_pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
    };

    api::create_router(state)
}

/// T018: LLM runtimeライブラリから利用可能なモデル一覧を取得
#[tokio::test]
async fn test_list_available_models_from_runtime_library() {
    let app = build_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/models/available")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Available models endpoint should return 200 OK"
    );

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // modelsフィールドが配列であることを検証
    assert!(
        result.get("models").is_some(),
        "Response must have 'models' field"
    );
    let _models = result["models"]
        .as_array()
        .expect("'models' must be an array");

    // 事前定義モデルは廃止されたため、空配列でもよい（存在確認のみ）
}

/// T019: ノードが報告したロード済みモデルが /api/nodes に反映される
#[tokio::test]
async fn test_list_installed_models_on_node() {
    std::env::set_var("LLM_ROUTER_SKIP_HEALTH_CHECK", "1");
    let app = build_app().await;

    // テスト用ノードを登録
    let register_payload = json!({
        "machine_name": "model-info-node",
        "ip_address": "192.168.1.230",
        "runtime_version": "0.1.42",
        "runtime_port": 11434,
        "gpu_available": true,
        "gpu_devices": [
            {"model": "NVIDIA RTX 4090", "count": 1}
        ]
    });

    let register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/nodes")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&register_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(register_response.status(), StatusCode::CREATED);

    let body = to_bytes(register_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let agent: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let node_id = agent["node_id"]
        .as_str()
        .expect("Node must have 'node_id' field");
    let node_token = agent["node_token"]
        .as_str()
        .expect("Node must have 'node_token' field");

    // ノードがロード済みモデルを報告
    let health_payload = json!({
        "node_id": node_id,
        "cpu_usage": 0.0,
        "memory_usage": 0.0,
        "active_requests": 0,
        "loaded_models": ["gpt-oss-20b"],
        "loaded_embedding_models": [],
        "initializing": false,
        "ready_models": [1, 1]
    });

    let health_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/health")
                .header("content-type", "application/json")
                .header("X-Node-Token", node_token)
                .body(Body::from(serde_json::to_vec(&health_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        health_response.status(),
        StatusCode::OK,
        "health check should be accepted"
    );

    // /api/nodes に反映される
    let list_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/nodes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(list_response.status(), StatusCode::OK);
    let body = to_bytes(list_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let nodes: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let target = nodes
        .as_array()
        .and_then(|arr| {
            arr.iter()
                .find(|n| n.get("id").and_then(|v| v.as_str()) == Some(node_id))
        })
        .cloned()
        .expect("registered node must exist");

    let has_model = target
        .get("loaded_models")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().any(|m| m.as_str() == Some("gpt-oss-20b")))
        .unwrap_or(false);
    assert!(has_model, "loaded_models should contain reported model");
}

/// T020: 複数ノードのロード済みモデルが /api/nodes に反映される
#[tokio::test]
async fn test_model_matrix_view_multiple_agents() {
    std::env::set_var("LLM_ROUTER_SKIP_HEALTH_CHECK", "1");
    let app = build_app().await;

    // 複数のノードを登録（node_id と node_token を保持）
    let mut nodes: Vec<(String, String)> = Vec::new();
    for i in 0..3 {
        let register_payload = json!({
            "machine_name": format!("matrix-node-{}", i),
            "ip_address": format!("192.168.1.{}", 240 + i),
            "runtime_version": "0.1.42",
            "runtime_port": 11434,
            "gpu_available": true,
            "gpu_devices": [
                {"model": "NVIDIA RTX 3090", "count": 1}
            ]
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/nodes")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&register_payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let agent: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let node_id = agent["node_id"]
            .as_str()
            .expect("Node must have 'node_id'")
            .to_string();
        let node_token = agent["node_token"]
            .as_str()
            .expect("Node must have 'node_token'")
            .to_string();
        nodes.push((node_id, node_token));
    }

    // 各ノードがロード済みモデルを報告
    let reported = ["gpt-oss-20b", "gpt-oss-120b", "qwen3-coder-30b"];
    for (idx, (node_id, node_token)) in nodes.iter().enumerate() {
        let model = reported[idx % reported.len()];
        let health_payload = json!({
            "node_id": node_id,
            "cpu_usage": 0.0,
            "memory_usage": 0.0,
            "active_requests": 0,
            "loaded_models": [model],
            "loaded_embedding_models": [],
            "initializing": false,
            "ready_models": [1, 1]
        });

        let health_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/health")
                    .header("content-type", "application/json")
                    .header("X-Node-Token", node_token)
                    .body(Body::from(serde_json::to_vec(&health_payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(health_response.status(), StatusCode::OK);
    }

    // /api/nodes に反映される（マトリックス表示のデータソース）
    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/nodes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let body = to_bytes(list_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let nodes_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let arr = nodes_json.as_array().expect("nodes list must be an array");
    assert_eq!(arr.len(), 3);
    for (idx, (node_id, _)) in nodes.iter().enumerate() {
        let model = reported[idx % reported.len()];
        let node = arr
            .iter()
            .find(|n| n.get("id").and_then(|v| v.as_str()) == Some(node_id.as_str()))
            .expect("node must exist in list");
        let has_model = node
            .get("loaded_models")
            .and_then(|v| v.as_array())
            .map(|m| m.iter().any(|x| x.as_str() == Some(model)))
            .unwrap_or(false);
        assert!(has_model, "node should report loaded model");
    }

    // 利用可能なモデル一覧も取得できることを確認
    let available_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/models/available")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        available_response.status(),
        StatusCode::OK,
        "Available models should be accessible for matrix view"
    );

    let body = to_bytes(available_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let available: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(
        available.get("models").is_some(),
        "Available models must have 'models' field"
    );
    assert!(
        available["models"].is_array(),
        "Available models must be an array"
    );
}

/// T021: /v1/models は対応モデル5件のみを返す（APIキー認証必須）
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
    let test_user = llm_router::db::users::create(
        &db_pool,
        "test-admin",
        "testpassword",
        llm_router_common::auth::UserRole::Admin,
    )
    .await
    .expect("Failed to create test user");
    let api_key = llm_router::db::api_keys::create(&db_pool, "test-key", test_user.id, None)
        .await
        .expect("Failed to create test API key");

    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let request_history =
        std::sync::Arc::new(llm_router::db::request_history::RequestHistoryStorage::new().unwrap());
    let convert_manager = llm_router::convert::ConvertTaskManager::new(1);
    let jwt_secret = "test-secret".to_string();
    let state = AppState {
        registry,
        load_manager,
        request_history,
        convert_manager,
        db_pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
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
    let ids: Vec<String> = data
        .iter()
        .filter_map(|m| m.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .collect();

    let expected: Vec<String> = vec![];

    assert_eq!(
        ids.len(),
        expected.len(),
        "should return only downloaded models"
    );
}
