//! Integration RED: safetensors モデルのメタデータ検証
//!
//! T0xx: config/tokenizer/シャード index が揃っていないとモデル登録を拒否する

use crate::support;
use llmlb::common::auth::{ApiKeyScope, UserRole};
use reqwest::{Client, StatusCode};
use serde_json::json;
use sqlx::SqlitePool;
use wiremock::{
    matchers::{method, path, query_param},
    Mock, MockServer, ResponseTemplate,
};

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

#[tokio::test]
async fn test_integration_register_safetensors_requires_metadata() {
    let mock = MockServer::start().await;
    std::env::set_var("HF_BASE_URL", mock.uri());

    let (lb, db_pool) = support::lb::spawn_test_lb_with_db().await;
    let admin_key = create_admin_api_key(&db_pool).await;

    Mock::given(method("GET"))
        .and(path("/api/models/safetensors-missing-meta"))
        .and(query_param("expand", "siblings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "siblings": [
                {"rfilename": "model.safetensors"}
            ]
        })))
        .mount(&mock)
        .await;

    let response = Client::new()
        .post(format!("http://{}/api/models/register", lb.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "repo": "safetensors-missing-meta",
            "format": "safetensors"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "safetensors registration without metadata should be rejected"
    );

    lb.stop().await;
}

#[tokio::test]
async fn test_integration_register_sharded_safetensors_requires_index() {
    let mock = MockServer::start().await;
    std::env::set_var("HF_BASE_URL", mock.uri());

    let (lb, db_pool) = support::lb::spawn_test_lb_with_db().await;
    let admin_key = create_admin_api_key(&db_pool).await;

    Mock::given(method("GET"))
        .and(path("/api/models/sharded-safetensors"))
        .and(query_param("expand", "siblings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "siblings": [
                {"rfilename": "config.json"},
                {"rfilename": "tokenizer.json"},
                {"rfilename": "model-00001.safetensors"},
                {"rfilename": "model-00002.safetensors"}
            ]
        })))
        .mount(&mock)
        .await;

    let response = Client::new()
        .post(format!("http://{}/api/models/register", lb.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "repo": "sharded-safetensors",
            "format": "safetensors"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "sharded safetensors without index should be rejected"
    );

    lb.stop().await;
}
