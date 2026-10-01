//! AC-2: `users.email`（通知先。ログイン識別子ではない）
//!
//! SPEC #777 FR-001 / T-001 / T-002（API 部分）

use crate::helpers::{
    build_app_with_admin, create_login_user, create_user_with_jwt, send_json, TEST_PASSWORD,
};
use axum::http::{Method, StatusCode};
use llmlb::common::auth::UserRole;
use serde_json::{json, Value};
use serial_test::serial;
use sqlx::{migrate::Migrator, Row, SqlitePool};
use std::borrow::Cow;

/// `users.email` を追加するマイグレーションのバージョン
const EMAIL_MIGRATION_VERSION: i64 = 20261001043458;

fn find_user<'a>(users: &'a Value, username: &str) -> &'a Value {
    users["users"]
        .as_array()
        .expect("users array")
        .iter()
        .find(|user| user["username"] == username)
        .unwrap_or_else(|| panic!("user {username} not found in {users}"))
}

#[tokio::test]
#[serial]
async fn ac2_users_table_has_nullable_email_column() {
    let pool = crate::support::lb::create_test_db_pool().await;

    let columns = sqlx::query("PRAGMA table_info(users)")
        .fetch_all(&pool)
        .await
        .unwrap();
    let email = columns
        .iter()
        .find(|row| row.get::<String, _>("name") == "email")
        .expect("users.email column must exist");

    assert_eq!(email.get::<i64, _>("notnull"), 0, "email must be nullable");
    assert!(
        email.get::<Option<String>, _>("dflt_value").is_none(),
        "email must default to NULL"
    );
}

