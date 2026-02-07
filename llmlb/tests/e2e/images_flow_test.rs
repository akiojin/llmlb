//! E2E RED: 画像生成API（generations/edits/variations）
//!
//! 実装前の期待振る舞いを定義する（ignored）。

use crate::support;
use axum::{
    body::Body,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use reqwest::{multipart, Client, StatusCode as ReqStatusCode};
use serde_json::{json, Value};

async fn spawn_image_stub() -> support::http::TestServer {
    let app = Router::new()
        .route("/v1/images/generations", post(images_generations_handler))
        .route("/v1/images/edits", post(images_edits_handler))
        .route("/v1/images/variations", post(images_variations_handler))
        .route("/v1/models", get(models_handler))
        .route("/api/health", post(|| async { StatusCode::OK }));
    support::http::spawn_lb(app).await
}

async fn images_generations_handler() -> impl IntoResponse {
    image_response()
}

async fn images_edits_handler() -> impl IntoResponse {
    image_response()
}

async fn images_variations_handler() -> impl IntoResponse {
    image_response()
}

fn image_response() -> axum::response::Response {
    let body = json!({
        "created": 1234567890,
        "data": [
            { "url": "https://example.com/image.png" }
        ]
    });
    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn models_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "object": "list",
            "data": [
                { "id": "sdxl" }
            ]
        })),
    )
        .into_response()
}

async fn register_image_endpoint(
    lb: &support::http::TestServer,
    node: &support::http::TestServer,
) -> String {
    let client = Client::new();

    let register_response = client
        .post(format!("http://{}/api/endpoints", lb.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "image-stub",
            "base_url": format!("http://{}", node.addr()),
            "capabilities": ["image_generation", "chat_completion"]
        }))
        .send()
        .await
        .expect("endpoint registration should succeed");
    assert_eq!(register_response.status(), ReqStatusCode::CREATED);

    let register_body: Value = register_response
        .json()
        .await
        .expect("endpoint registration response must be json");
    let endpoint_id = register_body["id"]
        .as_str()
        .expect("endpoint id should exist")
        .to_string();

    let test_response = client
        .post(format!(
            "http://{}/api/endpoints/{}/test",
            lb.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("endpoint test should succeed");
    assert_eq!(test_response.status(), ReqStatusCode::OK);

    let sync_response = client
        .post(format!(
            "http://{}/api/endpoints/{}/sync",
            lb.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("endpoint sync should succeed");
    assert_eq!(sync_response.status(), ReqStatusCode::OK);

    endpoint_id
}

fn dummy_png_bytes() -> Vec<u8> {
    vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]
}

#[tokio::test]
#[ignore = "TDD RED: image generations E2E not yet implemented"]
async fn e2e_images_generations_returns_image() {
    let node = spawn_image_stub().await;
    let lb = support::lb::spawn_test_lb().await;
    let _endpoint_id = register_image_endpoint(&lb, &node).await;

    let response = Client::new()
        .post(format!("http://{}/v1/images/generations", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&json!({
            "model": "sdxl",
            "prompt": "a cat sitting on a chair",
            "size": "1024x1024",
            "response_format": "url"
        }))
        .send()
        .await
        .expect("images/generations request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    let payload: Value = response.json().await.expect("json response");
    let url = payload["data"][0]["url"].as_str().unwrap_or_default();
    assert!(!url.is_empty(), "expected image url");

    node.stop().await;
    lb.stop().await;
}

#[tokio::test]
#[ignore = "TDD RED: image edits E2E not yet implemented"]
async fn e2e_images_edits_returns_image() {
    let node = spawn_image_stub().await;
    let lb = support::lb::spawn_test_lb().await;
    let _endpoint_id = register_image_endpoint(&lb, &node).await;

    let form = multipart::Form::new()
        .part(
            "image",
            multipart::Part::bytes(dummy_png_bytes())
                .file_name("image.png")
                .mime_str("image/png")
                .unwrap(),
        )
        .text("prompt", "make it brighter");

    let response = Client::new()
        .post(format!("http://{}/v1/images/edits", lb.addr()))
        .header("x-api-key", "sk_debug")
        .multipart(form)
        .send()
        .await
        .expect("images/edits request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    let payload: Value = response.json().await.expect("json response");
    let url = payload["data"][0]["url"].as_str().unwrap_or_default();
    assert!(!url.is_empty(), "expected edited image url");

    node.stop().await;
    lb.stop().await;
}

#[tokio::test]
#[ignore = "TDD RED: image variations E2E not yet implemented"]
async fn e2e_images_variations_returns_image() {
    let node = spawn_image_stub().await;
    let lb = support::lb::spawn_test_lb().await;
    let _endpoint_id = register_image_endpoint(&lb, &node).await;

    let form = multipart::Form::new().part(
        "image",
        multipart::Part::bytes(dummy_png_bytes())
            .file_name("image.png")
            .mime_str("image/png")
            .unwrap(),
    );

    let response = Client::new()
        .post(format!("http://{}/v1/images/variations", lb.addr()))
        .header("x-api-key", "sk_debug")
        .multipart(form)
        .send()
        .await
        .expect("images/variations request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    let payload: Value = response.json().await.expect("json response");
    let url = payload["data"][0]["url"].as_str().unwrap_or_default();
    assert!(!url.is_empty(), "expected variation image url");

    node.stop().await;
    lb.stop().await;
}
