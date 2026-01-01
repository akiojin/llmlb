//! Contract test: Dashboard React app initial state
//!
//! Verifies the React SPA shell is served correctly.
//! With React, modals are rendered conditionally by JavaScript state,
//! so the initial HTML only contains the mount point.

use axum::{body::to_bytes, Router};
use llm_router::{api, balancer::LoadManager, registry::NodeRegistry, AppState};
use tower::ServiceExt;

async fn build_app() -> Router {
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
async fn react_app_shell_is_clean_on_initial_load() {
    // React SPA: initial HTML contains only the app shell (mount point)
    // Modals are rendered by React state, not present in static HTML
    let app = build_app().await;
    let body = app
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
