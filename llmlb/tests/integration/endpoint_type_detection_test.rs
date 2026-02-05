//! Integration Test: US6 - エンドポイントタイプ自動判別
//!
//! SPEC-66555000: エンドポイントタイプ自動判別機能
//!
//! 管理者として、エンドポイント登録時に自動的にタイプ
//! （xLLM/Ollama/vLLM/OpenAI互換）を判別してほしい。

use llmlb::common::auth::{ApiKeyScope, UserRole};
use llmlb::health::EndpointHealthChecker;
use llmlb::registry::endpoints::EndpointRegistry;
use reqwest::Client;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use uuid::Uuid;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

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
        vec![ApiKeyScope::Admin],
    )
    .await
    .expect("create admin api key");
    api_key.key
}

/// US6-シナリオ1: エンドポイント登録時にタイプが自動判別される
/// NOTE: 実際のエンドポイントがないとタイプ判別できないため、unknownになる
#[tokio::test]
async fn test_endpoint_type_auto_detection_offline() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // エンドポイント登録（接続先がないのでタイプはunknownになる）
    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Unknown Endpoint",
            "base_url": "http://localhost:9999"
        }))
        .send()
        .await
        .expect("registration request failed");

    assert_eq!(response.status().as_u16(), 201);

    let body: Value = response.json().await.unwrap();

    // endpoint_typeフィールドが存在し、オフラインの場合はunknownになる
    assert!(
        body["endpoint_type"].is_string(),
        "endpoint_type should be present in response"
    );
    assert_eq!(
        body["endpoint_type"], "unknown",
        "Offline endpoint should have unknown type"
    );
}

/// US6-シナリオ2: 判別の優先順位（xLLM > Ollama > vLLM > OpenAI互換）
/// NOTE: モックサーバーを使用したテストが必要（実際の判別ロジック検証）
#[tokio::test]
async fn test_endpoint_type_detection_priority() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/system"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "xllm_version": "0.1.0"
        })))
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": []
        })))
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("server", "vllm")
                .set_body_json(json!({"object": "list", "data": []})),
        )
        .mount(&mock)
        .await;

    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Priority Endpoint",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .expect("registration request failed");

    assert_eq!(response.status().as_u16(), 201);
    let body: Value = response.json().await.unwrap();
    assert_eq!(
        body["endpoint_type"], "xllm",
        "xLLM should win priority over other detections"
    );
}

/// US6-シナリオ3: xLLM判別（/api/systemエンドポイント）
#[tokio::test]
async fn test_endpoint_type_detection_xllm() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/system"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "xllm_version": "0.1.0"
        })))
        .mount(&mock)
        .await;

    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "xLLM Endpoint",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .expect("registration request failed");

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["endpoint_type"], "xllm");
}

/// US6-シナリオ4: Ollama判別（/api/tagsエンドポイント）
#[tokio::test]
async fn test_endpoint_type_detection_ollama() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/system"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": []
        })))
        .mount(&mock)
        .await;

    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Ollama Endpoint",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .expect("registration request failed");

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["endpoint_type"], "ollama");
}

/// US6-シナリオ5: vLLM判別（Serverヘッダー）
#[tokio::test]
async fn test_endpoint_type_detection_vllm() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/system"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("server", "vllm")
                .set_body_json(json!({"object": "list", "data": []})),
        )
        .mount(&mock)
        .await;

    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "vLLM Endpoint",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .expect("registration request failed");

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["endpoint_type"], "vllm");
}

/// US6-シナリオ6: OpenAI互換判別（フォールバック）
#[tokio::test]
async fn test_endpoint_type_detection_openai_compatible() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/system"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{"id": "test-model", "object": "model"}]
        })))
        .mount(&mock)
        .await;

    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "OpenAI Compatible Endpoint",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .expect("registration request failed");

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["endpoint_type"], "openai_compatible");
}

/// US6-シナリオ7: オンライン復帰時のタイプ再判別
#[tokio::test]
async fn test_endpoint_type_redetection_on_online() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/system"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/health"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{"id": "test-model", "object": "model"}]
        })))
        .mount(&mock)
        .await;

    // 最初はオフラインでunknownタイプ
    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Test Endpoint",
            "base_url": "http://localhost:9999"
        }))
        .send()
        .await
        .expect("registration request failed");

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["endpoint_type"], "unknown");

    let endpoint_id = body["id"].as_str().expect("endpoint id");
    let endpoint_uuid = Uuid::parse_str(endpoint_id).expect("endpoint uuid");

    // base_url をオンラインのモックへ更新
    let update_response = client
        .put(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "base_url": mock.uri()
        }))
        .send()
        .await
        .expect("update request failed");

    assert_eq!(update_response.status().as_u16(), 200);

    let registry = EndpointRegistry::new(db_pool.clone())
        .await
        .expect("endpoint registry init");
    let checker = EndpointHealthChecker::new(registry);
    let endpoint = llmlb::db::endpoints::get_endpoint(&db_pool, endpoint_uuid)
        .await
        .expect("get endpoint")
        .expect("endpoint exists");

    checker
        .check_endpoint(&endpoint)
        .await
        .expect("health check should succeed");

    let updated = llmlb::db::endpoints::get_endpoint(&db_pool, endpoint_uuid)
        .await
        .expect("get endpoint")
        .expect("endpoint exists");
    assert_eq!(updated.endpoint_type.as_str(), "openai_compatible");
}
