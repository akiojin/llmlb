use axum::{
    body::Body,
    http::{request::Builder, Request, StatusCode},
    Router,
};
use std::env;
use tower::ServiceExt;

#[allow(dead_code)]
pub fn admin_request() -> Builder {
    Request::builder()
        .header("x-internal-token", "test-internal")
        .header("x-api-key", admin_api_key())
}

#[cfg(debug_assertions)]
fn admin_api_key() -> String {
    env::var("LLM_ADMIN_TEST_KEY").unwrap_or_else(|_| "sk_debug_admin".to_string())
}

#[cfg(not(debug_assertions))]
fn admin_api_key() -> String {
    env::var("LLM_ADMIN_TEST_KEY")
        .expect("LLM_ADMIN_TEST_KEY must be set when debug_assertions is disabled")
}

#[allow(dead_code)]
pub async fn approve_node(app: &Router, node_id: &str) {
    let response = app
        .clone()
        .oneshot(
            admin_request()
                .method("POST")
                .uri(format!("/api/runtimes/{}/approve", node_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
