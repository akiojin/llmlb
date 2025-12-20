use axum::{
    body::Body,
    http::{request::Builder, Request, StatusCode},
    Router,
};
use tower::ServiceExt;

#[allow(dead_code)]
pub fn admin_request() -> Builder {
    Request::builder().header("x-api-key", "sk_debug_admin")
}

#[allow(dead_code)]
pub async fn approve_node(app: &Router, node_id: &str) {
    let response = app
        .clone()
        .oneshot(
            admin_request()
                .method("POST")
                .uri(format!("/v0/nodes/{}/approve", node_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
