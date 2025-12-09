use axum::{body::to_bytes, Router};
use llm_router::{api, balancer::LoadManager, registry::NodeRegistry, AppState};
use tower::ServiceExt;

async fn build_app() -> Router {
    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let request_history =
        std::sync::Arc::new(llm_router::db::request_history::RequestHistoryStorage::new().unwrap());
    let task_manager = llm_router::tasks::DownloadTaskManager::new();
    let convert_manager = llm_router::convert::ConvertTaskManager::new(1);
    let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run migrations");
    let jwt_secret = "test-secret".to_string();
    let state = AppState {
        registry,
        load_manager,
        request_history,
        task_manager,
        convert_manager,
        db_pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
    };

    api::create_router(state)
}

#[tokio::test]
async fn dashboard_has_hf_url_registration_controls() {
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

    assert!(
        html.contains("hf-register-url"),
        "HF URL textarea should be present for direct registration"
    );
    assert!(
        html.contains("hf-register-url-submit"),
        "HF URL submit button should be present"
    );
    assert!(
        html.contains("Download Tasks"),
        "download tasks panel should still be rendered"
    );
    assert!(
        html.contains("Convert Tasks"),
        "convert tasks panel should still be rendered"
    );
}
