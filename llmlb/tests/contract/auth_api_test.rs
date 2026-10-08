//! 認証API Contract Tests
//!
//! POST /api/auth/login, POST /api/auth/logout, GET /api/auth/me,
//! PUT /api/auth/change-password,
//! POST /api/auth/forgot-password, POST /api/auth/reset-password

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

async fn login(app: &Router, username: &str, password: &str) -> (StatusCode, Value) {
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

    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    (status, value)
}

async fn login_admin(app: &Router) -> String {
    let (status, body) = login(app, "admin", "password123").await;
    assert_eq!(status, StatusCode::OK);
    body["token"].as_str().unwrap().to_string()
}

fn bearer_request(jwt: &str) -> axum::http::request::Builder {
    Request::builder().header("authorization", format!("Bearer {}", jwt))
}

// ---------------------------------------------------------------------------
// POST /api/auth/login
// ---------------------------------------------------------------------------

/// DB登録ユーザーでのログイン成功
#[tokio::test]
#[serial]
async fn test_login_success_with_db_user() {
    let (app, _db_pool) = build_app().await;
    let (status, body) = login(&app, "admin", "password123").await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["token"].is_string());
    assert_eq!(body["expires_in"], 86400);
    assert_eq!(body["user"]["username"], "admin");
    assert_eq!(body["user"]["role"], "admin");
}

/// 開発モード固定ユーザー（admin/test）でログイン成功
#[tokio::test]
#[serial]
async fn test_login_success_dev_mode() {
    let (app, _db_pool) = build_app().await;
    let (status, body) = login(&app, "admin", "test").await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["token"].is_string());
    assert_eq!(body["user"]["username"], "admin");
}

