//! UI Test: モデルパネルが削除されていることを確認
//!
//! NOTE: NodeRegistry廃止（SPEC-e8e9326e）に伴い、
//! EndpointRegistryベースのAppStateを使用するように更新。

#![allow(clippy::duplicate_mod)]

#[path = "../support/mod.rs"]
mod support;

use axum::body::to_bytes;
use support::lb::create_test_lb;
use tower::ServiceExt;

#[tokio::test]
async fn dashboard_html_has_no_model_panel() {
    // minimal load balancer serving static files
    let (lb, _db_pool) = create_test_lb().await;
    let body = lb
        .oneshot(
            axum::http::Request::builder()
                .uri("/dashboard/")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .into_body();
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    let html = String::from_utf8_lossy(&bytes);

    assert!(html.contains("LLM Load Balancer"));
    // The title belongs to the served shell; removed panels belong to React.
    let markup = html.to_string() + &super::source::dashboard_sources();
    assert!(
        !markup.contains("available-models-list"),
        "model panel should be removed"
    );
    assert!(
        !markup.contains("loaded-models-list"),
        "model load panel should be removed"
    );
}
