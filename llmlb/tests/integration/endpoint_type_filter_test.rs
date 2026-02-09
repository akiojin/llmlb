//! Integration Test: US7 - タイプフィルタリング
//!
//! SPEC-66555000: エンドポイントタイプ自動判別機能
//!
//! 管理者として、特定タイプのエンドポイントのみを
//! フィルタリングして一覧表示したい。

use llmlb::common::auth::{ApiKeyPermission, UserRole};
use reqwest::Client;
use serde_json::{json, Value};
use sqlx::SqlitePool;

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
    endpoint_type: &str,
) -> Value {
    let response = client
        .post(format!("http://{}/api/endpoints", addr))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": name,
            "base_url": base_url,
            "endpoint_type": endpoint_type
        }))
        .send()
        .await
        .expect("endpoint registration failed");

    assert_eq!(response.status().as_u16(), 201);
    response.json().await.expect("endpoint response json")
}

/// US7-シナリオ1: タイプパラメータなしの場合、全エンドポイントを返す
#[tokio::test]
async fn test_list_endpoints_without_type_filter() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // 複数エンドポイントを登録
    let _ = create_endpoint(
        &client,
        server.addr(),
        &admin_key,
        "Endpoint 1",
        "http://localhost:9001",
        "xllm",
    )
    .await;
    let _ = create_endpoint(
        &client,
        server.addr(),
        &admin_key,
        "Endpoint 2",
        "http://localhost:9002",
        "ollama",
    )
    .await;
    let _ = create_endpoint(
        &client,
        server.addr(),
        &admin_key,
        "Endpoint 3",
        "http://localhost:9003",
        "vllm",
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
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let _ = create_endpoint(
        &client,
        server.addr(),
        &admin_key,
        "xLLM Endpoint",
        "http://localhost:9101",
        "xllm",
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
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let _ = create_endpoint(
        &client,
        server.addr(),
        &admin_key,
        "Ollama Endpoint",
        "http://localhost:9102",
        "ollama",
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
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let _ = create_endpoint(
        &client,
        server.addr(),
        &admin_key,
        "vLLM Endpoint",
        "http://localhost:9103",
        "vllm",
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
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let _ = create_endpoint(
        &client,
        server.addr(),
        &admin_key,
        "OpenAI Compatible Endpoint",
        "http://localhost:9104",
        "openai_compatible",
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

/// US7-シナリオ6: type=unknownでフィルタリング
#[tokio::test]
async fn test_list_endpoints_filter_by_unknown() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // オフラインエンドポイントを登録（unknownタイプになる）
    let _ = create_endpoint(
        &client,
        server.addr(),
        &admin_key,
        "Offline Endpoint",
        "http://localhost:9999",
        "unknown",
    )
    .await;

    // unknownタイプでフィルタ
    let response = client
        .get(format!(
            "http://{}/api/endpoints?type=unknown",
            server.addr()
        ))
        .header("authorization", format!("Bearer {}", admin_key))
        .send()
        .await
        .expect("list request failed");

    assert_eq!(response.status().as_u16(), 200);

    let body: Value = response.json().await.unwrap();
    let endpoints = body["endpoints"].as_array().unwrap();

    // オフラインエンドポイントが含まれることを確認
    assert!(
        !endpoints.is_empty(),
        "Should have at least one unknown endpoint"
    );
    for endpoint in endpoints {
        assert_eq!(endpoint["endpoint_type"], "unknown");
    }
}

/// US7-シナリオ7: 複数フィルタの組み合わせ（type + status）
#[tokio::test]
async fn test_list_endpoints_combined_filters() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let _ = create_endpoint(
        &client,
        server.addr(),
        &admin_key,
        "Pending xLLM",
        "http://localhost:9105",
        "xllm",
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