/// 存在しないユーザーでログイン失敗
#[tokio::test]
#[serial]
async fn test_login_failure_unknown_user() {
    let (app, _db_pool) = build_app().await;
    let (status, _body) = login(&app, "unknown", "password123").await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// パスワード誤りでログイン失敗
#[tokio::test]
#[serial]
async fn test_login_failure_wrong_password() {
    let (app, _db_pool) = build_app().await;
    let (status, _body) = login(&app, "admin", "wrongpassword").await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// ユーザー名なしのリクエストは422
#[tokio::test]
#[serial]
async fn test_login_missing_username_returns_422() {
    let (app, _db_pool) = build_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "password": "pass" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

/// ログイン成功レスポンスにSet-Cookieヘッダーが含まれる
#[tokio::test]
#[serial]
async fn test_login_sets_cookie_headers() {
    let (app, _db_pool) = build_app().await;

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

    assert_eq!(response.status(), StatusCode::OK);
    let set_cookies: Vec<_> = response.headers().get_all("set-cookie").iter().collect();
    assert!(
        set_cookies.len() >= 2,
        "Should set jwt and csrf cookies, got {} cookies",
        set_cookies.len()
    );
}

/// must_change_passwordフラグがレスポンスに含まれる
#[tokio::test]
#[serial]
async fn test_login_must_change_password_flag() {
    let (app, db_pool) = crate::support::lb::create_test_lb().await;

    let password_hash = llmlb::auth::password::hash_password("changeme").unwrap();
    llmlb::db::users::create(&db_pool, "newuser", &password_hash, UserRole::Viewer, true)
        .await
        .ok();

    let (status, body) = login(&app, "newuser", "changeme").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["user"]["must_change_password"], true);
}

// ---------------------------------------------------------------------------
// POST /api/auth/logout
// ---------------------------------------------------------------------------

/// ログアウトは204を返す
#[tokio::test]
#[serial]
async fn test_logout_returns_no_content() {
    let (app, _db_pool) = build_app().await;
    let jwt = login_admin(&app).await;

    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("POST")
                .uri("/api/auth/logout")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

/// ログアウト時にCookieクリアヘッダーが含まれる
#[tokio::test]
#[serial]
async fn test_logout_clears_cookies() {
    let (app, _db_pool) = build_app().await;
    let jwt = login_admin(&app).await;

    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("POST")
                .uri("/api/auth/logout")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let set_cookies: Vec<_> = response.headers().get_all("set-cookie").iter().collect();
    assert!(
        !set_cookies.is_empty(),
        "Logout should set cookie-clearing headers"
    );
}

// ---------------------------------------------------------------------------
// GET /api/auth/me
// ---------------------------------------------------------------------------

/// 認証済みユーザー情報を取得
#[tokio::test]
#[serial]
async fn test_me_returns_user_info() {
    let (app, _db_pool) = build_app().await;
    let jwt = login_admin(&app).await;

    let response = app
        .oneshot(
            bearer_request(&jwt)
                .method("GET")
                .uri("/api/auth/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let data: Value = serde_json::from_slice(&body).unwrap();
    assert!(data["user_id"].is_string());
    assert!(data["username"].is_string());
    assert!(data["role"].is_string());
}

/// 認証なしで /api/auth/me は 401
#[tokio::test]
#[serial]
async fn test_me_requires_auth() {
    let (app, _db_pool) = build_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/auth/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// 無効なJWTで /api/auth/me は 401
#[tokio::test]
#[serial]
async fn test_me_invalid_jwt_returns_401() {
    let (app, _db_pool) = build_app().await;

    let response = app
        .oneshot(
            bearer_request("invalid-jwt-token")
                .method("GET")
                .uri("/api/auth/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// PUT /api/auth/change-password
// ---------------------------------------------------------------------------

/// パスワード変更成功
#[tokio::test]
#[serial]
async fn test_change_password_success() {
    let (app, _db_pool) = build_app().await;
    let jwt = login_admin(&app).await;

    let response = app
        .clone()
        .oneshot(
            bearer_request(&jwt)
                .method("PUT")
                .uri("/api/auth/change-password")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "current_password": "password123", "new_password": "Newpassword123" }))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

/// パスワードが短すぎると400
#[tokio::test]
#[serial]
async fn test_change_password_too_short() {
    let (app, _db_pool) = build_app().await;
    let jwt = login_admin(&app).await;

    let response = app
        .clone()
        .oneshot(
            bearer_request(&jwt)
                .method("PUT")
                .uri("/api/auth/change-password")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(
                        &json!({ "current_password": "password123", "new_password": "abc" }),
                    )
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// 認証なしでパスワード変更は401
#[tokio::test]
#[serial]
async fn test_change_password_requires_auth() {
    let (app, _db_pool) = build_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/auth/change-password")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "new_password": "Newpassword123" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// パスワード変更後に新パスワードでログイン可能
#[tokio::test]
#[serial]
async fn test_change_password_then_login_with_new() {
    let (app, _db_pool) = build_app().await;
    let jwt = login_admin(&app).await;

    // パスワード変更
    let response = app
        .clone()
        .oneshot(
            bearer_request(&jwt)
                .method("PUT")
                .uri("/api/auth/change-password")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "current_password": "password123", "new_password": "Newpassword456" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // 新パスワードでログイン
    let (status, _body) = login(&app, "admin", "Newpassword456").await;
    assert_eq!(status, StatusCode::OK);
}

/// must_change_passwordユーザーがパスワード変更後にフラグが解除される
#[tokio::test]
#[serial]
async fn test_change_password_clears_must_change_flag() {
    let (app, db_pool) = crate::support::lb::create_test_lb().await;

    let password_hash = llmlb::auth::password::hash_password("temppass1").unwrap();
    llmlb::db::users::create(&db_pool, "forced", &password_hash, UserRole::Admin, true)
        .await
        .ok();

    // must_change_password = true でログイン
    let (status, body) = login(&app, "forced", "temppass1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["user"]["must_change_password"], true);
    let jwt = body["token"].as_str().unwrap();

    // パスワード変更
    let response = app
        .clone()
        .oneshot(
            bearer_request(jwt)
                .method("PUT")
                .uri("/api/auth/change-password")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(
                        &json!({ "current_password": "temppass1", "new_password": "Newpass123" }),
                    )
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // 再ログインでフラグ解除を確認
    let (status, body) = login(&app, "forced", "Newpass123").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["user"]["must_change_password"], false);
}

// ---------------------------------------------------------------------------
// セッション無効化（password_changed_at）
// ---------------------------------------------------------------------------

/// 自分でパスワードを変更すると、旧トークンは無効化され、レスポンスの新トークンが有効になる
#[tokio::test]
#[serial]
async fn test_session_revoked_after_self_password_change() {
    let (app, _db_pool) = build_app().await;
    let token1 = login_admin(&app).await;

    // 旧トークンで /api/auth/me は 200
    let me1 = app
        .clone()
        .oneshot(
            bearer_request(&token1)
                .method("GET")
                .uri("/api/auth/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me1.status(), StatusCode::OK);

    // パスワード変更（新トークンが返る）
    let resp = app
        .clone()
        .oneshot(
            bearer_request(&token1)
                .method("PUT")
                .uri("/api/auth/change-password")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "current_password": "password123", "new_password": "NewAdminPass123" })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    let token2 = body["token"]
        .as_str()
        .expect("change-password should return a fresh token")
        .to_string();

    // 旧トークンは無効化され 401
    let me_old = app
        .clone()
        .oneshot(
            bearer_request(&token1)
                .method("GET")
                .uri("/api/auth/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        me_old.status(),
        StatusCode::UNAUTHORIZED,
        "old token must be revoked after password change"
    );

    // 新トークンは有効
    let me_new = app
        .oneshot(
            bearer_request(&token2)
                .method("GET")
                .uri("/api/auth/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me_new.status(), StatusCode::OK);
}

/// 管理者が他ユーザーのパスワードをリセットすると、そのユーザーの既存トークンが無効化される
#[tokio::test]
#[serial]
async fn test_session_revoked_after_admin_password_reset() {
    let (app, db_pool) = build_app().await;

    // viewer ユーザーを作成
    let vhash = llmlb::auth::password::hash_password("viewerpass123").unwrap();
    let viewer = llmlb::db::users::create(&db_pool, "victim", &vhash, UserRole::Viewer, false)
        .await
        .unwrap();

    // viewer でログイン
    let (status, body) = login(&app, "victim", "viewerpass123").await;
    assert_eq!(status, StatusCode::OK);
    let token_v = body["token"].as_str().unwrap().to_string();

    // 旧トークンで /api/auth/me は 200
    let me1 = app
        .clone()
        .oneshot(
            bearer_request(&token_v)
                .method("GET")
                .uri("/api/auth/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me1.status(), StatusCode::OK);

    // 管理者が viewer のパスワードをリセット
    let admin_token = login_admin(&app).await;
    let reset = app
        .clone()
        .oneshot(
            bearer_request(&admin_token)
                .method("PUT")
                .uri(format!("/api/users/{}", viewer.id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "password": "ResetPass123" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reset.status(), StatusCode::OK);

    // viewer の旧トークンは無効化され 401
    let me_after = app
        .oneshot(
            bearer_request(&token_v)
                .method("GET")
                .uri("/api/auth/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        me_after.status(),
        StatusCode::UNAUTHORIZED,
        "victim token must be revoked after admin password reset"
    );
}

// ---------------------------------------------------------------------------
// 現在パスワード検証（SPEC #580 AS-017）
// ---------------------------------------------------------------------------

async fn send_json(
    app: &Router,
    method: &str,
    uri: &str,
    jwt: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let builder = match jwt {
        Some(jwt) => bearer_request(jwt),
        None => Request::builder(),
    };
    let response = app
        .clone()
        .oneshot(
            builder
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

async fn reset_token_count(db_pool: &SqlitePool, user_id: uuid::Uuid) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM password_reset_tokens WHERE user_id = ?")
        .bind(user_id.to_string())
        .fetch_one(db_pool)
        .await
        .unwrap()
}

/// forgot-password はトークンを応答後に非同期発行するため、件数が期待値になるまで待つ
async fn wait_for_reset_token_count(db_pool: &SqlitePool, user_id: uuid::Uuid, expected: i64) {
    for _ in 0..100 {
        if reset_token_count(db_pool, user_id).await == expected {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(reset_token_count(db_pool, user_id).await, expected);
}

async fn create_email_user(db_pool: &SqlitePool, email: &str, password: &str) -> uuid::Uuid {
    let hash = llmlb::auth::password::hash_password(password).unwrap();
    llmlb::db::users::create(db_pool, email, &hash, UserRole::Viewer, false)
        .await
        .unwrap()
        .id
}

/// 現在のパスワードが誤っている場合は400で、パスワードは変更されない
#[tokio::test]
#[serial]
async fn test_change_password_rejects_wrong_current_password() {
    let (app, _db_pool) = build_app().await;
    let jwt = login_admin(&app).await;

    let (status, _) = send_json(
        &app,
        "PUT",
        "/api/auth/change-password",
        Some(&jwt),
        json!({ "current_password": "wrongpassword", "new_password": "Newpassword789" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = login(&app, "admin", "password123").await;
    assert_eq!(status, StatusCode::OK, "old password must still work");
    let (status, _) = login(&app, "admin", "Newpassword789").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// current_password を省略したリクエストは拒否される（422）
#[tokio::test]
#[serial]
async fn test_change_password_requires_current_password_field() {
    let (app, _db_pool) = build_app().await;
    let jwt = login_admin(&app).await;

    let (status, _) = send_json(
        &app,
        "PUT",
        "/api/auth/change-password",
        Some(&jwt),
        json!({ "new_password": "Newpassword789" }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, _) = login(&app, "admin", "password123").await;
    assert_eq!(status, StatusCode::OK);
}

// ---------------------------------------------------------------------------
// POST /api/auth/forgot-password（SPEC #580 AS-014）
// ---------------------------------------------------------------------------

/// 存在するユーザーのメールIDでリセットトークンが発行される（202）
#[tokio::test]
#[serial]
async fn test_forgot_password_issues_reset_token() {
    let (app, db_pool) = build_app().await;
    let user_id = create_email_user(&db_pool, "alice@example.com", "Password123").await;

    let (status, body) = send_json(
        &app,
        "POST",
        "/api/auth/forgot-password",
        None,
        json!({ "email": "alice@example.com" }),
    )
    .await;

    assert_eq!(status, StatusCode::ACCEPTED);
    assert!(body["message"].is_string());
    assert!(
        body.get("token").is_none(),
        "token must never be returned to an unauthenticated caller"
    );
    wait_for_reset_token_count(&db_pool, user_id, 1).await;

    // 平文トークンはDBに保存しない（ハッシュのみ）
    let stored: String =
        sqlx::query_scalar("SELECT token_hash FROM password_reset_tokens WHERE user_id = ?")
            .bind(user_id.to_string())
            .fetch_one(&db_pool)
            .await
            .unwrap();
    assert_eq!(stored.len(), 64, "SHA-256 hex digest expected");
}

/// 存在しないメールIDでも同じ202応答を返し（アカウント列挙防止）、トークンは作られない
#[tokio::test]
#[serial]
async fn test_forgot_password_unknown_email_is_indistinguishable() {
    let (app, db_pool) = build_app().await;
    let user_id = create_email_user(&db_pool, "alice@example.com", "Password123").await;

    let (known_status, known_body) = send_json(
        &app,
        "POST",
        "/api/auth/forgot-password",
        None,
        json!({ "email": "alice@example.com" }),
    )
    .await;
    let (unknown_status, unknown_body) = send_json(
        &app,
        "POST",
        "/api/auth/forgot-password",
        None,
        json!({ "email": "nobody@example.com" }),
    )
    .await;

    assert_eq!(known_status, StatusCode::ACCEPTED);
    assert_eq!(unknown_status, StatusCode::ACCEPTED);
    assert_eq!(known_body, unknown_body);
    wait_for_reset_token_count(&db_pool, user_id, 1).await;
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM password_reset_tokens")
        .fetch_one(&db_pool)
        .await
        .unwrap();
    assert_eq!(total, 1);
}

/// 再発行すると同ユーザーの以前のトークンは失効する
#[tokio::test]
#[serial]
async fn test_forgot_password_reissue_invalidates_previous_token() {
    let (app, db_pool) = build_app().await;
    let user_id = create_email_user(&db_pool, "alice@example.com", "Password123").await;

    let first =
        llmlb::db::password_reset_tokens::issue(&db_pool, user_id, chrono::Duration::minutes(30))
            .await
            .unwrap();
    let second =
        llmlb::db::password_reset_tokens::issue(&db_pool, user_id, chrono::Duration::minutes(30))
            .await
            .unwrap();
    assert_ne!(first, second);

    let (status, _) = send_json(
        &app,
        "POST",
        "/api/auth/reset-password",
        None,
        json!({ "token": first, "new_password": "Resetpass123" }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "superseded token must be rejected"
    );

    let (status, _) = send_json(
        &app,
        "POST",
        "/api/auth/reset-password",
        None,
        json!({ "token": second, "new_password": "Resetpass123" }),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

// ---------------------------------------------------------------------------
// POST /api/auth/reset-password（SPEC #580 AS-015 / AS-016）
// ---------------------------------------------------------------------------

/// 有効なトークンでパスワードが更新され、トークンは無効化される
#[tokio::test]
#[serial]
async fn test_reset_password_with_valid_token() {
    let (app, db_pool) = build_app().await;
    let user_id = create_email_user(&db_pool, "alice@example.com", "Password123").await;
    let token =
        llmlb::db::password_reset_tokens::issue(&db_pool, user_id, chrono::Duration::minutes(30))
            .await
            .unwrap();

    let (status, _) = send_json(
        &app,
        "POST",
        "/api/auth/reset-password",
        None,
        json!({ "token": token, "new_password": "Resetpass123" }),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body) = login(&app, "alice@example.com", "Resetpass123").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["user"]["must_change_password"], false);
    let (status, _) = login(&app, "alice@example.com", "Password123").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // 同じトークンの再利用は拒否（単回使用）
    let (status, _) = send_json(
        &app,
        "POST",
        "/api/auth/reset-password",
        None,
        json!({ "token": token, "new_password": "Another123" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = login(&app, "alice@example.com", "Resetpass123").await;
    assert_eq!(status, StatusCode::OK);
}

/// リセット後はそのユーザーの既存セッションが無効化される
#[tokio::test]
#[serial]
async fn test_reset_password_revokes_existing_sessions() {
    let (app, db_pool) = build_app().await;
    let user_id = create_email_user(&db_pool, "alice@example.com", "Password123").await;
    let (status, body) = login(&app, "alice@example.com", "Password123").await;
    assert_eq!(status, StatusCode::OK);
    let old_jwt = body["token"].as_str().unwrap().to_string();

    let token =
        llmlb::db::password_reset_tokens::issue(&db_pool, user_id, chrono::Duration::minutes(30))
            .await
            .unwrap();
    let (status, _) = send_json(
        &app,
        "POST",
        "/api/auth/reset-password",
        None,
        json!({ "token": token, "new_password": "Resetpass123" }),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let me = app
        .clone()
        .oneshot(
            bearer_request(&old_jwt)
                .method("GET")
                .uri("/api/auth/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me.status(), StatusCode::UNAUTHORIZED);
}

/// 期限切れトークンではエラーになり、パスワードは変更されない
#[tokio::test]
#[serial]
async fn test_reset_password_with_expired_token() {
    let (app, db_pool) = build_app().await;
    let user_id = create_email_user(&db_pool, "alice@example.com", "Password123").await;
    let token =
        llmlb::db::password_reset_tokens::issue(&db_pool, user_id, chrono::Duration::seconds(-1))
            .await
            .unwrap();

    let (status, _) = send_json(
        &app,
        "POST",
        "/api/auth/reset-password",
        None,
        json!({ "token": token, "new_password": "Resetpass123" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = login(&app, "alice@example.com", "Password123").await;
    assert_eq!(status, StatusCode::OK, "password must stay unchanged");
}

/// 不明なトークンではエラー
#[tokio::test]
#[serial]
async fn test_reset_password_with_unknown_token() {
    let (app, _db_pool) = build_app().await;

    let (status, _) = send_json(
        &app,
        "POST",
        "/api/auth/reset-password",
        None,
        json!({ "token": "not-a-real-token", "new_password": "Resetpass123" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// 弱いパスワードは拒否され、トークンは消費されない
#[tokio::test]
#[serial]
async fn test_reset_password_weak_password_keeps_token_usable() {
    let (app, db_pool) = build_app().await;
    let user_id = create_email_user(&db_pool, "alice@example.com", "Password123").await;
    let token =
        llmlb::db::password_reset_tokens::issue(&db_pool, user_id, chrono::Duration::minutes(30))
            .await
            .unwrap();

    let (status, _) = send_json(
        &app,
        "POST",
        "/api/auth/reset-password",
        None,
        json!({ "token": token, "new_password": "weak" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = send_json(
        &app,
        "POST",
        "/api/auth/reset-password",
        None,
        json!({ "token": token, "new_password": "Resetpass123" }),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}
