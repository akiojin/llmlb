//! Integration Test: ダッシュボード
//!
//! WebSocket接続 → リアルタイム更新 → ノード状態変化の受信

use axum::Router;
use futures::StreamExt;
use llm_router::{
    api, auth::jwt::create_jwt, balancer::LoadManager, registry::NodeRegistry, AppState,
};
use llm_router_common::auth::UserRole;
use serial_test::serial;
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

/// Guard to reset AUTH_DISABLED on drop
struct AuthDisabledGuard;

impl Drop for AuthDisabledGuard {
    fn drop(&mut self) {
        std::env::remove_var("AUTH_DISABLED");
    }
}

async fn build_test_app() -> (AppState, Router, AuthDisabledGuard) {
    let temp_dir = std::env::temp_dir().join(format!(
        "dashboard-ws-test-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    std::env::set_var("LLM_ROUTER_DATA_DIR", &temp_dir);
    std::env::set_var("HOME", &temp_dir);
    std::env::set_var("USERPROFILE", &temp_dir);
    // Disable authentication for integration tests (cleaned up by AuthDisabledGuard)
    std::env::set_var("AUTH_DISABLED", "true");
    let guard = AuthDisabledGuard;

    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run migrations");
    llm_router::api::models::clear_registered_models(&db_pool)
        .await
        .expect("clear registered models");
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
        event_bus: llm_router::events::create_shared_event_bus(),
    };

    let router = api::create_router(state.clone());
    (state, router, guard)
}

fn ws_url_with_token(addr: std::net::SocketAddr, secret: &str) -> String {
    let token = create_jwt("test-admin", UserRole::Admin, secret).expect("create test jwt");
    format!("ws://{}/ws/dashboard?token={}", addr, token)
}

#[tokio::test]
#[serial]
async fn test_dashboard_websocket_connection() {
    // Arrange: Router server startup
    let (state, router, _guard) = build_test_app().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    // Give the server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Act: WebSocket connection
    let ws_url = ws_url_with_token(addr, &state.jwt_secret);
    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("Failed to connect to WebSocket");

    let (mut _write, mut read) = ws_stream.split();

    // Assert: Should receive connection confirmation
    let msg = tokio::time::timeout(tokio::time::Duration::from_secs(5), read.next())
        .await
        .expect("Timeout waiting for message")
        .expect("No message received")
        .expect("Message error");

    if let Message::Text(text) = msg {
        let json: serde_json::Value = serde_json::from_str(&text).expect("Invalid JSON");
        assert_eq!(json["type"], "connected");
    } else {
        panic!("Expected text message, got {:?}", msg);
    }
}

#[tokio::test]
#[serial]
async fn test_dashboard_receives_node_registration_event() {
    // Arrange: Router server startup
    let (state, router, _guard) = build_test_app().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    // Give the server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Connect WebSocket
    let ws_url = ws_url_with_token(addr, &state.jwt_secret);
    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("Failed to connect to WebSocket");

    let (_write, mut read) = ws_stream.split();

    // Skip the initial "connected" message
    let _ = read.next().await;

    // Act: Register a node by publishing event directly (since we can't go through HTTP in this test)
    let node_id = uuid::Uuid::new_v4();
    state
        .event_bus
        .publish(llm_router::events::DashboardEvent::NodeRegistered {
            node_id,
            machine_name: "test-node".to_string(),
            ip_address: "127.0.0.1".to_string(),
            status: llm_router_common::types::NodeStatus::Online,
        });

    // Assert: WebSocket client should receive node registration event
    let msg = tokio::time::timeout(tokio::time::Duration::from_secs(5), read.next())
        .await
        .expect("Timeout waiting for message")
        .expect("No message received")
        .expect("Message error");

    if let Message::Text(text) = msg {
        let json: serde_json::Value = serde_json::from_str(&text).expect("Invalid JSON");
        assert_eq!(json["type"], "NodeRegistered");
        assert_eq!(json["data"]["node_id"], node_id.to_string());
        assert_eq!(json["data"]["machine_name"], "test-node");
    } else {
        panic!("Expected text message, got {:?}", msg);
    }
}

#[tokio::test]
#[serial]
async fn test_dashboard_receives_node_status_change() {
    // Arrange: Router server startup
    let (state, router, _guard) = build_test_app().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    // Give the server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Connect WebSocket
    let ws_url = ws_url_with_token(addr, &state.jwt_secret);
    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("Failed to connect to WebSocket");

    let (_write, mut read) = ws_stream.split();

    // Skip the initial "connected" message
    let _ = read.next().await;

    // Act: Publish a node status change event
    let node_id = uuid::Uuid::new_v4();
    state
        .event_bus
        .publish(llm_router::events::DashboardEvent::NodeStatusChanged {
            node_id,
            old_status: llm_router_common::types::NodeStatus::Online,
            new_status: llm_router_common::types::NodeStatus::Offline,
        });

    // Assert: WebSocket client should receive status change event
    let msg = tokio::time::timeout(tokio::time::Duration::from_secs(5), read.next())
        .await
        .expect("Timeout waiting for message")
        .expect("No message received")
        .expect("Message error");

    if let Message::Text(text) = msg {
        let json: serde_json::Value = serde_json::from_str(&text).expect("Invalid JSON");
        assert_eq!(json["type"], "NodeStatusChanged");
        assert_eq!(json["data"]["node_id"], node_id.to_string());
        assert_eq!(json["data"]["new_status"], "offline");
    } else {
        panic!("Expected text message, got {:?}", msg);
    }
}
