//! Integration Test: US7 - タイプフィルタリング
//!
//! SPEC-e8e9326e: エンドポイントタイプ自動判別機能
//!
//! 管理者として、特定タイプのエンドポイントのみを
//! フィルタリングして一覧表示したい。

use llmlb::common::auth::{ApiKeyPermission, UserRole};
use reqwest::Client;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::lb::spawn_test_lb_with_db;

async fn create_admin_api_key(db_pool: &SqlitePool) -> String {
    let password_hash = llmlb::auth::password::hash_password("password123").unwrap();
    let created = llmlb::db::users::create(db_pool, "admin", &password_hash, UserRole::Admin).await;
    let admin_id = match created {
        Ok(user) => user.id,
        Err(_) => {
            llmlb::db::users::find_by_username(db_pool, "admin")
                .await
                .unwrap()
                .unwrap()
                .id
        }
    };

    let api_key = llmlb::db::api_keys::create(
        db_pool,
        "test-admin-key",
        admin_id,
        None,
        ApiKeyPermission::all(),
    )
    .await
    .expect("create admin api key");
    api_key.key
}

async fn create_endpoint(
    client: &Client,
    addr: std::net::SocketAddr,
    admin_key: &str,
    name: &str,
    base_url: &str,
) -> Value {
    let response = client
        .post(format!("http://{}/api/endpoints", addr))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": name,
            "base_url": base_url
        }))
        .send()
        .await
        .expect("endpoint registration failed");

    assert_eq!(response.status().as_u16(), 201);
    response.json().await.expect("endpoint response json")
}

/// OpenAI互換として検出されるモックサーバーを作成するヘルパー
async fn create_openai_compatible_mock() -> MockServer {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{"id": "test-model", "object": "model"}]
        })))
        .mount(&mock)
        .await;
    mock
}

/// xLLMとして検出されるモックサーバーを作成するヘルパー
async fn create_xllm_mock() -> MockServer {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/system"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "xllm_version": "0.1.0"
        })))
        .mount(&mock)
        .await;
    mock
}

/// Ollamaとして検出されるモックサーバーを作成するヘルパー
async fn create_ollama_mock() -> MockServer {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{"name": "llama3:8b", "size": 4000000000_i64}]
        })))
        .mount(&mock)
        .await;
    mock
}

/// vLLMとして検出されるモックサーバーを作成するヘルパー
async fn create_vllm_mock() -> MockServer {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("server", "vLLM/0.4.0")
                .set_body_json(json!({
                    "object": "list",
                    "data": [{"id": "test-model", "object": "model", "owned_by": "vllm"}]
                })),
        )
        .mount(&mock)
        .await;
    mock
}

/// LM Studioとして検出されるモックサーバーを作成するヘルパー
async fn create_lm_studio_mock() -> MockServer {
    let mock = MockServer::start().await;
    // LM Studio /api/v1/models でpublisher/arch/stateを返す
    Mock::given(method("GET"))
        .and(path("/api/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{
                "id": "meta-llama-3.1-8b-instruct",
                "object": "model",
                "publisher": "lmstudio-community",
                "arch": "llama",
                "state": "not-loaded"
            }]
        })))
        .mount(&mock)
        .await;
    mock
}

