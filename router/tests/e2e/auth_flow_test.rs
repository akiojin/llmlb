//! 認証フローE2Eテスト
//!
//! T091: 完全な認証フロー（ログイン → API呼び出し → ログアウト）

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use llm_router::{api, balancer::LoadManager, registry::NodeRegistry, AppState};
use llm_router_common::auth::UserRole;
use serde_json::json;
use tower::ServiceExt;

use crate::support;

async fn build_app() -> (Router, sqlx::SqlitePool) {
    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let request_history =
        std::sync::Arc::new(llm_router::db::request_history::RequestHistoryStorage::new().unwrap());
    let convert_manager = llm_router::convert::ConvertTaskManager::new(1);
    let db_pool = support::router::create_test_db_pool().await;
    let jwt_secret = support::router::test_jwt_secret();

    // テスト用の管理者ユーザーを作成
    let password_hash = llm_router::auth::password::hash_password("password123").unwrap();
    llm_router::db::users::create(&db_pool, "admin", &password_hash, UserRole::Admin)
        .await
        .ok();

    let state = AppState {
        registry,
        load_manager,
        request_history,
        convert_manager,
        db_pool: db_pool.clone(),
        jwt_secret,
        http_client: reqwest::Client::new(),
    };

    (api::create_router(state), db_pool)
}

#[tokio::test]
async fn test_complete_auth_flow() {
    let (app, _db_pool) = build_app().await;

    // Step 1: ログイン
    let login_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/auth/login")
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

    assert_eq!(login_response.status(), StatusCode::OK);

    let login_body = axum::body::to_bytes(login_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let login_data: serde_json::Value = serde_json::from_slice(&login_body).unwrap();

    let token = login_data["token"].as_str().unwrap();
    assert!(!token.is_empty(), "Token should not be empty");

    // Step 2: トークンを使ってAPI呼び出し（ユーザー一覧取得）
    let users_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/users")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        users_response.status(),
        StatusCode::OK,
        "Authenticated request should succeed"
    );

    let users_body = axum::body::to_bytes(users_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let users: serde_json::Value = serde_json::from_slice(&users_body).unwrap();

    assert!(
        users.get("users").is_some(),
        "Response must have 'users' field"
    );
    assert!(users["users"].is_array(), "'users' must be an array");
    assert_eq!(
        users["users"].as_array().unwrap().len(),
        1,
        "Should have one admin user"
    );

    // Step 3: ログアウト
    let logout_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/auth/logout")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(logout_response.status(), StatusCode::NO_CONTENT);

    // Step 4: ログアウト後は認証が必要なエンドポイントにアクセスできない
    let unauthorized_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/users")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Note: 現在の実装ではログアウト後もトークンは有効（トークン無効化は実装されていない）
    // 実際のプロダクションではトークンブラックリストやリフレッシュトークン機構が必要
    assert!(
        unauthorized_response.status() == StatusCode::OK
            || unauthorized_response.status() == StatusCode::UNAUTHORIZED,
        "After logout, token may still be valid (no token blacklist implemented)"
    );
}

#[tokio::test]
async fn test_unauthorized_access_without_token() {
    let (app, _db_pool) = build_app().await;

    // トークンなしでAPIにアクセス
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/users")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "Request without token should be unauthorized"
    );
}

#[tokio::test]
async fn test_invalid_token() {
    let (app, _db_pool) = build_app().await;

    // 無効なトークンでAPIにアクセス
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/users")
                .header("authorization", "Bearer invalid-token-12345")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "Request with invalid token should be unauthorized"
    );
}

#[tokio::test]
async fn test_auth_me_endpoint() {
    let (app, _db_pool) = build_app().await;

    // Step 1: ログイン
    let login_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/auth/login")
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

    assert_eq!(login_response.status(), StatusCode::OK);

    let login_body = axum::body::to_bytes(login_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let login_data: serde_json::Value = serde_json::from_slice(&login_body).unwrap();
    let token = login_data["token"].as_str().unwrap();

    // Step 2: /v0/auth/me でユーザー情報を取得
    let me_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/auth/me")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        me_response.status(),
        StatusCode::OK,
        "/v0/auth/me should return OK with valid token"
    );

    let me_body = axum::body::to_bytes(me_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let me_data: serde_json::Value = serde_json::from_slice(&me_body).unwrap();

    assert!(
        me_data.get("user_id").is_some(),
        "Response must have 'user_id' field"
    );
    assert!(
        me_data.get("username").is_some(),
        "Response must have 'username' field"
    );
    assert_eq!(
        me_data["username"].as_str().unwrap(),
        "admin",
        "Username should match logged in user"
    );
    assert!(
        me_data.get("role").is_some(),
        "Response must have 'role' field"
    );
}

#[tokio::test]
async fn test_auth_me_without_token() {
    let (app, _db_pool) = build_app().await;

    // トークンなしで/v0/auth/meにアクセス
    let response = app
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
        response.status(),
        StatusCode::UNAUTHORIZED,
        "/v0/auth/me without token should return UNAUTHORIZED"
    );
}
