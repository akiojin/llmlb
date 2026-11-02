//! Issue #947: the organization boundary is exercised through real handlers and SQLite.
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use llmlb::common::auth::UserRole;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use tower::ServiceExt;
use uuid::Uuid;

#[path = "organization_isolation_key_mutations.rs"]
mod key_mutations;

async fn fixture() -> (Router, SqlitePool, String, Uuid) {
    let pool = crate::support::lb::create_test_db_pool().await;
    let admin =
        llmlb::db::users::create(&pool, "organization-root", "hash", UserRole::Admin, false)
            .await
            .unwrap();
    let jwt = token(admin.id, UserRole::Admin);
    let app = crate::support::lb::build_test_router(pool.clone()).await;
    (app, pool, jwt, admin.id)
}

fn token(id: Uuid, role: UserRole) -> String {
    llmlb::auth::jwt::create_jwt(
        &id.to_string(),
        role,
        &crate::support::lb::test_jwt_secret(),
        false,
        0,
    )
    .unwrap()
}

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    jwt: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(jwt) = jwt {
        request = request.header("authorization", format!("Bearer {jwt}"));
    }
    let response = app
        .clone()
        .oneshot(
            request
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn organization(app: &Router, jwt: &str, name: &str) -> Uuid {
    let (status, body) = call(
        app,
        "POST",
        "/api/organizations",
        Some(jwt),
        json!({"name":name}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_str().unwrap().parse().unwrap()
}

#[tokio::test]
async fn current_identity_contract_rejects_deleted_subject_and_stale_admin_claim() {
    let (app, pool, admin, _) = fixture().await;
    let viewer = llmlb::db::users::create(&pool, "stale-admin", "hash", UserRole::Viewer, false)
        .await
        .unwrap();
    let stale = token(viewer.id, UserRole::Admin);
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/users",
            Some(&stale),
            json!({"username":"escalated@example.com","role":"admin"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    llmlb::db::users::delete(&pool, viewer.id).await.unwrap();
    assert_eq!(
        call(&app, "GET", "/api/auth/me", Some(&stale), Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&app, "GET", "/api/auth/me", Some(&admin), Value::Null)
            .await
            .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn attributed_key_contract_preserves_target_independently_of_issuer() {
    use llmlb::db::organizations::{self, OrganizationRole};
    let (app, pool, admin, issuer) = fixture().await;
    let org = organizations::create(&pool, "Direct Alpha").await.unwrap();
    let target = llmlb::db::users::create(&pool, "direct-target", "hash", UserRole::Viewer, false)
        .await
        .unwrap();
    organizations::set_membership(&pool, target.id, Some(org.id), OrganizationRole::Member)
        .await
        .unwrap();
    let (status,key)=call(&app,"POST","/api/me/api-keys",Some(&admin),json!({"name":"direct-target-key","usage_account_id":target.id,"organization_id":org.id,"permissions":["openai.inference"]})).await;
    assert_eq!(status, StatusCode::CREATED, "{key}");
    assert_eq!(key["usage_account_id"], target.id.to_string());
    assert_eq!(key["organization_id"], org.id.to_string());
    let row: (Option<String>, Option<String>, String) = sqlx::query_as(
        "SELECT usage_account_id,organization_id,created_by FROM api_keys WHERE id=?",
    )
    .bind(key["id"].as_str().unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        row,
        (
            Some(target.id.to_string()),
            Some(org.id.to_string()),
            issuer.to_string()
        )
    );
}

async fn round_trip_key_must_stay_revoked(intermediate_organization: bool) {
    use llmlb::common::auth::ApiKeyPermission;
    let (app, pool, admin, _) = fixture().await;
    let alpha = organization(&app, &admin, "Round Trip Alpha").await;
    let beta = organization(&app, &admin, "Round Trip Beta").await;
    let (account, _) = member(&app, &pool, &admin, "round-trip-account", alpha, "member").await;
    let legacy = llmlb::db::api_keys::create(
        &pool,
        "legacy-null-target",
        account,
        None,
        vec![
            ApiKeyPermission::OpenaiInference,
            ApiKeyPermission::OpenaiModelsRead,
        ],
    )
    .await
    .unwrap();
    let payload = json!({"name":"original-alpha","usage_account_id":account,"organization_id":alpha,"permissions":["openai.inference","openai.models.read"]});
    let (status, key) = call(&app, "POST", "/api/me/api-keys", Some(&admin), payload).await;
    assert_eq!(status, StatusCode::CREATED, "{key}");
    let original = key["key"].as_str().unwrap();
    assert_eq!(
        call(&app, "GET", "/v1/models", Some(original), Value::Null)
            .await
            .0,
        StatusCode::OK
    );
    // An organization role adjustment keeps the same valid inference identity.
    assert_eq!(
        call(
            &app,
            "PUT",
            &format!("/api/users/{account}/membership"),
            Some(&admin),
            json!({"organization_id":alpha,"organization_role":"admin"})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&app, "GET", "/v1/models", Some(original), Value::Null)
            .await
            .0,
        StatusCode::OK
    );
    let intermediate = if intermediate_organization {
        Some(beta)
    } else {
        None
    };
    assert_eq!(
        call(
            &app,
            "PUT",
            &format!("/api/users/{account}/membership"),
            Some(&admin),
            json!({"organization_id":intermediate,"organization_role":"member"})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&app, "GET", "/v1/models", Some(original), Value::Null)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &app,
            "PUT",
            &format!("/api/users/{account}/membership"),
            Some(&admin),
            json!({"organization_id":alpha,"organization_role":"member"})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&app, "GET", "/v1/models", Some(original), Value::Null)
            .await
            .0,
        StatusCode::FORBIDDEN,
        "An old organization key must not become valid again after return"
    );
    assert_eq!(
        call(&app, "GET", "/v1/models", Some(&legacy.key), Value::Null)
            .await
            .0,
        StatusCode::OK,
        "Legacy NULL attribution keys remain unchanged"
    );
    let restarted = crate::support::lb::build_test_router(pool.clone()).await;
    assert_eq!(
        call(&restarted, "GET", "/v1/models", Some(original), Value::Null)
            .await
            .0,
        StatusCode::FORBIDDEN,
        "Revocation must persist across restart"
    );
    let (status,reissued)=call(&restarted,"POST","/api/me/api-keys",Some(&admin),json!({"name":"reissued-alpha","usage_account_id":account,"organization_id":alpha,"permissions":["openai.inference","openai.models.read"]})).await;
    assert_eq!(status, StatusCode::CREATED, "{reissued}");
    assert_eq!(
        call(
            &restarted,
            "GET",
            "/v1/models",
            Some(reissued["key"].as_str().unwrap()),
            Value::Null
        )
        .await
        .0,
        StatusCode::OK
    );
    let legacy_attribution: (Option<String>, Option<String>) =
        sqlx::query_as("SELECT usage_account_id,organization_id FROM api_keys WHERE id=?")
            .bind(legacy.id.to_string())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(legacy_attribution, (None, None));
}

#[tokio::test]
async fn attributed_key_contract_round_trip_organization_transfer_requires_reissue() {
    round_trip_key_must_stay_revoked(true).await;
}

#[tokio::test]
async fn attributed_key_contract_round_trip_unassignment_requires_reissue() {
    round_trip_key_must_stay_revoked(false).await;
}

async fn member(
    app: &Router,
    pool: &SqlitePool,
    admin: &str,
    name: &str,
    org: Uuid,
    role: &str,
) -> (Uuid, String) {
    let user = llmlb::db::users::create(pool, name, "hash", UserRole::Viewer, false)
        .await
        .unwrap();
    let (status, body) = call(
        app,
        "PUT",
        &format!("/api/users/{}/membership", user.id),
        Some(admin),
        json!({"organization_id":org,"organization_role":role}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    (user.id, token(user.id, UserRole::Viewer))
}

#[tokio::test]
async fn organization_contract_crud_membership_and_archive_are_persistent() {
    let (app, pool, admin, _) = fixture().await;
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/organizations",
            Some(&admin),
            json!({"name":"  "})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let alpha = organization(&app, &admin, " Alpha ").await;
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/organizations",
            Some(&admin),
            json!({"name":"alpha"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (id, jwt) = member(&app, &pool, &admin, "alpha-member", alpha, "admin").await;
    let (status, me) = call(&app, "GET", "/api/auth/me", Some(&jwt), Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["organization_id"], alpha.to_string());
    assert_eq!(me["organization_role"], "admin");
    assert_eq!(me["role"], "viewer");
    assert_eq!(
        call(
            &app,
            "DELETE",
            &format!("/api/organizations/{alpha}"),
            Some(&admin),
            Value::Null
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &app,
            "PUT",
            &format!("/api/organizations/{alpha}"),
            Some(&admin),
            json!({"name":"Renamed"})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "PUT",
            &format!("/api/users/{id}/membership"),
            Some(&admin),
            json!({"organization_id":null,"organization_role":"member"})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "DELETE",
            &format!("/api/organizations/{alpha}"),
            Some(&admin),
            Value::Null
        )
        .await
        .0,
        StatusCode::OK
    );
    let restarted = crate::support::lb::build_test_router(pool).await;
    let (_, list) = call(
        &restarted,
        "GET",
        "/api/organizations",
        Some(&admin),
        Value::Null,
    )
    .await;
    assert_eq!(list["organizations"][0]["name"], "Renamed");
    assert!(list["organizations"][0]["archived_at"].is_string());
}

#[tokio::test]
async fn organization_contract_org_roles_cannot_change_membership_or_global_settings() {
    let (app, pool, admin, _) = fixture().await;
    let alpha = organization(&app, &admin, "Alpha").await;
    for role in ["admin", "member"] {
        let (id, jwt) = member(&app, &pool, &admin, &format!("alpha-{role}"), alpha, role).await;
        assert_eq!(
            call(
                &app,
                "POST",
                "/api/organizations",
                Some(&jwt),
                json!({"name":"Escalation"})
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            call(
                &app,
                "PUT",
                &format!("/api/users/{id}/membership"),
                Some(&jwt),
                json!({"organization_id":null})
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        call(&app, "GET", "/api/organizations", None, Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &app,
            "GET",
            "/api/organizations",
            Some("sk_debug"),
            Value::Null
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn organization_contract_usage_and_account_lists_reject_cross_scope_filters() {
    let (app, pool, admin, _) = fixture().await;
    let alpha = organization(&app, &admin, "Alpha").await;
    let beta = organization(&app, &admin, "Beta").await;
    let (_, a) = member(&app, &pool, &admin, "alpha-admin", alpha, "admin").await;
    let (b_id, _) = member(&app, &pool, &admin, "beta-member", beta, "member").await;
    let (_, m) = member(&app, &pool, &admin, "alpha-member", alpha, "member").await;
    for jwt in [&a, &m] {
        assert_eq!(
            call(
                &app,
                "GET",
                &format!("/api/dashboard/usage?organization_id={beta}"),
                Some(jwt),
                Value::Null
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            call(
                &app,
                "GET",
                &format!("/api/dashboard/usage?account_id={b_id}"),
                Some(jwt),
                Value::Null
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        let (status, list) = call(&app, "GET", "/api/users", Some(jwt), Value::Null).await;
        assert_eq!(status, StatusCode::OK);
        assert!(list["users"]
            .as_array()
            .unwrap()
            .iter()
            .all(|u| u["organization_id"] == alpha.to_string()));
    }
    let unassigned = llmlb::db::users::create(&pool, "unassigned", "hash", UserRole::Viewer, false)
        .await
        .unwrap();
    assert_eq!(
        call(
            &app,
            "GET",
            "/api/dashboard/usage",
            Some(&token(unassigned.id, UserRole::Viewer)),
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn organization_contract_attributed_key_is_not_issuer_and_old_key_fails_after_move() {
    let (app, pool, admin, issuer) = fixture().await;
    let alpha = organization(&app, &admin, "Alpha").await;
    let beta = organization(&app, &admin, "Beta").await;
    let (account, _) = member(&app, &pool, &admin, "key-user", alpha, "member").await;
    let (status,key)=call(&app,"POST","/api/me/api-keys",Some(&admin),json!({"name":"account-key","usage_account_id":account,"organization_id":alpha,"permissions":["openai.inference","openai.models.read"]})).await;
    assert_eq!(status, StatusCode::CREATED, "{key}");
    assert_eq!(key["usage_account_id"], account.to_string());
    assert_eq!(key["organization_id"], alpha.to_string());
    assert_ne!(key["usage_account_id"], issuer.to_string());
    let plaintext = key["key"].as_str().unwrap();
    assert_eq!(
        call(
            &app,
            "GET",
            "/api/organizations",
            Some(plaintext),
            Value::Null
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(call(&app,"POST","/api/me/api-keys",Some(&admin),json!({"name":"elevated","usage_account_id":account,"organization_id":alpha,"permissions":["users.manage"]})).await.0,StatusCode::BAD_REQUEST);
    assert_eq!(
        call(
            &app,
            "PUT",
            &format!("/api/users/{account}/membership"),
            Some(&admin),
            json!({"organization_id":beta,"organization_role":"member"})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&app, "GET", "/v1/models", Some(plaintext), Value::Null)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
}

#[cfg(debug_assertions)]
#[tokio::test]
async fn organization_contract_bootstrapped_admin_requires_current_database_identity() {
    let pool = crate::support::lb::create_test_db_pool().await;
    llmlb::auth::bootstrap::ensure_admin_exists(&pool)
        .await
        .unwrap();
    let bootstrapped = llmlb::db::users::find_by_username(&pool, "admin")
        .await
        .unwrap()
        .unwrap();
    // Existing development databases use this stable ID. It grants no authority by itself.
    assert!(bootstrapped.id.is_nil());
    assert_eq!(bootstrapped.password_changed_at, 0);
    let app = crate::support::lb::build_test_router(pool.clone()).await;
    let credentials = json!({"username":"admin","password":"test"});
    let (status, login) = call(&app, "POST", "/api/auth/login", None, credentials.clone()).await;
    assert_eq!(status, StatusCode::OK, "{login}");
    let jwt = login["token"].as_str().unwrap();
    let claims = llmlb::auth::jwt::verify_jwt(jwt, &crate::support::lb::test_jwt_secret()).unwrap();
    assert_eq!(claims.sub, bootstrapped.id.to_string());
    assert_eq!(claims.password_changed_at, bootstrapped.password_changed_at);
    let (status, identity) = call(&app, "GET", "/api/auth/me", Some(jwt), Value::Null).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a freshly bootstrapped signed-in DB admin must authenticate: {identity}"
    );
    assert_eq!(identity["user_id"], bootstrapped.id.to_string());
    assert_eq!(identity["role"], "admin");

    llmlb::db::users::create(
        &pool,
        "remaining-bootstrap-admin",
        "hash",
        UserRole::Admin,
        false,
    )
    .await
    .unwrap();
    llmlb::db::organizations::set_disabled(&pool, bootstrapped.id, true)
        .await
        .unwrap();
    assert_eq!(
        call(&app, "GET", "/api/auth/me", Some(jwt), Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    llmlb::db::organizations::set_disabled(&pool, bootstrapped.id, false)
        .await
        .unwrap();
    assert_eq!(
        call(&app, "GET", "/api/auth/me", Some(jwt), Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED,
        "reenabling an account does not revive its old JWT"
    );
    let (status, reissued) = call(&app, "POST", "/api/auth/login", None, credentials.clone()).await;
    assert_eq!(status, StatusCode::OK);
    let current_jwt = reissued["token"].as_str().unwrap();
    assert_eq!(
        call(&app, "GET", "/api/auth/me", Some(current_jwt), Value::Null)
            .await
            .0,
        StatusCode::OK
    );

    llmlb::db::users::update(&pool, bootstrapped.id, None, None, Some(UserRole::Viewer))
        .await
        .unwrap();
    assert_eq!(
        call(&app, "GET", "/api/auth/me", Some(current_jwt), Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, viewer) = call(&app, "POST", "/api/auth/login", None, credentials).await;
    assert_eq!(status, StatusCode::OK);
    let viewer_jwt = viewer["token"].as_str().unwrap();
    let (status, identity) = call(&app, "GET", "/api/auth/me", Some(viewer_jwt), Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        identity["role"], "viewer",
        "nil identity never bypasses the database role"
    );
    assert_eq!(
        call(
            &app,
            "GET",
            "/api/organizations",
            Some(viewer_jwt),
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    llmlb::db::users::delete(&pool, bootstrapped.id)
        .await
        .unwrap();
    assert_eq!(
        call(&app, "GET", "/api/auth/me", Some(viewer_jwt), Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}
