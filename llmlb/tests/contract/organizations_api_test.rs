//! Issue #947: 組織管理、所属、認可とキー利用帰属の実 handler 契約。

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

struct Fixture {
    app: Router,
    pool: SqlitePool,
    global: String,
    global_id: Uuid,
    user_id: Uuid,
    user: String,
    other_id: Uuid,
    other: String,
}

async fn fixture() -> Fixture {
    let (app, pool) = crate::support::lb::create_test_lb().await;
    let admin = llmlb::db::users::create(
        &pool,
        "org-global@example.com",
        "hash",
        UserRole::Admin,
        false,
    )
    .await
    .unwrap();
    let user = llmlb::db::users::create(
        &pool,
        "org-member@example.com",
        "hash",
        UserRole::Viewer,
        false,
    )
    .await
    .unwrap();
    let other = llmlb::db::users::create(
        &pool,
        "org-other@example.com",
        "hash",
        UserRole::Viewer,
        false,
    )
    .await
    .unwrap();
    let secret = crate::support::lb::test_jwt_secret();
    let jwt = |id: Uuid, role| {
        llmlb::auth::jwt::create_jwt(&id.to_string(), role, &secret, false, 0).unwrap()
    };
    Fixture {
        app,
        pool,
        global: jwt(admin.id, UserRole::Admin),
        global_id: admin.id,
        user_id: user.id,
        user: jwt(user.id, UserRole::Viewer),
        other_id: other.id,
        other: jwt(other.id, UserRole::Viewer),
    }
}

