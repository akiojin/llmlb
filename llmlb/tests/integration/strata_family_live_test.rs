//! Issue #908: opt-in identification and registration against real upstream servers.
//!
//! These tests never download models or start a mock engine. Start the upstream
//! server, set its URL, then explicitly select the ignored test (see README).

use crate::support::lb::spawn_test_lb_with_db;
use llmlb::common::auth::{ApiKeyPermission, UserRole};
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;

async fn check_live_endpoint(prefix: &str, expected: &str) {
    let base_url = std::env::var(format!("LLMLB_{prefix}_URL"))
        .expect("set the upstream URL before explicitly running this ignored test");
    assert!(
        !base_url.trim().is_empty(),
        "upstream URL must not be empty"
    );
    let api_key = std::env::var(format!("LLMLB_{prefix}_API_KEY")).ok();
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();
    let detected =
        llmlb::detection::detect_endpoint_type_with_client(&client, &base_url, api_key.as_deref())
            .await
            .expect("detect real upstream endpoint");
    assert_eq!(detected.endpoint_type.as_str(), expected);
    println!(
        "live detection: endpoint_type={expected}, reason={}",
        detected.reason
    );

    let (server, db_pool) = spawn_test_lb_with_db().await;
    let password = llmlb::auth::password::hash_password("live-smoke-only").unwrap();
    let admin = llmlb::db::users::create(&db_pool, "live-admin", &password, UserRole::Admin, false)
        .await
        .unwrap();
    let admin_key = llmlb::db::api_keys::create(
        &db_pool,
        "live-smoke-admin",
        admin.id,
        None,
        ApiKeyPermission::all(),
    )
    .await
    .unwrap();
    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .bearer_auth(&admin_key.key)
        .json(&json!({"name": expected, "base_url": base_url, "api_key": api_key}))
        .send()
        .await
        .expect("register real upstream endpoint in isolated llmlb");
    let status = response.status();
    let body: Value = response.json().await.unwrap();
    assert_eq!(status.as_u16(), 201, "registration: {body}");
    assert_eq!(body["endpoint_type"], expected);
    let id = uuid::Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
    let stored = llmlb::db::endpoints::get_endpoint(&db_pool, id)
        .await
        .unwrap()
        .expect("registered endpoint is persisted");
    assert_eq!(stored.endpoint_type.as_str(), expected);
    println!(
        "live registration: HTTP {status}, endpoint_type={expected}, SQLite={}",
        stored.endpoint_type
    );
}

#[tokio::test]
#[ignore = "requires a real Strata server and LLMLB_STRATA_URL"]
async fn test_live_strata_identification_and_registration() {
    check_live_endpoint("STRATA", "strata").await;
}

#[tokio::test]
#[ignore = "requires a real Project Maya server and LLMLB_MAYA_URL"]
async fn test_live_project_maya_identification_and_registration() {
    check_live_endpoint("MAYA", "project_maya").await;
}
