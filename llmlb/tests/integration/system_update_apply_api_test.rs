//! Integration tests for self-update apply APIs.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use llmlb::{api, balancer::LoadManager, registry::endpoints::EndpointRegistry, AppState};
use serde_json::Value;
use serial_test::serial;
use std::sync::Arc;
use tower::ServiceExt;

use crate::support;

struct AuthDisabledGuard;

impl AuthDisabledGuard {
    fn set() -> Self {
        std::env::set_var("AUTH_DISABLED", "true");
        Self
    }
}

impl Drop for AuthDisabledGuard {
    fn drop(&mut self) {
        std::env::remove_var("AUTH_DISABLED");
    }
}

async fn build_app() -> Router {
    let temp_dir = std::env::temp_dir().join(format!(
        "update-api-test-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    std::env::set_var("LLMLB_DATA_DIR", &temp_dir);

    let db_pool = support::lb::create_test_db_pool().await;
    let request_history = Arc::new(llmlb::db::request_history::RequestHistoryStorage::new(
        db_pool.clone(),
    ));
    let endpoint_registry = EndpointRegistry::new(db_pool.clone())
        .await
        .expect("Failed to create endpoint registry");
    let load_manager = LoadManager::new(Arc::new(endpoint_registry.clone()));

    let http_client = reqwest::Client::new();
    let inference_gate = llmlb::inference_gate::InferenceGate::default();
    let shutdown = llmlb::shutdown::ShutdownController::default();
    let update_manager = llmlb::update::UpdateManager::new(
        http_client.clone(),
        inference_gate.clone(),
        shutdown.clone(),
    )
    .expect("Failed to create update manager");

    api::create_app(AppState {
        load_manager,
        request_history,
        db_pool,
        jwt_secret: support::lb::test_jwt_secret(),
        http_client,
        queue_config: llmlb::config::QueueConfig::from_env(),
        event_bus: llmlb::events::create_shared_event_bus(),
        endpoint_registry,
        inference_gate,
        shutdown,
        update_manager,
    })
}

#[tokio::test]
#[serial]
async fn normal_apply_returns_mode_and_queued_flag() {
    let _guard = AuthDisabledGuard::set();
    let app = build_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/system/update/apply")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["mode"], "normal");
    assert!(
        json.get("queued").and_then(|v| v.as_bool()).is_some(),
        "queued must be a boolean"
    );
}

#[tokio::test]
#[serial]
async fn force_apply_returns_conflict_when_no_update_available() {
    let _guard = AuthDisabledGuard::set();
    let app = build_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/system/update/apply/force")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_text = String::from_utf8(body.to_vec()).unwrap();
    assert!(
        body_text.contains("No update is available"),
        "unexpected error body: {body_text}"
    );
}
