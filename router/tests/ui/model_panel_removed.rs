//! UI Test: モデルパネルが削除されていることを確認
//!
//! NOTE: NodeRegistry廃止（SPEC-66555000）に伴い、
//! EndpointRegistryベースのAppStateを使用するように更新。

#[path = "../support/mod.rs"]
mod support;

use axum::body::to_bytes;
use support::router::create_test_router;
use tower::ServiceExt;

#[tokio::test]
async fn dashboard_html_has_no_model_panel() {
    // minimal router serving static files
    let (router, _db_pool) = create_test_router().await;
    let body = router
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

    assert!(html.contains("LLM Router"));
    assert!(
        !html.contains("available-models-list"),
        "model panel should be removed"
    );
    assert!(
        !html.contains("loaded-models-list"),
        "model load panel should be removed"
    );
}
