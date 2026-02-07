//! Integration Test: Dashboard endpoints response includes endpoint type metadata
//!
//! Dashboard UI expects `endpoint_type` and related metadata from
//! `GET /api/dashboard/endpoints` (Type column + detail modal).

use llmlb::common::auth::{ApiKeyScope, UserRole};
use reqwest::Client;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::support::lb::spawn_test_lb_with_db;

async fn create_admin_api_key(db_pool: &SqlitePool) -> String {
    let password_hash = llmlb::auth::password::hash_password("password123").unwrap();
    let created = llmlb::db::users::create(db_pool, "admin", &password_hash, UserRole::Admin).await;
    let admin_id = match created {
        Ok(user) => user.id,
        Err(_) => {
            llmlb::db::users::find_by_username(db_pool, "admin")
                .await
                .unwrap()
                .unwrap()
                .id
        }
    };

    let api_key = llmlb::db::api_keys::create(
        db_pool,
        "test-admin-key-dashboard-endpoints",
        admin_id,
        None,
        vec![ApiKeyScope::Admin],
    )
    .await
    .expect("create admin api key");

    api_key.key
}

#[tokio::test]
async fn test_dashboard_endpoints_includes_endpoint_type_metadata() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // Register endpoint with manual type to ensure metadata is set.
    let create_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Type for Dashboard",
            "base_url": "http://localhost:8080",
            "endpoint_type": "xllm",
            "endpoint_type_reason": "manual for dashboard test"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(create_resp.status().as_u16(), 201);
    let created: Value = create_resp.json().await.unwrap();
    let endpoint_id = created["id"].as_str().unwrap();

    let dash_resp = client
        .get(format!("http://{}/api/dashboard/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .send()
        .await
        .unwrap();

    assert_eq!(dash_resp.status().as_u16(), 200);
    let endpoints: Vec<Value> = dash_resp.json().await.unwrap();
    let endpoint = endpoints
        .into_iter()
        .find(|ep| ep["id"].as_str() == Some(endpoint_id))
        .expect("Endpoint should exist in dashboard list");

    assert_eq!(endpoint["endpoint_type"], "xllm");
    assert_eq!(endpoint["endpoint_type_source"], "manual");
    assert_eq!(
        endpoint["endpoint_type_reason"],
        "manual for dashboard test"
    );
    assert!(
        endpoint["endpoint_type_detected_at"].is_string(),
        "endpoint_type_detected_at should be set"
    );
}
