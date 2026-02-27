//! ユーザー管理API Contract Tests
//!
//! GET /api/users, POST /api/users, PUT /api/users/{id}, DELETE /api/users/{id}

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use llmlb::common::auth::UserRole;
use serde_json::{json, Value};
use serial_test::serial;
use sqlx::SqlitePool;
use tower::ServiceExt;

async fn build_app() -> (Router, SqlitePool) {
    let (app, db_pool) = crate::support::lb::create_test_lb().await;

    let password_hash = llmlb::auth::password::hash_password("password123").unwrap();
    llmlb::db::users::create(&db_pool, "admin", &password_hash, UserRole::Admin, false)
        .await
        .ok();

    (app, db_pool)
}

async fn login_as(app: &Router, username: &str, password: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "username": username,
                        "password": password
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let data: Value = serde_json::from_slice(&body).unwrap();
    data["token"].as_str().unwrap().to_string()
}

fn bearer_request(jwt: &str) -> axum::http::request::Builder {
    Request::builder().header("authorization", format!("Bearer {}", jwt))
}

// ---------------------------------------------------------------------------
// GET /api/users
// ---------------------------------------------------------------------------

/// 管理者がユーザー一覧を取得
#[tokio::test]
#[serial]
async fn test_list_users_as_admin() {
    let (app, _db_pool) = build_app().await;
    let jwt = login_as(&app, "admin", "password123").await;

    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("GET")
                .uri("/api/users")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let data: Value = serde_json::from_slice(&body).unwrap();
    assert!(data["users"].is_array());
    assert!(data["users"].as_array().unwrap().len() >= 1);
}

/// 認証なしでユーザー一覧は401
#[tokio::test]
#[serial]
async fn test_list_users_requires_auth() {
    let (app, _db_pool) = build_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/users")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// POST /api/users
// ---------------------------------------------------------------------------

/// 管理者がviewerユーザーを作成
#[tokio::test]
#[serial]
async fn test_create_viewer_user() {
    let (app, _db_pool) = build_app().await;
    let jwt = login_as(&app, "admin", "password123").await;

    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("POST")
                .uri("/api/users")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "username": "newviewer",
                        "role": "viewer"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let data: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(data["user"]["username"], "newviewer");
    assert_eq!(data["user"]["role"], "viewer");
    assert!(data["generated_password"].is_string());
}

/// 管理者がadminユーザーを作成
#[tokio::test]
#[serial]
async fn test_create_admin_user() {
    let (app, _db_pool) = build_app().await;
    let jwt = login_as(&app, "admin", "password123").await;

    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("POST")
                .uri("/api/users")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "username": "newadmin",
                        "role": "admin"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let data: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(data["user"]["role"], "admin");
}

/// 重複ユーザー名で作成は409
#[tokio::test]
#[serial]
async fn test_create_user_duplicate_username() {
    let (app, _db_pool) = build_app().await;
    let jwt = login_as(&app, "admin", "password123").await;

    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("POST")
                .uri("/api/users")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "username": "admin",
                        "role": "viewer"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

/// 不正なロールでユーザー作成は422
#[tokio::test]
#[serial]
async fn test_create_user_invalid_role() {
    let (app, _db_pool) = build_app().await;
    let jwt = login_as(&app, "admin", "password123").await;

    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("POST")
                .uri("/api/users")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "username": "baduser",
                        "role": "superuser"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

/// 認証なしでユーザー作成は401
#[tokio::test]
#[serial]
async fn test_create_user_requires_auth() {
    let (app, _db_pool) = build_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "username": "noauth",
                        "role": "viewer"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// PUT /api/users/{id}
// ---------------------------------------------------------------------------

/// 管理者がユーザーのロールを変更
#[tokio::test]
#[serial]
async fn test_update_user_role() {
    let (app, db_pool) = build_app().await;
    let jwt = login_as(&app, "admin", "password123").await;

    // viewerユーザーを作成
    let pw_hash = llmlb::auth::password::hash_password("viewerpass").unwrap();
    let viewer =
        llmlb::db::users::create(&db_pool, "rolechange", &pw_hash, UserRole::Viewer, false)
            .await
            .unwrap();

    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("PUT")
                .uri(&format!("/api/users/{}", viewer.id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "role": "admin" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let data: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(data["role"], "admin");
}

/// 管理者がユーザー名を変更
#[tokio::test]
#[serial]
async fn test_update_user_username() {
    let (app, db_pool) = build_app().await;
    let jwt = login_as(&app, "admin", "password123").await;

    let pw_hash = llmlb::auth::password::hash_password("pass1234").unwrap();
    let user = llmlb::db::users::create(&db_pool, "oldname", &pw_hash, UserRole::Viewer, false)
        .await
        .unwrap();

    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("PUT")
                .uri(&format!("/api/users/{}", user.id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "username": "newname" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let data: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(data["username"], "newname");
}

/// 存在しないユーザーの更新は404
#[tokio::test]
#[serial]
async fn test_update_nonexistent_user() {
    let (app, _db_pool) = build_app().await;
    let jwt = login_as(&app, "admin", "password123").await;

    let fake_id = uuid::Uuid::new_v4();
    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("PUT")
                .uri(&format!("/api/users/{}", fake_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "username": "ghost" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// ユーザー名重複の更新は409
#[tokio::test]
#[serial]
async fn test_update_user_duplicate_username() {
    let (app, db_pool) = build_app().await;
    let jwt = login_as(&app, "admin", "password123").await;

    let pw_hash = llmlb::auth::password::hash_password("pass1234").unwrap();
    let user = llmlb::db::users::create(&db_pool, "user_a", &pw_hash, UserRole::Viewer, false)
        .await
        .unwrap();
    llmlb::db::users::create(&db_pool, "user_b", &pw_hash, UserRole::Viewer, false)
        .await
        .ok();

    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("PUT")
                .uri(&format!("/api/users/{}", user.id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "username": "user_b" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

// ---------------------------------------------------------------------------
// DELETE /api/users/{id}
// ---------------------------------------------------------------------------

/// 管理者がユーザーを削除
#[tokio::test]
#[serial]
async fn test_delete_user() {
    let (app, db_pool) = build_app().await;
    let jwt = login_as(&app, "admin", "password123").await;

    let pw_hash = llmlb::auth::password::hash_password("pass1234").unwrap();
    let user = llmlb::db::users::create(&db_pool, "deleteme", &pw_hash, UserRole::Viewer, false)
        .await
        .unwrap();

    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("DELETE")
                .uri(&format!("/api/users/{}", user.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

/// 存在しないユーザーの削除は404
#[tokio::test]
#[serial]
async fn test_delete_nonexistent_user() {
    let (app, _db_pool) = build_app().await;
    let jwt = login_as(&app, "admin", "password123").await;

    let fake_id = uuid::Uuid::new_v4();
    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("DELETE")
                .uri(&format!("/api/users/{}", fake_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// 最後の管理者は削除不可（400）
#[tokio::test]
#[serial]
async fn test_delete_last_admin_fails() {
    let (app, db_pool) = build_app().await;
    let jwt = login_as(&app, "admin", "password123").await;

    // admin ユーザーIDを取得
    let admin_user = llmlb::db::users::find_by_username(&db_pool, "admin")
        .await
        .unwrap()
        .unwrap();

    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("DELETE")
                .uri(&format!("/api/users/{}", admin_user.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// 認証なしでユーザー削除は401
#[tokio::test]
#[serial]
async fn test_delete_user_requires_auth() {
    let (app, _db_pool) = build_app().await;

    let fake_id = uuid::Uuid::new_v4();
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(&format!("/api/users/{}", fake_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