/// email カラムが無い時代に作られたユーザーが、マイグレーション後も email NULL のまま
/// ログイン・自身の情報取得・ユーザー管理を行えること
#[tokio::test]
#[serial]
async fn ac2_existing_user_keeps_working_with_null_email_after_migration() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let legacy_migrations: Vec<_> = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < EMAIL_MIGRATION_VERSION)
        .cloned()
        .collect();
    Migrator {
        migrations: Cow::Owned(legacy_migrations),
        ..Migrator::DEFAULT
    }
    .run(&pool)
    .await
    .unwrap();

    let password_hash = llmlb::auth::password::hash_password(TEST_PASSWORD).unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, password_hash, role, created_at, last_login, must_change_password, password_changed_at)
         VALUES (?, 'legacy-admin', ?, 'admin', '2026-01-01T00:00:00+00:00', NULL, 0, 0)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(&password_hash)
    .execute(&pool)
    .await
    .unwrap();

    llmlb::db::migrations::run_migrations(&pool).await.unwrap();
    let app = crate::support::lb::build_test_router(pool.clone()).await;

    let (status, login) = send_json(
        &app,
        Method::POST,
        "/api/auth/login",
        None,
        Some(json!({ "username": "legacy-admin", "password": TEST_PASSWORD })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "login failed: {login}");
    let jwt = login["token"].as_str().expect("login token").to_string();

    let (status, me) = send_json(&app, Method::GET, "/api/auth/me", Some(&jwt), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["username"], "legacy-admin");

    let (status, users) = send_json(&app, Method::GET, "/api/users", Some(&jwt), None).await;
    assert_eq!(status, StatusCode::OK);
    let legacy = find_user(&users, "legacy-admin");
    assert!(
        legacy.as_object().unwrap().contains_key("email"),
        "user response must expose the email field: {legacy}"
    );
    assert_eq!(legacy["email"], Value::Null);

    // email を持たない管理者がユーザー管理（作成・更新・削除）を行える
    let (status, created) = send_json(
        &app,
        Method::POST,
        "/api/users",
        Some(&jwt),
        Some(json!({ "username": "made-by-legacy@example.com", "role": "viewer" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let created_id = created["user"]["id"].as_str().unwrap().to_string();

    let (status, updated) = send_json(
        &app,
        Method::PUT,
        &format!("/api/users/{created_id}"),
        Some(&jwt),
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["role"], "admin");

    let (status, _) = send_json(
        &app,
        Method::DELETE,
        &format!("/api/users/{created_id}"),
        Some(&jwt),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
#[serial]
async fn ac2_create_user_accepts_optional_email() {
    let (app, _pool, jwt) = build_app_with_admin().await;

    let (status, with_email) = send_json(
        &app,
        Method::POST,
        "/api/users",
        Some(&jwt),
        Some(json!({ "username": "ops@example.com", "role": "admin", "email": "ops-alerts@example.net" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{with_email}");
    assert_eq!(with_email["user"]["email"], "ops-alerts@example.net");

    let (status, without_email) = send_json(
        &app,
        Method::POST,
        "/api/users",
        Some(&jwt),
        Some(json!({ "username": "no-mail@example.com", "role": "viewer" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{without_email}");
    assert_eq!(without_email["user"]["email"], Value::Null);

    let (status, users) = send_json(&app, Method::GET, "/api/users", Some(&jwt), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        find_user(&users, "ops@example.com")["email"],
        "ops-alerts@example.net"
    );
    assert_eq!(
        find_user(&users, "no-mail@example.com")["email"],
        Value::Null
    );
}

#[tokio::test]
#[serial]
async fn ac2_update_user_sets_changes_and_clears_email() {
    let (app, _pool, jwt) = build_app_with_admin().await;
    let (_, created) = send_json(
        &app,
        Method::POST,
        "/api/users",
        Some(&jwt),
        Some(json!({ "username": "target@example.com", "role": "viewer" })),
    )
    .await;
    let uri = format!("/api/users/{}", created["user"]["id"].as_str().unwrap());

    let (status, set) = send_json(
        &app,
        Method::PUT,
        &uri,
        Some(&jwt),
        Some(json!({ "email": "  first@example.com " })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{set}");
    assert_eq!(
        set["email"], "first@example.com",
        "surrounding whitespace is trimmed"
    );

    let (status, changed) = send_json(
        &app,
        Method::PUT,
        &uri,
        Some(&jwt),
        Some(json!({ "email": "second@example.com" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(changed["email"], "second@example.com");

    // email を含まない更新は email を変更しない
    let (status, role_only) = send_json(
        &app,
        Method::PUT,
        &uri,
        Some(&jwt),
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(role_only["role"], "admin");
    assert_eq!(role_only["email"], "second@example.com");

    // 空文字で通知先を解除する
    let (status, cleared) = send_json(
        &app,
        Method::PUT,
        &uri,
        Some(&jwt),
        Some(json!({ "email": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cleared["email"], Value::Null);

    let (_, users) = send_json(&app, Method::GET, "/api/users", Some(&jwt), None).await;
    assert_eq!(
        find_user(&users, "target@example.com")["email"],
        Value::Null
    );
}

#[tokio::test]
#[serial]
async fn ac2_invalid_email_is_rejected() {
    let (app, _pool, jwt) = build_app_with_admin().await;

    for invalid in ["not-an-address", "a@", "@example.com", "a b@example.com"] {
        let (status, body) = send_json(
            &app,
            Method::POST,
            "/api/users",
            Some(&jwt),
            Some(json!({ "username": "bad-mail@example.com", "role": "viewer", "email": invalid })),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "create must reject {invalid:?}: {body}"
        );
    }

    let (_, users) = send_json(&app, Method::GET, "/api/users", Some(&jwt), None).await;
    assert!(
        users["users"]
            .as_array()
            .unwrap()
            .iter()
            .all(|user| user["username"] != "bad-mail@example.com"),
        "a rejected request must not create the user"
    );

    let (_, created) = send_json(
        &app,
        Method::POST,
        "/api/users",
        Some(&jwt),
        Some(json!({ "username": "keeps-mail@example.com", "role": "viewer", "email": "keep@example.com" })),
    )
    .await;
    let uri = format!("/api/users/{}", created["user"]["id"].as_str().unwrap());
    let (status, _) = send_json(
        &app,
        Method::PUT,
        &uri,
        Some(&jwt),
        Some(json!({ "email": "not-an-address" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (_, users) = send_json(&app, Method::GET, "/api/users", Some(&jwt), None).await;
    assert_eq!(
        find_user(&users, "keeps-mail@example.com")["email"],
        "keep@example.com"
    );
}

/// email は通知先であり、ログイン識別子ではない
#[tokio::test]
#[serial]
async fn ac2_email_is_not_a_login_identifier() {
    let (app, pool, jwt) = build_app_with_admin().await;
    create_login_user(&pool, "alice@example.com", UserRole::Viewer).await;
    create_user_with_jwt(&pool, "bob@example.com", UserRole::Viewer).await;

    let (_, users) = send_json(&app, Method::GET, "/api/users", Some(&jwt), None).await;
    for username in ["alice@example.com", "bob@example.com"] {
        // 通知先は識別子ではないので、複数ユーザーが同じアドレスを共有できる
        let id = find_user(&users, username)["id"].as_str().unwrap();
        let (status, body) = send_json(
            &app,
            Method::PUT,
            &format!("/api/users/{id}"),
            Some(&jwt),
            Some(json!({ "email": "shared-alerts@example.net" })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["email"], "shared-alerts@example.net");
    }

    let (status, _) = send_json(
        &app,
        Method::POST,
        "/api/auth/login",
        None,
        Some(json!({ "username": "shared-alerts@example.net", "password": TEST_PASSWORD })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "email must not be accepted as a login identifier"
    );

    let (status, _) = send_json(
        &app,
        Method::POST,
        "/api/auth/login",
        None,
        Some(json!({ "username": "alice@example.com", "password": TEST_PASSWORD })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "username login keeps working");
}

/// viewer は他ユーザーの通知先を変更できない
#[tokio::test]
#[serial]
async fn ac2_viewer_cannot_change_email() {
    let (app, pool, admin_jwt) = build_app_with_admin().await;
    let viewer_jwt = create_user_with_jwt(&pool, "viewer", UserRole::Viewer).await;

    let (_, users) = send_json(&app, Method::GET, "/api/users", Some(&admin_jwt), None).await;
    let id = find_user(&users, "viewer")["id"].as_str().unwrap();
    let (status, _) = send_json(
        &app,
        Method::PUT,
        &format!("/api/users/{id}"),
        Some(&viewer_jwt),
        Some(json!({ "email": "viewer@example.com" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
