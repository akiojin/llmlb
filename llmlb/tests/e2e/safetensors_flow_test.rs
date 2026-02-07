//! E2E RED: safetensors モデルの共通フロー（gpt-oss/Whisper/SD）
//!
//! T0yy: 想定する各モデル種別で necessary metadata が欠如すると smb registers fail

use crate::support;
use reqwest::{Client, StatusCode};
use serde_json::json;
use wiremock::{
    matchers::{method, path, query_param},
    Mock, MockServer, ResponseTemplate,
};

async fn assert_register_safetensors_fails(repo: &str, siblings: serde_json::Value) {
    let mock = MockServer::start().await;
    std::env::set_var("HF_BASE_URL", mock.uri());

    let (lb, db_pool) = support::lb::spawn_test_lb_with_db().await;
    let admin_key = support::lb::create_test_api_key(lb.addr(), &db_pool).await;

    Mock::given(method("GET"))
        .and(path(format!("/api/models/{repo}")))
        .and(query_param("expand", "siblings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "siblings": siblings
        })))
        .mount(&mock)
        .await;

    let response = Client::new()
        .post(format!("http://{}/api/models/register", lb.addr()))

        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "repo": repo,
            "format": "safetensors"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "safetensors register should fail when metadata is incomplete"
    );

    lb.stop().await;
}

#[tokio::test]
async fn test_e2e_register_gpt_oss_requires_metadata() {
    assert_register_safetensors_fails(
        "openai/gpt-oss-20b",
        json!([
            {"rfilename": "model.safetensors"}
        ]),
    )
    .await;
}

#[tokio::test]
async fn test_e2e_register_whisper_requires_metadata() {
    assert_register_safetensors_fails(
        "openai/whisper-base",
        json!([
            {"rfilename": "model.safetensors"}
        ]),
    )
    .await;
}

#[tokio::test]
async fn test_e2e_register_sd_requires_metadata() {
    assert_register_safetensors_fails(
        "runwayml/stable-diffusion-v1-5",
        json!([
            {"rfilename": "model.safetensors"}
        ]),
    )
    .await;
}
