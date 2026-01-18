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
    let endpoint_registry = llm_router::registry::endpoints::EndpointRegistry::new(db_pool.clone())
        .await
        .expect("Failed to create endpoint registry");
    let request_history = std::sync::Arc::new(
        llm_router::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let jwt_secret = "test-secret".to_string();
    #[allow(deprecated)]
    let state = AppState {
        registry,
        load_manager,
        request_history,
        db_pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
        queue_config: llm_router::config::QueueConfig::from_env(),
        event_bus: llm_router::events::create_shared_event_bus(),
        endpoint_registry,
    };

    api::create_router(state)
}

#[tokio::test]
async fn dashboard_html_has_no_model_panel() {
    // minimal router serving static files
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
