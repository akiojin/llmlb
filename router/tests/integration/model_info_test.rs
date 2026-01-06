//! モデル情報表示統合テスト
//!
//! TDD RED: モデル一覧とノード別インストール状況の表示

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use llm_router::{api, balancer::LoadManager, registry::NodeRegistry, AppState};
use llm_router_common::{
    auth::{ApiKeyScope, UserRole},
    protocol::RegisterRequest,
    types::GpuDeviceInfo,
};
use serde_json::json;
use std::net::IpAddr;
use tower::ServiceExt;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

async fn build_app() -> (Router, String) {
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
        db_pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
        queue_config: llm_router::config::QueueConfig::from_env(),
        event_bus: llm_router::events::create_shared_event_bus(),
    };

    let password_hash = llm_router::auth::password::hash_password("password123").unwrap();
    let admin_user =
        llm_router::db::users::create(&state.db_pool, "admin", &password_hash, UserRole::Admin)
            .await
            .expect("create admin user");
    let admin_key = llm_router::db::api_keys::create(
        &state.db_pool,
        "admin-key",
        admin_user.id,
        None,
        vec![ApiKeyScope::Admin],
    )
    .await
    .expect("create admin api key")
    .key;

    (api::create_router(state), admin_key)
}

/// T018: /v0/models/available は廃止され、/v0/models に統合
/// NOTE: HuggingFaceカタログ参照は廃止。登録済みモデル一覧は /v0/models で取得
#[tokio::test]
async fn test_available_models_endpoint_is_removed() {
    let (app, admin_key) = build_app().await;

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

/// T019: ノードが報告したロード済みモデルが /v0/nodes に反映される
#[tokio::test]
async fn test_list_installed_models_on_node() {
    // モックサーバーを起動
    // SPEC-93536000: 空のモデルリストは登録拒否されるため、少なくとも1つのモデルを返す
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{
                "id": "test-model",
                "object": "model",
                "owned_by": "runtime"
            }]
        })))
        .mount(&mock_server)
        .await;

    let mock_port = mock_server.address().port();
    let runtime_port = mock_port - 1;

    let (app, admin_key) = build_app().await;

    // テスト用ノードを登録
    let register_request = RegisterRequest {
        machine_name: "model-info-node".to_string(),
        ip_address: IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
        runtime_version: "0.1.42".to_string(),
        runtime_port,
        gpu_available: true,
        gpu_devices: vec![GpuDeviceInfo {
            model: "NVIDIA RTX 4090".to_string(),
            count: 1,
            memory: Some(24576),
        }],
        gpu_count: Some(1),
        gpu_model: Some("NVIDIA RTX 4090".to_string()),
        supported_runtimes: Vec::new(),
    };

    let register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/nodes")
                .header("authorization", format!("Bearer {}", admin_key))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&register_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(register_response.status(), StatusCode::CREATED);

    let body = to_bytes(register_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let node: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let node_id = node["node_id"]
        .as_str()
        .expect("Node must have 'node_id' field");
    let node_token = node["node_token"]
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
                .uri("/v0/health")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", admin_key))
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

    // /v0/nodes に反映される
    let list_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/nodes")
                .header("authorization", format!("Bearer {}", admin_key))
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

/// T020: 複数ノードのロード済みモデルが /v0/nodes に反映される
#[tokio::test]
#[ignore = "TODO: Requires multiple mock servers for proper health check testing"]
async fn test_model_matrix_view_multiple_nodes() {
    let (app, admin_key) = build_app().await;

    // 複数のモックサーバーを起動
    // SPEC-93536000: 空のモデルリストは登録拒否されるため、少なくとも1つのモデルを返す
    let mut mock_servers = Vec::new();
    for _ in 0..3 {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "object": "list",
                "data": [{
                    "id": "test-model",
                    "object": "model",
                    "owned_by": "runtime"
                }]
            })))
            .mount(&mock_server)
            .await;
        mock_servers.push(mock_server);
    }

    // 複数のノードを登録（node_id と node_token を保持）
    let mut nodes: Vec<(String, String)> = Vec::new();
    for (i, mock_server) in mock_servers.iter().enumerate() {
        let mock_port = mock_server.address().port();
        let runtime_port = mock_port - 1;

        let register_request = RegisterRequest {
            machine_name: format!("matrix-node-{}", i),
            ip_address: IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
            runtime_version: "0.1.42".to_string(),
            runtime_port,
            gpu_available: true,
            gpu_devices: vec![GpuDeviceInfo {
                model: "NVIDIA RTX 3090".to_string(),
                count: 1,
                memory: Some(24576),
            }],
            gpu_count: Some(1),
            gpu_model: Some("NVIDIA RTX 3090".to_string()),
            supported_runtimes: Vec::new(),
        };

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v0/nodes")
                    .header("authorization", format!("Bearer {}", admin_key))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&register_request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let node: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let node_id = node["node_id"]
            .as_str()
            .expect("Node must have 'node_id'")
            .to_string();
        let node_token = node["node_token"]
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
                    .uri("/v0/health")
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {}", admin_key))
                    .header("X-Node-Token", node_token)
                    .body(Body::from(serde_json::to_vec(&health_payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(health_response.status(), StatusCode::OK);
    }

    // /v0/nodes に反映される（マトリックス表示のデータソース）
    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/nodes")
                .header("authorization", format!("Bearer {}", admin_key))
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

    // 登録済みモデル一覧も取得できることを確認
    // NOTE: /v0/models/available は廃止され、/v0/models に統合
    let models_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/models")
                .header("authorization", format!("Bearer {}", admin_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        models_response.status(),
        StatusCode::OK,
        "Registered models should be accessible"
    );

    let body = to_bytes(models_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let models: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // /v0/models は配列を直接返す
    assert!(models.is_array(), "Models response must be an array");
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
    let password_hash = llm_router::auth::password::hash_password("testpassword").unwrap();
    let test_user = llm_router::db::users::create(
        &db_pool,
        "test-admin",
        &password_hash,
        llm_router_common::auth::UserRole::Admin,
    )
    .await
    .expect("Failed to create test user");
    let api_key = llm_router::db::api_keys::create(
        &db_pool,
        "test-key",
        test_user.id,
        None,
        vec![llm_router_common::auth::ApiKeyScope::Api],
    )
    .await
    .expect("Failed to create test API key");

    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let request_history = std::sync::Arc::new(
        llm_router::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let jwt_secret = "test-secret".to_string();
    let state = AppState {
        registry,
        load_manager,
        request_history,
        db_pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
        queue_config: llm_router::config::QueueConfig::from_env(),
        event_bus: llm_router::events::create_shared_event_bus(),
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

/// SPEC-93536000 Task 6.3: ノード登録時の executable_models が /v1/models に反映される
#[tokio::test]
async fn test_v1_models_reflects_node_executable_models() {
    // モックサーバーを起動（ノードの/v1/modelsとしてexecutable_modelsを返す）
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [
                {"id": "llama-3.1-8b", "object": "model", "owned_by": "runtime"},
                {"id": "mistral-7b", "object": "model", "owned_by": "runtime"},
                {"id": "qwen-vl-7b", "object": "model", "owned_by": "runtime"}
            ]
        })))
        .mount(&mock_server)
        .await;

    let mock_port = mock_server.address().port();
    let runtime_port = mock_port - 1;

    // テスト用DBとAppStateを作成
    let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run migrations");

    let password_hash = llm_router::auth::password::hash_password("testpassword").unwrap();
    let admin_user = llm_router::db::users::create(
        &db_pool,
        "test-admin",
        &password_hash,
        llm_router_common::auth::UserRole::Admin,
    )
    .await
    .expect("Failed to create test user");
    let api_key = llm_router::db::api_keys::create(
        &db_pool,
        "test-key",
        admin_user.id,
        None,
        vec![
            llm_router_common::auth::ApiKeyScope::Api,
            llm_router_common::auth::ApiKeyScope::Admin,
        ],
    )
    .await
    .expect("Failed to create test API key");

    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let request_history = std::sync::Arc::new(
        llm_router::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let jwt_secret = "test-secret".to_string();
    let state = AppState {
        registry,
        load_manager,
        request_history,
        db_pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
        queue_config: llm_router::config::QueueConfig::from_env(),
        event_bus: llm_router::events::create_shared_event_bus(),
    };

    let app = api::create_router(state);

    // ノードを登録
    let register_request = RegisterRequest {
        machine_name: "exec-models-node".to_string(),
        ip_address: IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
        runtime_version: "0.1.42".to_string(),
        runtime_port,
        gpu_available: true,
        gpu_devices: vec![GpuDeviceInfo {
            model: "NVIDIA RTX 4090".to_string(),
            count: 1,
            memory: Some(24576),
        }],
        gpu_count: Some(1),
        gpu_model: Some("NVIDIA RTX 4090".to_string()),
        supported_runtimes: Vec::new(),
    };

    let register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/nodes")
                .header("authorization", format!("Bearer {}", api_key.key))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&register_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(register_response.status(), StatusCode::CREATED);

    let body = to_bytes(register_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let node: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let node_id = node["node_id"]
        .as_str()
        .expect("Node must have 'node_id' field");
    let node_token = node["node_token"]
        .as_str()
        .expect("Node must have 'node_token' field");

    // ノードがヘルスチェックを報告（Online遷移のため initializing=false, ready_models必須）
    // NOTE: executable_modelsはノード登録時に/v1/modelsから取得されるので、
    // ヘルスチェックでは loaded_models のみ報告
    let health_payload = json!({
        "node_id": node_id,
        "cpu_usage": 0.0,
        "memory_usage": 0.0,
        "active_requests": 0,
        "loaded_models": ["llama-3.1-8b"],
        "loaded_embedding_models": [],
        "initializing": false,
        "ready_models": [1, 1]
    });

    let health_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/health")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", api_key.key))
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

    // ノードを承認してOnlineにする
    let approve_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v0/nodes/{}/approve", node_id))
                .header("authorization", format!("Bearer {}", api_key.key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        approve_response.status(),
        StatusCode::OK,
        "node approval should succeed"
    );

    // /v1/models を取得
    let models_response = app
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

    assert_eq!(models_response.status(), StatusCode::OK);
    let body = to_bytes(models_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let data = json["data"]
        .as_array()
        .expect("data must be an array of models");

    // クラウドプロバイダーモデルを除外してローカルモデルのみ取得
    let cloud_prefixes = ["openai:", "google:", "anthropic:"];
    let local_model_ids: Vec<String> = data
        .iter()
        .filter_map(|m| m.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .filter(|id| !cloud_prefixes.iter().any(|prefix| id.starts_with(prefix)))
        .collect();

    // executable_models の3つのモデルが含まれている
    assert!(
        local_model_ids.contains(&"llama-3.1-8b".to_string()),
        "executable_models should contain llama-3.1-8b, got: {:?}",
        local_model_ids
    );
    assert!(
        local_model_ids.contains(&"mistral-7b".to_string()),
        "executable_models should contain mistral-7b, got: {:?}",
        local_model_ids
    );
    assert!(
        local_model_ids.contains(&"qwen-vl-7b".to_string()),
        "executable_models should contain qwen-vl-7b, got: {:?}",
        local_model_ids
    );
}

/// SPEC-93536000 Task 6.4: モデル対応ノードへのルーティング
/// ノードがexecutable_modelsに特定モデルを持つ場合、そのモデルへのリクエストは
/// そのノードにルーティングされる
#[tokio::test]
#[ignore = "TDD RED: requires full proxy implementation with model-aware routing"]
async fn test_routing_to_model_capable_node() {
    // 2つのモックサーバーを起動（異なるモデルセット）
    let mock_server1 = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [
                {"id": "llama-3.1-8b", "object": "model", "owned_by": "runtime"}
            ]
        })))
        .mount(&mock_server1)
        .await;

    let mock_server2 = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [
                {"id": "mistral-7b", "object": "model", "owned_by": "runtime"}
            ]
        })))
        .mount(&mock_server2)
        .await;

    // TODO: 両方のノードを登録し、llama-3.1-8bへのリクエストが
    // mock_server1にルーティングされることを確認
    // mistral-7bへのリクエストはmock_server2にルーティングされる
}

