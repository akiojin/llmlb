//! Issue #913: 未承認アカウントには権限を与えない登録申請契約。

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use llmlb::common::auth::{ApiKeyPermission, UserRole};
use serde_json::{json, Value};
use serial_test::serial;
use sqlx::SqlitePool;
use tower::ServiceExt;
use uuid::Uuid;

const PASSWORD: &str = "PendingPass123";

async fn setup() -> (Router, SqlitePool, String, String) {
    let (app, pool) = crate::support::lb::create_test_lb().await;
    let hash = llmlb::auth::password::hash_password(PASSWORD).unwrap();
    let admin = llmlb::db::users::create(&pool, "admin@example.com", &hash, UserRole::Admin, false)
        .await
        .unwrap();
    let viewer =
        llmlb::db::users::create(&pool, "viewer@example.com", &hash, UserRole::Viewer, false)
            .await
            .unwrap();
    let secret = crate::support::lb::test_jwt_secret();
    let token = |id: Uuid, role| {
        llmlb::auth::jwt::create_jwt(&id.to_string(), role, &secret, false, 0).unwrap()
    };
    (
        app,
        pool,
        token(admin.id, UserRole::Admin),
        token(viewer.id, UserRole::Viewer),
    )
}

async fn request(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn submit(app: &Router, username: &str) -> (StatusCode, Value) {
    request(
        app,
        "POST",
        "/api/auth/register",
        None,
        json!({"username": username, "password": PASSWORD}),
    )
    .await
}

async fn pending_id(app: &Router, token: &str) -> String {
    let (status, rows) = request(
        app,
        "GET",
        "/api/registration-requests",
        Some(token),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    rows[0]["id"].as_str().unwrap().to_string()
}

#[tokio::test]
#[serial]
async fn registration_requests_create_pending_without_user_or_session() {
    let (app, pool, admin, _) = setup().await;
    let (status, body) = submit(&app, "applicant@example.com").await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(body["status"], "pending");
    assert!(body["message"].is_string());
    assert!(body.get("token").is_none());
    assert!(
        llmlb::db::users::find_by_username(&pool, "applicant@example.com")
            .await
            .unwrap()
            .is_none()
    );
    let (status, rows) = request(
        &app,
        "GET",
        "/api/registration-requests",
        Some(&admin),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rows.as_array().unwrap().len(), 1);
    assert_eq!(rows[0]["username"], "applicant@example.com");
    assert!(rows[0]["created_at"].is_string());
    assert_eq!(
        rows[0].as_object().unwrap().len(),
        3,
        "list must not disclose password hashes"
    );
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM registration_requests")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_ne!(hash, PASSWORD);
    assert!(llmlb::auth::password::verify_password(PASSWORD, &hash).unwrap());
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/auth/login",
            None,
            json!({"username":"applicant@example.com", "password":PASSWORD})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    for uri in ["/api/auth/me", "/api/users", "/api/me/api-keys"] {
        assert_eq!(
            request(&app, "GET", uri, None, Value::Null).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
}

#[tokio::test]
#[serial]
async fn registration_requests_approve_each_explicit_role_and_login() {
    for role in ["admin", "viewer"] {
        let (app, pool, admin, _) = setup().await;
        assert_eq!(
            submit(&app, "approved@example.com").await.0,
            StatusCode::ACCEPTED
        );
        let id = pending_id(&app, &admin).await;
        let uri = format!("/api/registration-requests/{id}/approve");
        assert_eq!(
            request(&app, "POST", &uri, Some(&admin), json!({})).await.0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(
            request(&app, "POST", &uri, Some(&admin), json!({"role":"owner"}))
                .await
                .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
        let (status, user) = request(&app, "POST", &uri, Some(&admin), json!({"role":role})).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(user["role"], role);
        assert_eq!(user["username"], "approved@example.com");
        assert!(user.get("password_hash").is_none());
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM registration_requests")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        assert_eq!(
            request(&app, "POST", &uri, Some(&admin), json!({"role":role}))
                .await
                .0,
            StatusCode::NOT_FOUND
        );
        let (status, login) = request(
            &app,
            "POST",
            "/api/auth/login",
            None,
            json!({"username":"approved@example.com","password":PASSWORD}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(login["user"]["role"], role);
        assert_eq!(login["user"]["must_change_password"], false);
    }
}

#[tokio::test]
#[serial]
async fn registration_requests_reject_allows_resubmission() {
    let (app, pool, admin, _) = setup().await;
    assert_eq!(
        submit(&app, "rejected@example.com").await.0,
        StatusCode::ACCEPTED
    );
    let id = pending_id(&app, &admin).await;
    let uri = format!("/api/registration-requests/{id}");
    assert_eq!(
        request(&app, "DELETE", &uri, Some(&admin), Value::Null)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(&app, "DELETE", &uri, Some(&admin), Value::Null)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert!(
        llmlb::db::users::find_by_username(&pool, "rejected@example.com")
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        submit(&app, "rejected@example.com").await.0,
        StatusCode::ACCEPTED
    );
}

#[tokio::test]
#[serial]
async fn registration_requests_require_users_manage_permission() {
    let (app, pool, admin, viewer) = setup().await;
    for (method, uri) in [
        ("GET", "/api/registration-requests".to_string()),
        (
            "POST",
            format!("/api/registration-requests/{}/approve", Uuid::new_v4()),
        ),
        (
            "DELETE",
            format!("/api/registration-requests/{}", Uuid::new_v4()),
        ),
    ] {
        assert_eq!(
            request(&app, method, &uri, None, json!({"role":"viewer"}))
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            request(&app, method, &uri, Some(&viewer), json!({"role":"viewer"}))
                .await
                .0,
            StatusCode::FORBIDDEN
        );
    }
    let id = llmlb::db::users::find_by_username(&pool, "admin@example.com")
        .await
        .unwrap()
        .unwrap()
        .id;
    for (name, permissions, expected) in [
        (
            "allowed",
            vec![ApiKeyPermission::UsersManage],
            StatusCode::OK,
        ),
        (
            "denied",
            vec![ApiKeyPermission::OpenaiInference],
            StatusCode::FORBIDDEN,
        ),
    ] {
        let key = llmlb::db::api_keys::create(&pool, name, id, None, permissions)
            .await
            .unwrap();
        assert_eq!(
            request(
                &app,
                "GET",
                "/api/registration-requests",
                Some(&key.key),
                Value::Null
            )
            .await
            .0,
            expected
        );
    }
    assert_eq!(
        request(&app, "GET", "/api/invitations", Some(&admin), Value::Null)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
#[serial]
async fn registration_requests_validate_and_reserve_usernames() {
    let (app, pool, admin, _) = setup().await;
    for body in [
        json!({"username":"", "password":PASSWORD}),
        json!({"username":"not-an-email","password":PASSWORD}),
        json!({"username":"valid@example.com","password":"weak"}),
    ] {
        assert_eq!(
            request(&app, "POST", "/api/auth/register", None, body)
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        submit(&app, "admin@example.com").await.0,
        StatusCode::CONFLICT
    );
    let (one, two) = tokio::join!(
        submit(&app, "unique@example.com"),
        submit(&app, "unique@example.com")
    );
    assert!([one.0, two.0].contains(&StatusCode::ACCEPTED));
    assert!([one.0, two.0].contains(&StatusCode::CONFLICT));
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/users",
            Some(&admin),
            json!({"username":"unique@example.com","role":"viewer"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let viewer_id = llmlb::db::users::find_by_username(&pool, "viewer@example.com")
        .await
        .unwrap()
        .unwrap()
        .id;
    assert_eq!(
        request(
            &app,
            "PUT",
            &format!("/api/users/{viewer_id}"),
            Some(&admin),
            json!({"username":"unique@example.com"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
#[serial]
async fn registration_requests_enforce_cap_atomically() {
    let (app, pool, _, _) = setup().await;
    for index in 0..99 {
        sqlx::query("INSERT INTO registration_requests (id, username, password_hash, created_at) VALUES (?, ?, 'hash', '2026-10-15T00:00:00Z')")
            .bind(Uuid::new_v4().to_string()).bind(format!("pending{index}@example.com")).execute(&pool).await.unwrap();
    }
    let (one, two) = tokio::join!(
        submit(&app, "last1@example.com"),
        submit(&app, "last2@example.com")
    );
    assert!([one.0, two.0].contains(&StatusCode::ACCEPTED));
    assert!([one.0, two.0].contains(&StatusCode::TOO_MANY_REQUESTS));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM registration_requests")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 100);
}

#[tokio::test]
#[serial]
async fn registration_requests_last_admin_survives_demotion_and_competing_deletes() {
    let (app, pool, admin, _) = setup().await;
    let first = llmlb::db::users::find_by_username(&pool, "admin@example.com")
        .await
        .unwrap()
        .unwrap();
    let uri = format!("/api/users/{}", first.id);
    assert_eq!(
        request(&app, "PUT", &uri, Some(&admin), json!({"role":"viewer"}))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let second =
        llmlb::db::users::create(&pool, "second@example.com", "hash", UserRole::Admin, false)
            .await
            .unwrap();
    let (one, two) = tokio::join!(
        llmlb::db::users::delete(&pool, first.id),
        llmlb::db::users::delete(&pool, second.id)
    );
    assert!(one.is_ok() ^ two.is_ok());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE role = 'admin'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
#[serial]
async fn registration_requests_approval_rolls_back_when_user_insert_fails() {
    let (app, pool, admin, _) = setup().await;
    assert_eq!(
        submit(&app, "rollback@example.com").await.0,
        StatusCode::ACCEPTED
    );
    let id = pending_id(&app, &admin).await;
    sqlx::query("CREATE TRIGGER fail_approval BEFORE INSERT ON users WHEN NEW.username = 'rollback@example.com' BEGIN SELECT RAISE(ABORT, 'forced failure'); END").execute(&pool).await.unwrap();
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("/api/registration-requests/{id}/approve"),
            Some(&admin),
            json!({"role":"viewer"})
        )
        .await
        .0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM registration_requests WHERE id = ?")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
#[serial]
async fn registration_requests_demotion_revokes_existing_admin_session() {
    let (app, pool, admin, _) = setup().await;
    llmlb::db::users::create(
        &pool,
        "remaining@example.com",
        "hash",
        UserRole::Admin,
        false,
    )
    .await
    .unwrap();
    let id = llmlb::db::users::find_by_username(&pool, "admin@example.com")
        .await
        .unwrap()
        .unwrap()
        .id;
    assert_eq!(
        request(
            &app,
            "PUT",
            &format!("/api/users/{id}"),
            Some(&admin),
            json!({"role":"viewer"})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        request(
            &app,
            "GET",
            "/api/registration-requests",
            Some(&admin),
            Value::Null
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, login) = request(
        &app,
        "POST",
        "/api/auth/login",
        None,
        json!({"username":"admin@example.com", "password":PASSWORD}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(login["user"]["role"], "viewer");
    assert_eq!(
        request(
            &app,
            "GET",
            "/api/registration-requests",
            login["token"].as_str(),
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
}

/// 承認制は管理画面/APIの契約。既存 /v1 APIキーの認証・権限を変更しない。
#[tokio::test]
#[serial]
async fn registration_requests_leave_existing_v1_api_key_contract_unchanged() {
    let (app, pool, admin, _) = setup().await;
    let viewer = llmlb::db::users::find_by_username(&pool, "viewer@example.com")
        .await
        .unwrap()
        .unwrap();
    let allowed = llmlb::db::api_keys::create(
        &pool,
        "existing-viewer-key",
        viewer.id,
        None,
        vec![
            ApiKeyPermission::OpenaiInference,
            ApiKeyPermission::OpenaiModelsRead,
        ],
    )
    .await
    .unwrap();
    let inference_only = llmlb::db::api_keys::create(
        &pool,
        "existing-inference-only-key",
        viewer.id,
        None,
        vec![ApiKeyPermission::OpenaiInference],
    )
    .await
    .unwrap();

    for state in ["before", "pending", "rejected", "approved_viewer"] {
        match state {
            "pending" => {
                assert_eq!(
                    submit(&app, "upstream-applicant@example.com").await.0,
                    StatusCode::ACCEPTED
                );
            }
            "rejected" => {
                let id = pending_id(&app, &admin).await;
                assert_eq!(
                    request(
                        &app,
                        "DELETE",
                        &format!("/api/registration-requests/{id}"),
                        Some(&admin),
                        Value::Null
                    )
                    .await
                    .0,
                    StatusCode::NO_CONTENT
                );
            }
            "approved_viewer" => {
                assert_eq!(
                    submit(&app, "upstream-applicant@example.com").await.0,
                    StatusCode::ACCEPTED
                );
                let id = pending_id(&app, &admin).await;
                assert_eq!(
                    request(
                        &app,
                        "POST",
                        &format!("/api/registration-requests/{id}/approve"),
                        Some(&admin),
                        json!({"role":"viewer"})
                    )
                    .await
                    .0,
                    StatusCode::CREATED
                );
            }
            _ => {}
        }
        let (status, models) =
            request(&app, "GET", "/v1/models", Some(&allowed.key), Value::Null).await;
        assert_eq!(status, StatusCode::OK, "registration state: {state}");
        assert_eq!(models["object"], "list");
        assert!(models["data"].is_array());
        assert_eq!(
            request(
                &app,
                "GET",
                "/v1/models",
                Some(&inference_only.key),
                Value::Null
            )
            .await
            .0,
            StatusCode::FORBIDDEN,
            "models scope must still be required: {state}"
        );
        assert_eq!(
            request(&app, "GET", "/v1/models", None, Value::Null)
                .await
                .0,
            StatusCode::UNAUTHORIZED,
            "API key authentication must still be required: {state}"
        );
    }
}
