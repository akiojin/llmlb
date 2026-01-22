//! Integration RED: safetensors モデルのメタデータ検証
//!
//! T0xx: config/tokenizer/シャード index が揃っていないとモデル登録を拒否する

use crate::support;
use reqwest::{Client, StatusCode};
use serde_json::json;
use wiremock::{
    matchers::{method, path, query_param},
    Mock, MockServer, ResponseTemplate,
};

#[tokio::test]
async fn test_integration_register_safetensors_requires_metadata() {
    let mock = MockServer::start().await;
    std::env::set_var("HF_BASE_URL", mock.uri());

    let (lb, db_pool) = support::lb::spawn_test_lb_with_db().await;
    let admin_key = support::lb::create_test_api_key(lb.addr(), &db_pool).await;

    Mock::given(method("GET"))
        .and(path("/api/models/safetensors-missing-meta"))
        .and(query_param("expand", "siblings"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({
                "siblings": [
                    {"rfilename": "model.safetensors"}
                ]
            })),
        )
        .mount(&mock)
        .await;

    let response = Client::new()
        .post(format!("http://{}/v0/models/register", lb.addr()))
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
    let admin_key = support::lb::create_test_api_key(lb.addr(), &db_pool).await;

    Mock::given(method("GET"))
        .and(path("/api/models/sharded-safetensors"))
        .and(query_param("expand", "siblings"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({
                "siblings": [
                    {"rfilename": "config.json"},
                    {"rfilename": "tokenizer.json"},
                    {"rfilename": "model-00001.safetensors"},
                    {"rfilename": "model-00002.safetensors"}
                ]
            })),
        )
        .mount(&mock)
        .await;

    let response = Client::new()
        .post(format!("http://{}/v0/models/register", lb.addr()))
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