async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    jwt: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(jwt) = jwt {
        builder = builder.header("authorization", format!("Bearer {jwt}"));
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

async fn organization(f: &Fixture, name: &str) -> String {
    let (status, body) = send(
        &f.app,
        "POST",
        "/api/organizations",
        Some(&f.global),
        json!({"name":name}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_str().unwrap().to_string()
}

async fn assign(
    f: &Fixture,
    account_id: Uuid,
    organization_id: Option<&str>,
    role: &str,
) -> (StatusCode, Value) {
    send(
        &f.app,
        "PUT",
        &format!("/api/users/{account_id}/membership"),
        Some(&f.global),
        json!({"organization_id":organization_id,"organization_role":role}),
    )
    .await
}

#[tokio::test]
#[serial]
async fn organizations_me_reports_current_scope_and_membership() {
    let f = fixture().await;
    let (status, me) = send(&f.app, "GET", "/api/auth/me", Some(&f.user), Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["scope"], "unassigned");
    assert!(me["organization_id"].is_null());
    let (_, me) = send(&f.app, "GET", "/api/auth/me", Some(&f.global), Value::Null).await;
    assert_eq!(me["scope"], "global");
    let id = organization(&f, "Me scope").await;
    assert_eq!(
        assign(&f, f.user_id, Some(&id), "admin").await.0,
        StatusCode::OK
    );
    let (_, me) = send(&f.app, "GET", "/api/auth/me", Some(&f.user), Value::Null).await;
    assert_eq!(me["scope"], "organization");
    assert_eq!(me["organization_id"], id);
    assert_eq!(me["organization_role"], "admin");
    assert_eq!(me["role"], "viewer");
    assert_eq!(
        assign(&f, f.user_id, Some(&id), "member").await.0,
        StatusCode::OK
    );
    let (_, me) = send(&f.app, "GET", "/api/auth/me", Some(&f.user), Value::Null).await;
    assert_eq!(me["scope"], "account");
}

#[tokio::test]
#[serial]
async fn organizations_crud_validates_names_and_refuses_nonempty_archive() {
    let f = fixture().await;
    let id = organization(&f, " Team A ").await;
    let (status, rows) = send(
        &f.app,
        "GET",
        "/api/organizations",
        Some(&f.global),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rows["organizations"][0]["name"], "Team A");
    assert_eq!(
        send(
            &f.app,
            "POST",
            "/api/organizations",
            Some(&f.global),
            json!({"name":"team a"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        send(
            &f.app,
            "POST",
            "/api/organizations",
            Some(&f.global),
            json!({"name":"  "})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        assign(&f, f.user_id, Some(&id), "admin").await.0,
        StatusCode::OK
    );
    assert_eq!(
        send(
            &f.app,
            "DELETE",
            &format!("/api/organizations/{id}"),
            Some(&f.global),
            Value::Null
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        send(
            &f.app,
            "PATCH",
            &format!("/api/organizations/{id}"),
            Some(&f.global),
            json!({"name":"Renamed"})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        assign(&f, f.user_id, None, "member").await.0,
        StatusCode::OK
    );
    assert_eq!(
        send(
            &f.app,
            "DELETE",
            &format!("/api/organizations/{id}"),
            Some(&f.global),
            Value::Null
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        assign(&f, f.user_id, Some(&id), "member").await.0,
        StatusCode::BAD_REQUEST
    );
    let (_, rows) = send(
        &f.app,
        "GET",
        "/api/organizations",
        Some(&f.global),
        Value::Null,
    )
    .await;
    assert_eq!(rows["organizations"][0]["name"], "Renamed");
    assert!(rows["organizations"][0]["archived_at"].is_string());
}

#[tokio::test]
#[serial]
async fn organizations_membership_changes_are_global_jwt_only_and_cookie_csrf_protected() {
    let f = fixture().await;
    let id = organization(&f, "Restricted").await;
    assert_eq!(
        assign(&f, f.user_id, Some(&id), "admin").await.0,
        StatusCode::OK
    );
    assert_eq!(
        send(
            &f.app,
            "POST",
            "/api/organizations",
            Some(&f.user),
            json!({"name":"Escalate"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            &f.app,
            "PUT",
            &format!("/api/users/{}/membership", f.other_id),
            Some(&f.user),
            json!({"organization_id":id,"organization_role":"admin"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let request = Request::builder()
        .method("POST")
        .uri("/api/organizations")
        .header("content-type", "application/json")
        .header(
            "cookie",
            format!("{}={}", llmlb::auth::DASHBOARD_JWT_COOKIE, f.global),
        )
        .body(Body::from(r#"{"name":"CSRF"}"#))
        .unwrap();
    assert_eq!(
        f.app.clone().oneshot(request).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let request = Request::builder()
        .uri("/api/organizations")
        .header("x-api-key", "sk_debug")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        f.app.clone().oneshot(request).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(&f.app, "GET", "/api/organizations", None, Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
#[serial]
async fn organizations_reads_follow_current_membership_and_never_global_role_claims() {
    let f = fixture().await;
    let a = organization(&f, "Organization A").await;
    let b = organization(&f, "Organization B").await;
    assert_eq!(
        assign(&f, f.user_id, Some(&a), "admin").await.0,
        StatusCode::OK
    );
    assert_eq!(
        assign(&f, f.other_id, Some(&a), "member").await.0,
        StatusCode::OK
    );
    let (status, members) = send(
        &f.app,
        "GET",
        &format!("/api/organizations/{a}/members"),
        Some(&f.user),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(members["users"].as_array().unwrap().len(), 2);
    assert_eq!(
        send(
            &f.app,
            "GET",
            &format!("/api/organizations/{b}/members"),
            Some(&f.user),
            Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (_, members) = send(
        &f.app,
        "GET",
        &format!("/api/organizations/{a}/members"),
        Some(&f.other),
        Value::Null,
    )
    .await;
    assert_eq!(members["users"].as_array().unwrap().len(), 1);
    assert_eq!(members["users"][0]["id"], f.other_id.to_string());
    assert_eq!(
        assign(&f, f.user_id, Some(&b), "member").await.0,
        StatusCode::OK
    );
    assert_eq!(
        send(
            &f.app,
            "GET",
            &format!("/api/organizations/{a}/members"),
            Some(&f.user),
            Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let forged_role = llmlb::auth::jwt::create_jwt(
        &f.user_id.to_string(),
        UserRole::Admin,
        &crate::support::lb::test_jwt_secret(),
        false,
        0,
    )
    .unwrap();
    assert_eq!(
        send(
            &f.app,
            "POST",
            "/api/organizations",
            Some(&forged_role),
            json!({"name":"Forged global"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
#[serial]
async fn organizations_membership_persists_and_invalid_targets_are_rejected() {
    let f = fixture().await;
    let id = organization(&f, "Persistent").await;
    assert_eq!(
        assign(&f, f.user_id, Some(&id), "admin").await.0,
        StatusCode::OK
    );
    let row: (Option<String>, Option<String>) =
        sqlx::query_as("SELECT organization_id, organization_role FROM users WHERE id = ?")
            .bind(f.user_id.to_string())
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(row, (Some(id.clone()), Some("admin".to_string())));
    assert_eq!(
        assign(&f, f.other_id, Some(&Uuid::new_v4().to_string()), "member")
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        assign(&f, f.other_id, Some(&id), "owner").await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        assign(&f, Uuid::new_v4(), Some(&id), "member").await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
#[serial]
async fn organizations_new_keys_separate_issuer_and_usage_and_refuse_after_transfer() {
    let f = fixture().await;
    let a = organization(&f, "Key A").await;
    let b = organization(&f, "Key B").await;
    assert_eq!(
        assign(&f, f.user_id, Some(&a), "member").await.0,
        StatusCode::OK
    );
    let (status, key) = send(&f.app, "POST", "/api/me/api-keys", Some(&f.global), json!({"name":"delegated","permissions":["openai.inference","openai.models.read"],"usage_account_id":f.user_id,"organization_id":a})).await;
    assert_eq!(status, StatusCode::CREATED, "{key}");
    let row: (String, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT created_by, usage_account_id, organization_id FROM api_keys WHERE id = ?",
    )
    .bind(key["id"].as_str().unwrap())
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_ne!(row.0, f.user_id.to_string());
    assert_eq!(row.1, Some(f.user_id.to_string()));
    assert_eq!(row.2, Some(a.clone()));
    let api_request = || {
        Request::builder()
            .uri("/v1/models")
            .header(
                "authorization",
                format!("Bearer {}", key["key"].as_str().unwrap()),
            )
            .body(Body::empty())
            .unwrap()
    };
    assert_eq!(
        f.app.clone().oneshot(api_request()).await.unwrap().status(),
        StatusCode::OK
    );
    assert_eq!(
        assign(&f, f.user_id, Some(&b), "member").await.0,
        StatusCode::OK
    );
    assert_eq!(
        f.app.clone().oneshot(api_request()).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let row: Option<String> =
        sqlx::query_scalar("SELECT organization_id FROM api_keys WHERE id = ?")
            .bind(key["id"].as_str().unwrap())
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(row, Some(a));
}

#[tokio::test]
#[serial]
async fn organizations_new_keys_reject_spoofed_or_unassigned_usage_and_hide_legacy() {
    let f = fixture().await;
    let a = organization(&f, "Visible").await;
    let b = organization(&f, "Hidden").await;
    assert_eq!(
        assign(&f, f.user_id, Some(&a), "admin").await.0,
        StatusCode::OK
    );
    for (account, org) in [(f.user_id, &b), (f.other_id, &a)] {
        assert_eq!(send(&f.app, "POST", "/api/me/api-keys", Some(&f.global), json!({"name":format!("invalid-{account}-{org}"),"permissions":["openai.inference"],"usage_account_id":account,"organization_id":org})).await.0, StatusCode::BAD_REQUEST);
    }
    assert_eq!(
        send(
            &f.app,
            "POST",
            "/api/me/api-keys",
            Some(&f.user),
            json!({"name":"spoofed","usage_account_id":f.other_id,"organization_id":a})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let legacy = llmlb::db::api_keys::create(
        &f.pool,
        "legacy",
        f.user_id,
        None,
        vec![ApiKeyPermission::OpenaiModelsRead],
    )
    .await
    .unwrap();
    let (_, keys) = send(
        &f.app,
        "GET",
        "/api/me/api-keys",
        Some(&f.user),
        Value::Null,
    )
    .await;
    assert!(!keys["api_keys"]
        .as_array()
        .unwrap()
        .iter()
        .any(|key| key["id"] == legacy.id.to_string()));
    let request = Request::builder()
        .uri("/v1/models")
        .header("authorization", format!("Bearer {}", legacy.key))
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        f.app.clone().oneshot(request).await.unwrap().status(),
        StatusCode::OK
    );
}

#[tokio::test]
#[serial]
async fn organizations_scope_rejects_deleted_and_disabled_accounts() {
    let f = fixture().await;
    let id = organization(&f, "Account state").await;
    assert_eq!(
        assign(&f, f.user_id, Some(&id), "member").await.0,
        StatusCode::OK
    );
    sqlx::query("UPDATE users SET is_active = 0 WHERE id = ?")
        .bind(f.user_id.to_string())
        .execute(&f.pool)
        .await
        .unwrap();
    assert_eq!(
        send(
            &f.app,
            "GET",
            "/api/organizations",
            Some(&f.user),
            Value::Null
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    llmlb::db::users::delete(&f.pool, f.other_id).await.unwrap();
    assert_eq!(
        send(
            &f.app,
            "GET",
            "/api/organizations",
            Some(&f.other),
            Value::Null
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
#[serial]
async fn organizations_membership_disable_preserves_last_global_admin() {
    let f = fixture().await;
    let id = organization(&f, "Disable membership").await;
    assert_eq!(
        assign(&f, f.user_id, Some(&id), "member").await.0,
        StatusCode::OK
    );
    assert_eq!(
        send(
            &f.app,
            "PUT",
            &format!("/api/users/{}/membership", f.user_id),
            Some(&f.global),
            json!({"organization_id":id,"organization_role":"member","is_active":false})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        send(&f.app, "GET", "/api/auth/me", Some(&f.user), Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(
            &f.app,
            "PUT",
            &format!("/api/users/{}/membership", f.global_id),
            Some(&f.global),
            json!({"organization_id":null,"organization_role":"member","is_active":false})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        send(&f.app, "GET", "/api/auth/me", Some(&f.global), Value::Null)
            .await
            .0,
        StatusCode::OK
    );
}

#[tokio::test]
#[serial]
async fn organizations_attributed_keys_cannot_receive_global_management_permissions() {
    let f = fixture().await;
    let id = organization(&f, "Inference-only keys").await;
    assert_eq!(
        assign(&f, f.user_id, Some(&id), "member").await.0,
        StatusCode::OK
    );
    let (status, _) = send(&f.app, "POST", "/api/me/api-keys", Some(&f.global), json!({"name":"tenant-escalation","permissions":["openai.inference","users.manage"],"usage_account_id":f.user_id,"organization_id":id})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let key = llmlb::db::api_keys::create(
        &f.pool,
        "legacy automation",
        f.global_id,
        None,
        vec![ApiKeyPermission::UsersManage],
    )
    .await
    .unwrap();
    let request = Request::builder()
        .uri("/api/users")
        .header("x-api-key", key.key)
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        f.app.clone().oneshot(request).await.unwrap().status(),
        StatusCode::OK
    );
}

#[tokio::test]
#[serial]
async fn organizations_attributed_key_runtime_cannot_use_legacy_global_management_gate() {
    let f = fixture().await;
    let id = organization(&f, "Runtime key boundary").await;
    assert_eq!(
        assign(&f, f.user_id, Some(&id), "member").await.0,
        StatusCode::OK
    );
    // Exercise the auth boundary independently of the HTTP issuance validation.
    let key = llmlb::db::api_keys::create_attributed(
        &f.pool,
        "restricted runtime",
        f.global_id,
        None,
        vec![ApiKeyPermission::UsersManage],
        f.user_id,
        id.parse().unwrap(),
    )
    .await
    .unwrap();
    let request = Request::builder()
        .uri("/api/users")
        .header("x-api-key", key.key)
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        f.app.clone().oneshot(request).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
#[serial]
async fn organizations_login_rejects_disabled_account_with_valid_password() {
    let f = fixture().await;
    let password = "OrganizationPass123";
    let hash = llmlb::auth::password::hash_password(password).unwrap();
    sqlx::query("UPDATE users SET password_hash = ?, is_active = 0 WHERE id = ?")
        .bind(hash)
        .bind(f.user_id.to_string())
        .execute(&f.pool)
        .await
        .unwrap();
    assert_eq!(
        send(
            &f.app,
            "POST",
            "/api/auth/login",
            None,
            json!({"username":"org-member@example.com","password":password})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
#[serial]
async fn organizations_deactivation_permanently_revokes_existing_jwt_after_reenable() {
    let f = fixture().await;
    let id = organization(&f, "JWT revocation").await;
    assert_eq!(
        assign(&f, f.user_id, Some(&id), "member").await.0,
        StatusCode::OK
    );
    for is_active in [false, true] {
        assert_eq!(
            send(
                &f.app,
                "PUT",
                &format!("/api/users/{}/membership", f.user_id),
                Some(&f.global),
                json!({"organization_id":id,"organization_role":"member","is_active":is_active})
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    assert_eq!(
        send(&f.app, "GET", "/api/auth/me", Some(&f.user), Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let account = llmlb::db::users::find_by_id(&f.pool, f.user_id)
        .await
        .unwrap()
        .unwrap();
    let fresh = llmlb::auth::jwt::create_jwt(
        &f.user_id.to_string(),
        UserRole::Viewer,
        &crate::support::lb::test_jwt_secret(),
        false,
        account.password_changed_at,
    )
    .unwrap();
    assert_eq!(
        send(&f.app, "GET", "/api/auth/me", Some(&fresh), Value::Null)
            .await
            .0,
        StatusCode::OK
    );
}

#[tokio::test]
#[serial]
async fn organizations_login_debug_credentials_use_current_database_actor() {
    let f = fixture().await;
    assert_eq!(
        send(
            &f.app,
            "POST",
            "/api/auth/login",
            None,
            json!({"username":"admin","password":"test"})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let hash = llmlb::auth::password::hash_password("test").unwrap();
    llmlb::db::users::create_with_id(
        &f.pool,
        Uuid::nil(),
        "admin",
        &hash,
        UserRole::Viewer,
        false,
    )
    .await
    .unwrap();
    let (status, login) = send(
        &f.app,
        "POST",
        "/api/auth/login",
        None,
        json!({"username":"admin","password":"test"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(login["user"]["role"], "viewer");
    assert_eq!(
        send(
            &f.app,
            "GET",
            "/api/auth/me",
            login["token"].as_str(),
            Value::Null
        )
        .await
        .0,
        StatusCode::OK
    );
    sqlx::query("UPDATE users SET is_active = 0 WHERE id = ?")
        .bind(Uuid::nil().to_string())
        .execute(&f.pool)
        .await
        .unwrap();
    assert_eq!(
        send(
            &f.app,
            "POST",
            "/api/auth/login",
            None,
            json!({"username":"admin","password":"test"})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
}
