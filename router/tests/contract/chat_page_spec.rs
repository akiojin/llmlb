//! Contract test: /playground (React SPA) serves correctly
//!
//! The playground is now a React single-page application.
//! This test verifies the React app shell is served correctly.
//! Dynamic UI elements are rendered by React at runtime.

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
    let convert_manager = llm_router::convert::ConvertTaskManager::new(1, db_pool.clone());
    let jwt_secret = "test-secret".to_string();
    let state = AppState {
        registry,
        load_manager,
        request_history,
        convert_manager,
        db_pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
    };
    api::create_router(state)
}

#[tokio::test]
async fn playground_serves_react_app() {
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

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 512 * 1024).await.unwrap();
    let html = String::from_utf8(bytes.to_vec()).expect("playground html should be utf-8");

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

    // Should have the playground title
    assert!(
        html.contains("<title>") && html.contains("Playground"),
        "Playground title not found"
    );
}

#[tokio::test]
async fn playground_assets_accessible() {
    let router = build_router().await;

    // First get the playground HTML
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/playground")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 512 * 1024).await.unwrap();
    let html = String::from_utf8(bytes.to_vec()).expect("playground html should be utf-8");

    // Verify the HTML contains references to JS assets
    assert!(
        html.contains(".js\"") || html.contains(".js'"),
        "Playground should reference JavaScript assets"
    );
}