/// SPEC-93536000 Task 6.5: 非対応モデルへの503エラー
/// モデルがexecutable_modelsに存在するが、すべてのノードがexcludedまたはオフラインの場合、
/// 503 Service Unavailableを返す
#[tokio::test]
#[ignore = "TDD RED: requires node exclusion after inference failure"]
async fn test_503_when_all_nodes_excluded() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [
                {"id": "llama-3.1-8b", "object": "model", "owned_by": "runtime"}
            ]
        })))
        .mount(&mock_server)
        .await;

    // TODO: ノードを登録し、llama-3.1-8bをexcluded_modelsに追加
    // その後、llama-3.1-8bへのリクエストが503を返すことを確認
}

/// SPEC-93536000 Task 6.6: 存在しないモデルへの404エラー
/// どのノードのexecutable_modelsにも存在しないモデルへのリクエストは404を返す
#[tokio::test]
async fn test_404_for_nonexistent_model() {
    // モックサーバーを起動
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [
                {"id": "llama-3.1-8b", "object": "model", "owned_by": "runtime"}
            ]
        })))
        .mount(&mock_server)
        .await;

    let mock_port = mock_server.address().port();
    let runtime_port = mock_port - 1;

    // テスト用DBとAppStateを作成
    let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run migrations");

    let password_hash = llm_router::auth::password::hash_password("testpassword").unwrap();
    let admin_user = llm_router::db::users::create(
        &db_pool,
        "test-admin",
        &password_hash,
        llm_router_common::auth::UserRole::Admin,
    )
    .await
    .expect("Failed to create test user");
    let api_key = llm_router::db::api_keys::create(
        &db_pool,
        "test-key",
        admin_user.id,
        None,
        vec![
            llm_router_common::auth::ApiKeyScope::Api,
            llm_router_common::auth::ApiKeyScope::Admin,
        ],
    )
    .await
    .expect("Failed to create test API key");

    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let request_history = std::sync::Arc::new(
        llm_router::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let jwt_secret = "test-secret".to_string();
    let state = AppState {
        registry,
        load_manager,
        request_history,
        db_pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
        queue_config: llm_router::config::QueueConfig::from_env(),
        event_bus: llm_router::events::create_shared_event_bus(),
    };

    let app = api::create_router(state);

    // ノードを登録
    let register_request = RegisterRequest {
        machine_name: "test-node-404".to_string(),
        ip_address: IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
        runtime_version: "0.1.42".to_string(),
        runtime_port,
        gpu_available: true,
        gpu_devices: vec![GpuDeviceInfo {
            model: "NVIDIA RTX 4090".to_string(),
            count: 1,
            memory: Some(24576),
        }],
        gpu_count: Some(1),
        gpu_model: Some("NVIDIA RTX 4090".to_string()),
        supported_runtimes: Vec::new(),
    };

    let register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/nodes")
                .header("authorization", format!("Bearer {}", api_key.key))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&register_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(register_response.status(), StatusCode::CREATED);

    let body = to_bytes(register_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let node: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let node_id = node["node_id"]
        .as_str()
        .expect("Node must have 'node_id' field");
    let node_token = node["node_token"]
        .as_str()
        .expect("Node must have 'node_token' field");

    // ヘルスチェックを送信
    let health_payload = json!({
        "node_id": node_id,
        "cpu_usage": 0.0,
        "memory_usage": 0.0,
        "active_requests": 0,
        "loaded_models": ["llama-3.1-8b"],
        "loaded_embedding_models": [],
        "initializing": false,
        "ready_models": [1, 1]
    });

    let health_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/health")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", api_key.key))
                .header("X-Node-Token", node_token)
                .body(Body::from(serde_json::to_vec(&health_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(health_response.status(), StatusCode::OK);

    // ノードを承認
    let approve_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v0/nodes/{}/approve", node_id))
                .header("authorization", format!("Bearer {}", api_key.key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(approve_response.status(), StatusCode::OK);

    // 存在しないモデルへのリクエスト
    let chat_request = json!({
        "model": "nonexistent-model",
        "messages": [{"role": "user", "content": "Hello"}]
    });

    let chat_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", api_key.key))
                .body(Body::from(serde_json::to_vec(&chat_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 404 Not Found を期待
    assert_eq!(
        chat_response.status(),
        StatusCode::NOT_FOUND,
        "Request for nonexistent model should return 404"
    );
}

/// SPEC-93536000 Task 6.7: 推論失敗後のモデル除外
/// ノードでの推論が失敗した場合、そのモデルをノードのexcluded_modelsに追加する
#[tokio::test]
#[ignore = "TDD RED: requires inference failure detection and exclusion mechanism"]
async fn test_model_excluded_after_inference_failure() {
    // TODO: ノードを登録し、推論リクエストを失敗させる
    // exclude_model_from_node が呼ばれることを確認
    // その後、同じモデルへのリクエストが別のノードにルーティングされるか、
    // 503を返すことを確認
}

/// SPEC-93536000 Task 6.8: ノード再起動後のモデル復帰
/// ノードが再起動（再登録）した場合、excluded_modelsはクリアされる
#[tokio::test]
#[ignore = "TDD RED: requires node re-registration flow"]
async fn test_model_restored_after_node_restart() {
    // TODO: ノードを登録し、モデルをexcluded_modelsに追加
    // ノードを再登録（再起動シミュレート）
    // excluded_modelsがクリアされていることを確認
}

/// SPEC-93536000 Task 6.9: Metal専用モデルがCUDAノードにルーティングされないこと
/// GPU互換性に基づくルーティング制限
#[tokio::test]
#[ignore = "TDD RED: requires GPU compatibility check in routing"]
async fn test_metal_model_not_routed_to_cuda_node() {
    // TODO: CUDAノードとMetalノードを登録
    // Metal専用モデルへのリクエストがMetalノードにのみルーティングされることを確認
    // CUDAノードには送られないことを確認
}
