use coordinator::api;
use tower::ServiceExt;
use axum::Router;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn dashboard_html_has_no_model_panel() {
    // minimal router serving static files
    let app = api::create_router(coordinator::AppState::new_for_tests());
    let body = app
        .oneshot(axum::http::Request::builder()
            .uri("/dashboard/")
            .body(axum::body::Body::empty())
            .unwrap())
        .await
        .unwrap()
        .into_body();
    let bytes = hyper::body::to_bytes(body).await.unwrap();
    let html = String::from_utf8_lossy(&bytes);

    assert!(html.contains("Ollama Coordinator"));
    assert!(!html.contains("available-models-list"), "model panel should be removed");
    assert!(!html.contains("loaded-models-list"), "model load panel should be removed");
}
