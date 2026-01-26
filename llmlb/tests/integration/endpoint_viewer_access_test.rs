//! Integration Test: T016d - viewerロール制限
//!
//! SPEC-66555000: ルーター主導エンドポイント登録システム
//!
//! viewerロールはGET操作のみ許可、変更操作は禁止することを検証する。

use reqwest::Client;
use serde_json::{json, Value};

use crate::support::lb::spawn_test_lb_with_db;

/// viewerロールでエンドポイント一覧を取得可能
#[tokio::test]
async fn test_viewer_can_list_endpoints() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();

    // viewerユーザーとAPIキーを作成
    let password_hash = llmlb::auth::password::hash_password("password123").unwrap();
    let viewer_user = llmlb::db::users::create(
        &db_pool,
        "viewer",
        &password_hash,
        llmlb::common::auth::UserRole::Viewer,
    )
    .await
    .expect("create viewer user");

    let viewer_key = llmlb::db::api_keys::create(
        &db_pool,
        "viewer-key",
        viewer_user.id,
        None,
        vec![llmlb::common::auth::ApiKeyScope::Api],
    )
    .await
    .expect("create viewer api key")
    .key;

    // adminでエンドポイントを登録
    let _ = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Test Endpoint",
            "base_url": "http://localhost:11434"
        }))
        .send()
        .await
        .unwrap();

    // viewerで一覧取得
    let list_resp = client
        .get(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", viewer_key))
        .send()
        .await
        .unwrap();

    assert_eq!(list_resp.status().as_u16(), 200);
}

/// viewerロールでエンドポイント詳細を取得可能
#[tokio::test]
async fn test_viewer_can_get_endpoint_detail() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();

    // viewerユーザーとAPIキーを作成
    let password_hash = llmlb::auth::password::hash_password("password123").unwrap();
    let viewer_user = llmlb::db::users::create(
        &db_pool,
        "viewer",
        &password_hash,
        llmlb::common::auth::UserRole::Viewer,
    )
    .await
    .expect("create viewer user");

    let viewer_key = llmlb::db::api_keys::create(
        &db_pool,
        "viewer-key",
        viewer_user.id,
        None,
        vec![llmlb::common::auth::ApiKeyScope::Api],
    )
    .await
    .expect("create viewer api key")
    .key;

    // adminでエンドポイントを登録
    let reg_resp = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Detail Test",
            "base_url": "http://localhost:11434"
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();

    // viewerで詳細取得
    let detail_resp = client
        .get(format!(
            "http://{}/v0/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", format!("Bearer {}", viewer_key))
        .send()
        .await
        .unwrap();

    assert_eq!(detail_resp.status().as_u16(), 200);
}

/// viewerロールでエンドポイント登録は禁止
#[tokio::test]
async fn test_viewer_cannot_create_endpoint() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();

    // viewerユーザーとAPIキーを作成
    let password_hash = llmlb::auth::password::hash_password("password123").unwrap();
    let viewer_user = llmlb::db::users::create(
        &db_pool,
        "viewer",
        &password_hash,
        llmlb::common::auth::UserRole::Viewer,
    )
    .await
    .expect("create viewer user");

    let viewer_key = llmlb::db::api_keys::create(
        &db_pool,
        "viewer-key",
        viewer_user.id,
        None,
        vec![llmlb::common::auth::ApiKeyScope::Api],
    )
    .await
    .expect("create viewer api key")
    .key;

    // viewerでエンドポイント登録を試行
    let create_resp = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", viewer_key))
        .json(&json!({
            "name": "Forbidden",
            "base_url": "http://localhost:11434"
        }))
        .send()
        .await
        .unwrap();

    // 403 Forbiddenが期待される
    assert_eq!(create_resp.status().as_u16(), 403);
}

/// viewerロールでエンドポイント更新は禁止
#[tokio::test]
async fn test_viewer_cannot_update_endpoint() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();

    // viewerユーザーとAPIキーを作成
    let password_hash = llmlb::auth::password::hash_password("password123").unwrap();
    let viewer_user = llmlb::db::users::create(
        &db_pool,
        "viewer",
        &password_hash,
        llmlb::common::auth::UserRole::Viewer,
    )
    .await
    .expect("create viewer user");

    let viewer_key = llmlb::db::api_keys::create(
        &db_pool,
        "viewer-key",
        viewer_user.id,
        None,
        vec![llmlb::common::auth::ApiKeyScope::Api],
    )
    .await
    .expect("create viewer api key")
    .key;

    // adminでエンドポイントを登録
    let reg_resp = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Update Test",
            "base_url": "http://localhost:11434"
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();

    // viewerで更新を試行
    let update_resp = client
        .put(format!(
            "http://{}/v0/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", format!("Bearer {}", viewer_key))
        .json(&json!({
            "name": "Forbidden Update"
        }))
        .send()
        .await
        .unwrap();

    // 403 Forbiddenが期待される
    assert_eq!(update_resp.status().as_u16(), 403);
}

/// viewerロールでエンドポイント削除は禁止
#[tokio::test]
async fn test_viewer_cannot_delete_endpoint() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();

    // viewerユーザーとAPIキーを作成
    let password_hash = llmlb::auth::password::hash_password("password123").unwrap();
    let viewer_user = llmlb::db::users::create(
        &db_pool,
        "viewer",
        &password_hash,
        llmlb::common::auth::UserRole::Viewer,
    )
    .await
    .expect("create viewer user");

    let viewer_key = llmlb::db::api_keys::create(
        &db_pool,
        "viewer-key",
        viewer_user.id,
        None,
        vec![llmlb::common::auth::ApiKeyScope::Api],
    )
    .await
    .expect("create viewer api key")
    .key;

    // adminでエンドポイントを登録
    let reg_resp = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Delete Test",
            "base_url": "http://localhost:11434"
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();

    // viewerで削除を試行
    let delete_resp = client
        .delete(format!(
            "http://{}/v0/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", format!("Bearer {}", viewer_key))
        .send()
        .await
        .unwrap();

    // 403 Forbiddenが期待される
    assert_eq!(delete_resp.status().as_u16(), 403);
}
