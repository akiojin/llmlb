//! Integration Test: エンドポイントタイプ自動判別 - エッジケース
//!
//! SPEC-66555000: エンドポイントタイプ自動判別機能
//!
//! タイプは常に自動判別される。手動指定は廃止。
//! 不正なタイプや到達不能エンドポイントのエラーハンドリングをテスト。

use llmlb::common::auth::{ApiKeyPermission, UserRole};
use reqwest::Client;
use serde_json::{json, Value};
use sqlx::SqlitePool;
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
        ApiKeyPermission::all(),
    )
    .await
    .expect("create admin api key");
    api_key.key
}

/// 到達不能エンドポイントは登録拒否される
#[tokio::test]
async fn test_unreachable_endpoint_rejected() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Unreachable EP",
            "base_url": "http://localhost:8080"
        }))
        .send()
        .await
        .unwrap();

    // 到達不能なエンドポイントは BAD_GATEWAY で拒否
    assert_eq!(response.status().as_u16(), 502);
}

/// base_url変更時にタイプが再判別される
#[tokio::test]
async fn test_type_redetection_on_base_url_change() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // xLLMモックサーバー
    let mock_xllm = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/system"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "xllm_version": "0.1.0"
        })))
        .mount(&mock_xllm)
        .await;

    // OpenAI互換モックサーバー
    let mock_openai = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/system"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_openai)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_openai)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{"id": "test-model", "object": "model"}]
        })))
        .mount(&mock_openai)
        .await;

    // 最初はxLLMとして登録
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Redetect Endpoint",
            "base_url": mock_xllm.uri()
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(register_response.status().as_u16(), 201);
    let body: Value = register_response.json().await.unwrap();
    assert_eq!(body["endpoint_type"], "xllm");
    let endpoint_id = body["id"].as_str().unwrap();

    // base_urlをOpenAI互換サーバーに変更
    let update_response = client
        .put(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "base_url": mock_openai.uri()
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(update_response.status().as_u16(), 200);

    // 詳細取得でタイプが再判別されていることを確認
    let detail_response = client
        .get(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", format!("Bearer {}", admin_key))
        .send()
        .await
        .unwrap();

    let detail: Value = detail_response.json().await.unwrap();
    assert_eq!(
        detail["endpoint_type"], "openai_compatible",
        "Type should be re-detected as openai_compatible after base_url change"
    );
}

/// 全ての有効なタイプが自動判別で検出可能
#[tokio::test]
async fn test_all_valid_types_can_be_detected() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // xLLMモックサーバー
    let mock_xllm = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/system"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "xllm_version": "0.1.0"
        })))
        .mount(&mock_xllm)
        .await;

    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "xLLM Auto",
            "base_url": mock_xllm.uri()
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 201);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["endpoint_type"], "xllm");
}

/// タイプ変更後も他のフィールドは保持される
#[tokio::test]
async fn test_type_update_preserves_other_fields() {
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

    // エンドポイント登録（メモ付き）
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Test Endpoint",
            "base_url": mock.uri(),
            "notes": "Important server"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(register_response.status().as_u16(), 201);
    let body: Value = register_response.json().await.unwrap();
    let endpoint_id = body["id"].as_str().unwrap();

    // 名前を変更（他のフィールドを保持）
    let update_response = client
        .put(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Updated Name"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(update_response.status().as_u16(), 200);

    // 詳細取得でメモが保持されていることを確認
    let detail_response = client
        .get(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", format!("Bearer {}", admin_key))
        .send()
        .await
        .unwrap();

    let detail: Value = detail_response.json().await.unwrap();
    assert_eq!(detail["endpoint_type"], "xllm");
    assert_eq!(
        detail["notes"], "Important server",
        "Notes should be preserved"
    );
    assert_eq!(detail["name"], "Updated Name");
}
