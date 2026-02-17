//! Dashboard smoke tests
//!
//! Axum ロードバランサーを直接呼び出し、ダッシュボードの主要なHTTP経路が期待通りに
//! 応答することを確認する。UI機能の最小限のE2E保証として利用する。
//!
//! NOTE: NodeRegistry廃止（SPEC-e8e9326e）に伴い、一部のテストは
//! EndpointRegistryベースのテストに移行が必要です。

mod support;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use support::lb::create_test_lb;
use tower::ServiceExt;

#[tokio::test]
async fn dashboard_serves_static_index() {
    let (lb, _db_pool) = create_test_lb().await;

    let response = lb
        .clone()
        .oneshot(
            Request::builder()
                .uri("/dashboard/index.html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let (parts, body) = response.into_parts();
    let bytes = to_bytes(body, 1024 * 1024).await.unwrap();

    let content_type = parts
        .headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(
        content_type.starts_with("text/html"),
        "content-type was {content_type}"
    );
    assert!(
        bytes.starts_with(b"<!DOCTYPE html>"),
        "unexpected body prefix: {:?}",
        &bytes[..bytes.len().min(32)]
    );
}

#[tokio::test]
async fn dashboard_static_index_is_react_app() {
    // Dashboard is now a React SPA - verify app shell is served correctly
    let (lb, _db_pool) = create_test_lb().await;

    let response = lb
        .clone()
        .oneshot(
            Request::builder()
                .uri("/dashboard/index.html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let html = String::from_utf8(bytes.to_vec()).expect("dashboard html should be valid utf-8");

    // React app mount point
    assert!(
        html.contains("id=\"root\""),
        "dashboard should have React mount point: {html}"
    );

    // Should reference bundled JavaScript
    assert!(
        html.contains("<script") && html.contains("</script>"),
        "dashboard should reference bundled scripts: {html}"
    );

    // Should have appropriate title
    assert!(
        html.contains("Dashboard"),
        "dashboard should have Dashboard in title: {html}"
    );
}

// NOTE: 以下のテストはSPEC-e8e9326e（NodeRegistry廃止）により削除済み:
// - dashboard_nodes_and_stats_reflect_registry
// - dashboard_request_history_tracks_activity
// - dashboard_overview_returns_combined_payload
// - dashboard_node_metrics_endpoint_returns_history
//
// EndpointRegistryベースのテストは llmlb/tests/e2e/dashboard_flow_test.rs で実装済み。
