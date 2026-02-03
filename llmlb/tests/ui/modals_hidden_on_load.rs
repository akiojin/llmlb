//! Contract test: Dashboard React app initial state
//!
//! Verifies the React SPA shell is served correctly.
//! With React, modals are rendered conditionally by JavaScript state,
//! so the initial HTML only contains the mount point.
//!
//! NOTE: NodeRegistry廃止（SPEC-66555000）に伴い、EndpointRegistryベースに更新済み。

#![allow(clippy::duplicate_mod)]

#[path = "../support/mod.rs"]
mod support;

use axum::body::to_bytes;
use support::lb::create_test_lb;
use tower::ServiceExt;

#[tokio::test]
async fn react_app_shell_is_clean_on_initial_load() {
    // React SPA: initial HTML contains only the app shell (mount point)
    // Modals are rendered by React state, not present in static HTML
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

    // React mount point should exist
    assert!(
        html.contains("id=\"root\""),
        "React mount point (id=root) should exist",
    );

    // Initial HTML should be clean (no pre-rendered modal content)
    // With React, modal content is rendered by JavaScript, not in initial HTML
    assert!(
        !html.contains("class=\"modal\""),
        "React SPA should not have pre-rendered modal HTML",
    );

    // Should have script references for the React bundle
    assert!(
        html.contains("<script"),
        "React app should reference JavaScript bundles",
    );
}
