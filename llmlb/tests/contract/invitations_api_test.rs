//! 招待コード管理API Contract Tests
//!
//! POST /api/invitations, GET /api/invitations, DELETE /api/invitations/{id}

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

async fn login_admin(app: &Router) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "username": "admin",
                        "password": "password123"
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

async fn login_viewer(app: &Router, db_pool: &SqlitePool) -> String {
    let password_hash = llmlb::auth::password::hash_password("viewerpass").unwrap();
    llmlb::db::users::create(db_pool, "viewer1", &password_hash, UserRole::Viewer, false)
        .await
        .ok();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "username": "viewer1",
                        "password": "viewerpass"
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
// POST /api/invitations
// ---------------------------------------------------------------------------

/// 管理者が招待コードを発行
#[tokio::test]
#[serial]
async fn test_create_invitation_as_admin() {
    let (app, _db_pool) = build_app().await;
    let jwt = login_admin(&app).await;

    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("POST")
                .uri("/api/invitations")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let data: Value = serde_json::from_slice(&body).unwrap();
    assert!(data["id"].is_string());
    assert!(data["code"].is_string());
    assert!(data["created_at"].is_string());
    assert!(data["expires_at"].is_string());
}

/// 有効期限を指定して招待コード発行
#[tokio::test]
#[serial]
async fn test_create_invitation_with_custom_expiry() {
    let (app, _db_pool) = build_app().await;
    let jwt = login_admin(&app).await;

    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("POST")
                .uri("/api/invitations")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "expires_in_hours": 48 })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let data: Value = serde_json::from_slice(&body).unwrap();
    assert!(data["code"].is_string());
}

/// 認証なしで招待コード発行は401
#[tokio::test]
#[serial]
async fn test_create_invitation_requires_auth() {
    let (app, _db_pool) = build_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/invitations")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// Viewerユーザーが招待コード発行は403
#[tokio::test]
#[serial]
async fn test_create_invitation_viewer_forbidden() {
    let (app, db_pool) = build_app().await;
    let jwt = login_viewer(&app, &db_pool).await;

    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("POST")
                .uri("/api/invitations")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// ---------------------------------------------------------------------------
// GET /api/invitations
// ---------------------------------------------------------------------------

/// 管理者が招待コード一覧を取得（初期は空）
#[tokio::test]
#[serial]
async fn test_list_invitations_empty() {
    let (app, _db_pool) = build_app().await;
    let jwt = login_admin(&app).await;

    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("GET")
                .uri("/api/invitations")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let data: Value = serde_json::from_slice(&body).unwrap();
    assert!(data["invitations"].is_array());
}

/// 招待コード発行後に一覧で取得可能
#[tokio::test]
#[serial]
async fn test_list_invitations_after_create() {
    let (app, _db_pool) = build_app().await;
    let jwt = login_admin(&app).await;

    // 招待コードを作成
    let create_resp = app
        .clone()
        .oneshot(
            bearer_request(&jwt)
                .method("POST")
                .uri("/api/invitations")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_resp.status(), StatusCode::CREATED);

    // 一覧取得
    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("GET")
                .uri("/api/invitations")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let data: Value = serde_json::from_slice(&body).unwrap();
    let invitations = data["invitations"].as_array().unwrap();
    assert!(invitations.len() >= 1);
    assert!(invitations[0]["id"].is_string());
    assert!(invitations[0]["status"].is_string());
}

/// 認証なしで招待コード一覧は401
#[tokio::test]
#[serial]
async fn test_list_invitations_requires_auth() {
    let (app, _db_pool) = build_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/invitations")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// Viewerユーザーが招待コード一覧は403
#[tokio::test]
#[serial]
async fn test_list_invitations_viewer_forbidden() {
    let (app, db_pool) = build_app().await;
    let jwt = login_viewer(&app, &db_pool).await;

    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("GET")
                .uri("/api/invitations")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// ---------------------------------------------------------------------------
// DELETE /api/invitations/{id}
// ---------------------------------------------------------------------------

/// 管理者が招待コードを無効化
#[tokio::test]
#[serial]
async fn test_revoke_invitation() {
    let (app, _db_pool) = build_app().await;
    let jwt = login_admin(&app).await;

    // 招待コードを作成
    let create_resp = app
        .clone()
        .oneshot(
            bearer_request(&jwt)
                .method("POST")
                .uri("/api/invitations")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let create_body = to_bytes(create_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let create_data: Value = serde_json::from_slice(&create_body).unwrap();
    let invitation_id = create_data["id"].as_str().unwrap();

    // 無効化
    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("DELETE")
                .uri(&format!("/api/invitations/{}", invitation_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

/// 存在しない招待コードの無効化は404
#[tokio::test]
#[serial]
async fn test_revoke_nonexistent_invitation() {
    let (app, _db_pool) = build_app().await;
    let jwt = login_admin(&app).await;

    let fake_id = uuid::Uuid::new_v4();
    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("DELETE")
                .uri(&format!("/api/invitations/{}", fake_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// 認証なしで招待コード無効化は401
#[tokio::test]
#[serial]
async fn test_revoke_invitation_requires_auth() {
    let (app, _db_pool) = build_app().await;

    let fake_id = uuid::Uuid::new_v4();
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(&format!("/api/invitations/{}", fake_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// Viewerユーザーが招待コード無効化は403
#[tokio::test]
#[serial]
async fn test_revoke_invitation_viewer_forbidden() {
    let (app, db_pool) = build_app().await;
    let jwt = login_viewer(&app, &db_pool).await;

    let fake_id = uuid::Uuid::new_v4();
    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("DELETE")
                .uri(&format!("/api/invitations/{}", fake_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
