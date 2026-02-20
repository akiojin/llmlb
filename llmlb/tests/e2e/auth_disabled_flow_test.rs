//! 認証無効化モードのE2Eテスト
//!
//! AUTH_DISABLED=true のときに認証なしでアクセスできることを確認する
//!
//! NOTE: NodeRegistry廃止（SPEC-e8e9326e）に伴い、EndpointRegistryベースに更新済み。

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use llmlb::{api, balancer::LoadManager, registry::endpoints::EndpointRegistry, AppState};
use serial_test::serial;
use std::sync::Arc;
use tower::ServiceExt;

use crate::support;

struct EnvGuard {
    key: &'static str,
    value: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let prev = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self { key, value: prev }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(value) = &self.value {
            std::env::set_var(self.key, value);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

async fn build_app() -> Router {
    let db_pool = support::lb::create_test_db_pool().await;
    let endpoint_registry = EndpointRegistry::new(db_pool.clone())
        .await
        .expect("Failed to create endpoint registry");
    let load_manager = LoadManager::new(Arc::new(endpoint_registry.clone()));
    let request_history = std::sync::Arc::new(
        llmlb::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let jwt_secret = support::lb::test_jwt_secret();

    let http_client = reqwest::Client::new();
    let inference_gate = llmlb::inference_gate::InferenceGate::default();
    let shutdown = llmlb::shutdown::ShutdownController::default();
    let update_manager = llmlb::update::UpdateManager::new(
        http_client.clone(),
        inference_gate.clone(),
        shutdown.clone(),
    )
    .expect("Failed to create update manager");

    let state = AppState {
        load_manager,
        request_history,
        db_pool: db_pool.clone(),
        jwt_secret,
        http_client,
        queue_config: llmlb::config::QueueConfig::from_env(),
        event_bus: llmlb::events::create_shared_event_bus(),
        endpoint_registry,
        inference_gate,
        shutdown,
        update_manager,
        audit_log_writer: llmlb::audit::writer::AuditLogWriter::new(
            llmlb::db::audit_log::AuditLogStorage::new(db_pool.clone()),
            llmlb::audit::writer::AuditLogWriterConfig::default(),
        ),
        audit_log_storage: std::sync::Arc::new(llmlb::db::audit_log::AuditLogStorage::new(db_pool)),
        audit_archive_pool: None,
    };

    api::create_app(state)
}

/// SPEC-e8e9326e: /api/nodes は廃止されたため、/api/endpoints を使用
/// このテストはAUTH_DISABLEDモードの動作確認に焦点を当てる
#[tokio::test]
#[serial]
async fn auth_disabled_allows_dashboard_and_endpoints() {
    let _guard = EnvGuard::set("AUTH_DISABLED", "true");
    let app = build_app().await;

    // /api/endpoints エンドポイントをテスト（/api/nodesは廃止）
    let endpoints_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/endpoints")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        endpoints_response.status(),
        StatusCode::OK,
        "AUTH_DISABLED should allow /api/endpoints without auth"
    );

    let me_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/auth/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        me_response.status(),
        StatusCode::OK,
        "AUTH_DISABLED should allow /api/auth/me without auth"
    );

    let dashboard_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/dashboard")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        dashboard_response.status(),
        StatusCode::OK,
        "AUTH_DISABLED should allow /dashboard without auth"
    );
}
