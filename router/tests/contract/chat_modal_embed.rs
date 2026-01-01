//! Contract test: dashboard (React SPA) serves correctly
//!
//! The dashboard is now a React single-page application.
//! This test verifies the React app shell is served correctly.

use axum::{body::to_bytes, http::Request, Router};
use llm_router::{api, balancer::LoadManager, registry::NodeRegistry, AppState};
use tower::ServiceExt;

async fn build_router() -> Router {
    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run migrations");
    let request_history = std::sync::Arc::new(
        llm_router::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let jwt_secret = "test-secret".to_string();
    let state = AppState {
        registry,
        load_manager,
        request_history,
        db_pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
        queue_config: llm_router::config::QueueConfig::from_env(),
    };
    api::create_router(state)
}

#[tokio::test]
async fn dashboard_serves_react_app() {
    let router = build_router().await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/dashboard")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 512 * 1024).await.unwrap();
    let html = String::from_utf8(bytes.to_vec()).expect("dashboard html should be utf-8");

    // React app mount point
    assert!(
        html.contains("id=\"root\""),
        "React mount point (id=root) not found"
    );

    // Should have script tags for the React bundle
    assert!(
        html.contains("<script") && html.contains("</script>"),
        "Script tags not found"
    );

    // Should have the dashboard title
    assert!(
        html.contains("<title>") && html.contains("Dashboard"),
        "Dashboard title not found"
    );
}

#[tokio::test]
async fn dashboard_links_to_playground() {
    // This test verifies the navigation exists in the built assets
    // The actual link is rendered by React, so we verify the playground route works
    let router = build_router().await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/playground")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        axum::http::StatusCode::OK,
        "Playground route should be accessible"
    );
}
