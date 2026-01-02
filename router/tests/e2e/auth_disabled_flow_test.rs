//! 認証無効化モードのE2Eテスト
//!
//! AUTH_DISABLED=true のときに認証なしでアクセスできることを確認する

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use llm_router::{api, balancer::LoadManager, registry::NodeRegistry, AppState};
use serial_test::serial;
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
    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let db_pool = support::router::create_test_db_pool().await;
    let request_history = std::sync::Arc::new(
        llm_router::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let convert_manager = llm_router::convert::ConvertTaskManager::new(1, db_pool.clone());
    let jwt_secret = support::router::test_jwt_secret();

    let state = AppState {
        registry,
        load_manager,
        request_history,
        convert_manager,
        db_pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
        queue_config: llm_router::config::QueueConfig::from_env(),
        event_bus: llm_router::events::create_shared_event_bus(),
    };

    api::create_router(state)
}

#[tokio::test]
#[serial]
async fn auth_disabled_allows_dashboard_and_nodes() {
    let _guard = EnvGuard::set("AUTH_DISABLED", "true");
    let app = build_app().await;

    let nodes_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/nodes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        nodes_response.status(),
        StatusCode::OK,
        "AUTH_DISABLED should allow /v0/nodes without auth"
    );

    let me_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/auth/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        me_response.status(),
        StatusCode::OK,
        "AUTH_DISABLED should allow /v0/auth/me without auth"
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
