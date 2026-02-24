//! ユーザー作成API契約テスト（viewerロール）
//!
//! Issue #446: フロントエンドが "user" を送信するが、バックエンドは "viewer" のみ受け付ける。
//! このテストは "viewer" ロールでのユーザー作成が成功し、
//! "user" ロールでの作成が拒否されることを検証する。

use axum::{body::Body, http::Request};
use llmlb::common::auth::UserRole;
use serde_json::json;
use tower::ServiceExt;

use crate::support;

async fn build_app() -> (axum::Router, sqlx::SqlitePool) {
    let (app, db_pool) = support::lb::create_test_lb().await;

    // テスト用の管理者ユーザーを作成
    let password_hash = llmlb::auth::password::hash_password("password123").unwrap();
    llmlb::db::users::create(&db_pool, "admin", &password_hash, UserRole::Admin, false)
        .await
        .ok();

    (app, db_pool)
}

async fn login(app: &axum::Router) -> String {
    let login_response = app
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

    let login_body = axum::body::to_bytes(login_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let login_data: serde_json::Value = serde_json::from_slice(&login_body).unwrap();
    login_data["token"].as_str().unwrap().to_string()
}

/// viewerロールでのユーザー作成が成功することを検証
#[tokio::test]
async fn test_create_user_with_viewer_role() {
    let (app, _db_pool) = build_app().await;
    let token = login(&app).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "username": "testviewer",
                        "role": "viewer"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        axum::http::StatusCode::CREATED,
        "Creating user with 'viewer' role should succeed"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let resp: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(resp["user"]["username"].as_str().unwrap(), "testviewer");
    assert_eq!(
        resp["user"]["role"].as_str().unwrap(),
        "viewer",
        "Created user should have 'viewer' role"
    );
    assert!(
        resp["generated_password"].as_str().is_some(),
        "Response should include generated_password"
    );
}

/// "user" ロールでのユーザー作成は拒否されることを検証（不正なバリアント）
#[tokio::test]
async fn test_create_user_with_invalid_user_role_rejected() {
    let (app, _db_pool) = build_app().await;
    let token = login(&app).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "username": "testuser",
                        "role": "user"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        axum::http::StatusCode::UNPROCESSABLE_ENTITY,
        "Creating user with 'user' role (invalid variant) should be rejected"
    );
}

/// adminロールでのユーザー作成が成功することを検証
#[tokio::test]
async fn test_create_user_with_admin_role() {
    let (app, _db_pool) = build_app().await;
    let token = login(&app).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "username": "testadmin",
                        "role": "admin"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        axum::http::StatusCode::CREATED,
        "Creating user with 'admin' role should succeed"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let resp: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(resp["user"]["role"].as_str().unwrap(), "admin");
}