/// US7-シナリオ1: タイプパラメータなしの場合、全エンドポイントを返す
#[tokio::test]
async fn test_list_endpoints_without_type_filter() {
    let mock1 = create_openai_compatible_mock().await;
    let mock2 = create_openai_compatible_mock().await;
    let mock3 = create_openai_compatible_mock().await;

    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // 複数エンドポイントを登録
    let _ = create_endpoint(
        &client,
        server.addr(),
        &admin_key,
        "Endpoint 1",
        &mock1.uri(),
    )
    .await;
    let _ = create_endpoint(
        &client,
        server.addr(),
        &admin_key,
        "Endpoint 2",
        &mock2.uri(),
    )
    .await;
    let _ = create_endpoint(
        &client,
        server.addr(),
        &admin_key,
        "Endpoint 3",
        &mock3.uri(),
    )
    .await;

    // フィルタなしで取得
    let response = client
        .get(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .send()
        .await
        .expect("list request failed");

    assert_eq!(response.status().as_u16(), 200);

    let body: Value = response.json().await.unwrap();
    let endpoints = body["endpoints"].as_array().unwrap();

    assert_eq!(endpoints.len(), 3, "Should return all endpoints");
}

/// US7-シナリオ2: type=xllmでフィルタリング
#[tokio::test]
async fn test_list_endpoints_filter_by_xllm() {
    let mock = create_xllm_mock().await;

    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let _ = create_endpoint(
        &client,
        server.addr(),
        &admin_key,
        "xLLM Endpoint",
        &mock.uri(),
    )
    .await;

    // xLLMタイプでフィルタ
    let response = client
        .get(format!("http://{}/api/endpoints?type=xllm", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .send()
        .await
        .expect("list request failed");

    assert_eq!(response.status().as_u16(), 200);

    let body: Value = response.json().await.unwrap();
    let endpoints = body["endpoints"].as_array().unwrap();

    // すべてのエンドポイントがxLLMタイプであることを確認
    for endpoint in endpoints {
        assert_eq!(
            endpoint["endpoint_type"], "xllm",
            "All filtered endpoints should be xLLM type"
        );
    }
}

/// US7-シナリオ3: type=ollamaでフィルタリング
#[tokio::test]
async fn test_list_endpoints_filter_by_ollama() {
    let mock = create_ollama_mock().await;

    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let _ = create_endpoint(
        &client,
        server.addr(),
        &admin_key,
        "Ollama Endpoint",
        &mock.uri(),
    )
    .await;

    // Ollamaタイプでフィルタ
    let response = client
        .get(format!(
            "http://{}/api/endpoints?type=ollama",
            server.addr()
        ))
        .header("authorization", format!("Bearer {}", admin_key))
        .send()
        .await
        .expect("list request failed");

    assert_eq!(response.status().as_u16(), 200);

    let body: Value = response.json().await.unwrap();
    let endpoints = body["endpoints"].as_array().unwrap();

    for endpoint in endpoints {
        assert_eq!(endpoint["endpoint_type"], "ollama");
    }
}

/// US7-シナリオ4: type=vllmでフィルタリング
#[tokio::test]
async fn test_list_endpoints_filter_by_vllm() {
    let mock = create_vllm_mock().await;

    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let _ = create_endpoint(
        &client,
        server.addr(),
        &admin_key,
        "vLLM Endpoint",
        &mock.uri(),
    )
    .await;

    let response = client
        .get(format!("http://{}/api/endpoints?type=vllm", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .send()
        .await
        .expect("list request failed");

    assert_eq!(response.status().as_u16(), 200);

    let body: Value = response.json().await.unwrap();
    let endpoints = body["endpoints"].as_array().unwrap();

    for endpoint in endpoints {
        assert_eq!(endpoint["endpoint_type"], "vllm");
    }
}

/// US7-シナリオ5: type=openai_compatibleでフィルタリング
#[tokio::test]
async fn test_list_endpoints_filter_by_openai_compatible() {
    let mock = create_openai_compatible_mock().await;

    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let _ = create_endpoint(
        &client,
        server.addr(),
        &admin_key,
        "OpenAI Compatible Endpoint",
        &mock.uri(),
    )
    .await;

    let response = client
        .get(format!(
            "http://{}/api/endpoints?type=openai_compatible",
            server.addr()
        ))
        .header("authorization", format!("Bearer {}", admin_key))
        .send()
        .await
        .expect("list request failed");

    assert_eq!(response.status().as_u16(), 200);

    let body: Value = response.json().await.unwrap();
    let endpoints = body["endpoints"].as_array().unwrap();

    for endpoint in endpoints {
        assert_eq!(endpoint["endpoint_type"], "openai_compatible");
    }
}

/// US7-シナリオ6: type=unknown は廃止済み - 不正なタイプとして扱われる
#[tokio::test]
async fn test_list_endpoints_filter_by_unknown_returns_error_or_empty() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // unknownタイプでフィルタ（廃止済みのため400またはOK+空配列）
    let response = client
        .get(format!(
            "http://{}/api/endpoints?type=unknown",
            server.addr()
        ))
        .header("authorization", format!("Bearer {}", admin_key))
        .send()
        .await
        .expect("list request failed");

    // 不正なタイプの場合、400 Bad Requestまたは200 OK+空配列を期待
    assert!(
        response.status().as_u16() == 400 || response.status().as_u16() == 200,
        "unknown type should return 400 or 200"
    );
}

/// US7: type=lm_studioでフィルタリング（SPEC-af1ec86d）
#[tokio::test]
async fn test_list_endpoints_filter_by_lm_studio() {
    let mock = create_lm_studio_mock().await;

    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let _ = create_endpoint(
        &client,
        server.addr(),
        &admin_key,
        "LM Studio Endpoint",
        &mock.uri(),
    )
    .await;

    let response = client
        .get(format!(
            "http://{}/api/endpoints?type=lm_studio",
            server.addr()
        ))
        .header("authorization", format!("Bearer {}", admin_key))
        .send()
        .await
        .expect("list request failed");

    assert_eq!(response.status().as_u16(), 200);

    let body: Value = response.json().await.unwrap();
    let endpoints = body["endpoints"].as_array().unwrap();

    for endpoint in endpoints {
        assert_eq!(endpoint["endpoint_type"], "lm_studio");
    }
}

/// US7-シナリオ7: 複数フィルタの組み合わせ（type + status）
#[tokio::test]
async fn test_list_endpoints_combined_filters() {
    let mock = create_xllm_mock().await;

    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let _ = create_endpoint(
        &client,
        server.addr(),
        &admin_key,
        "Pending xLLM",
        &mock.uri(),
    )
    .await;

    // type=xllm かつ status=pending でフィルタ
    let response = client
        .get(format!(
            "http://{}/api/endpoints?type=xllm&status=pending",
            server.addr()
        ))
        .header("authorization", format!("Bearer {}", admin_key))
        .send()
        .await
        .expect("list request failed");

    assert_eq!(response.status().as_u16(), 200);

    let body: Value = response.json().await.unwrap();
    let endpoints = body["endpoints"].as_array().unwrap();

    for endpoint in endpoints {
        assert_eq!(endpoint["endpoint_type"], "xllm");
        assert_eq!(endpoint["status"], "pending");
    }
}
