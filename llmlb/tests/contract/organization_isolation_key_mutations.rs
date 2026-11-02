//! API-key mutation authority is the intersection of current scope and issuer.
use super::{call, fixture, member, organization};
use axum::{http::StatusCode, Router};
use llmlb::common::auth::{ApiKeyPermission, UserRole};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use uuid::Uuid;

async fn fresh_token(pool: &SqlitePool, id: Uuid) -> String {
    let user = llmlb::db::users::find_by_id(pool, id)
        .await
        .unwrap()
        .unwrap();
    llmlb::auth::jwt::create_jwt(
        &id.to_string(),
        user.role,
        &crate::support::lb::test_jwt_secret(),
        user.must_change_password,
        user.password_changed_at,
    )
    .unwrap()
}

async fn issued_key(app: &Router, jwt: &str, target: Uuid, org: Uuid, name: &str) -> Uuid {
    let (status, body) = call(app, "POST", "/api/me/api-keys", Some(jwt), json!({
        "name":name,"usage_account_id":target,"organization_id":org,"permissions":["openai.inference"]
    })).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_str().unwrap().parse().unwrap()
}

async fn move_and_demote_issuer_fixture() -> (Router, SqlitePool, String, Uuid) {
    let (app, pool, root, _) = fixture().await;
    // Keep the original global administrator; demotion is a legitimate transition.
    let issuer = llmlb::db::users::create(&pool, "mutation-issuer", "hash", UserRole::Admin, false)
        .await
        .unwrap();
    let alpha = organization(&app, &root, "Issuer Alpha").await;
    let beta = organization(&app, &root, "Target Beta").await;
    let delta = organization(&app, &root, "New Issuer Delta").await;
    let (target, _) = member(&app, &pool, &root, "foreign-target", beta, "member").await;
    assert_eq!(
        call(
            &app,
            "PUT",
            &format!("/api/users/{}/membership", issuer.id),
            Some(&root),
            json!({"organization_id":alpha,"organization_role":"member"})
        )
        .await
        .0,
        StatusCode::OK
    );
    let key = issued_key(
        &app,
        &fresh_token(&pool, issuer.id).await,
        target,
        beta,
        "foreign-original",
    )
    .await;
    assert_eq!(
        call(
            &app,
            "PUT",
            &format!("/api/users/{}", issuer.id),
            Some(&root),
            json!({"role":"viewer"})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "PUT",
            &format!("/api/users/{}/membership", issuer.id),
            Some(&root),
            json!({"organization_id":delta,"organization_role":"member"})
        )
        .await
        .0,
        StatusCode::OK
    );
    let jwt = fresh_token(&pool, issuer.id).await;
    let (status, keys) = call(&app, "GET", "/api/me/api-keys", Some(&jwt), Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{keys}");
    assert!(!keys["api_keys"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["id"] == key.to_string()));
    (app, pool, jwt, key)
}

async fn assert_foreign_mutation_rejected(method: &str) {
    let (app, pool, jwt, key) = move_and_demote_issuer_fixture().await;
    let (status, body) = call(
        &app,
        method,
        &format!("/api/me/api-keys/{key}"),
        Some(&jwt),
        json!({"name":"must-not-change","expires_at":"2030-01-01T00:00:00Z"}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a current-scope-hidden key must also be hidden from {method}: {body}"
    );
    let persisted: (String, Option<String>) =
        sqlx::query_as("SELECT name,expires_at FROM api_keys WHERE id=?")
            .bind(key.to_string())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(persisted, ("foreign-original".to_string(), None));
}

#[tokio::test]
async fn key_mutation_foreign_put_after_issuer_demotion_and_transfer_is_not_found() {
    assert_foreign_mutation_rejected("PUT").await;
}

#[tokio::test]
async fn key_mutation_foreign_delete_after_issuer_demotion_and_transfer_is_not_found() {
    assert_foreign_mutation_rejected("DELETE").await;
}

async fn assert_legacy_non_global_mutation_rejected(method: &str) {
    let (app, pool, root, _) = fixture().await;
    let alpha = organization(&app, &root, "Legacy Alpha").await;
    let (issuer, jwt) = member(&app, &pool, &root, "legacy-member", alpha, "member").await;
    let key = llmlb::db::api_keys::create(
        &pool,
        "legacy-private",
        issuer,
        None,
        vec![ApiKeyPermission::OpenaiInference],
    )
    .await
    .unwrap();
    let (status, body) = call(
        &app,
        method,
        &format!("/api/me/api-keys/{}", key.id),
        Some(&jwt),
        json!({"name":"legacy-must-not-change","expires_at":"2030-01-01T00:00:00Z"}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "legacy NULL attribution is global-only: {body}"
    );
    let persisted: String = sqlx::query_scalar("SELECT name FROM api_keys WHERE id=?")
        .bind(key.id.to_string())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(persisted, "legacy-private");
}

#[tokio::test]
async fn key_mutation_legacy_put_is_hidden_from_non_global_issuer() {
    assert_legacy_non_global_mutation_rejected("PUT").await;
}

#[tokio::test]
async fn key_mutation_legacy_delete_is_hidden_from_non_global_issuer() {
    assert_legacy_non_global_mutation_rejected("DELETE").await;
}

async fn assert_same_scope_issuer_can_mutate(organization_admin: bool) {
    let (app, pool, root, _) = fixture().await;
    let alpha = organization(&app, &root, "Valid Scope Alpha").await;
    let issuer = llmlb::db::users::create(
        &pool,
        "valid-mutation-issuer",
        "hash",
        UserRole::Admin,
        false,
    )
    .await
    .unwrap();
    let (target, _) = member(&app, &pool, &root, "same-scope-target", alpha, "member").await;
    assert_eq!(call(&app,"PUT",&format!("/api/users/{}/membership",issuer.id),Some(&root),
        json!({"organization_id":alpha,"organization_role":if organization_admin{"admin"}else{"member"}})).await.0,StatusCode::OK);
    let usage_account = if organization_admin {
        target
    } else {
        issuer.id
    };
    let key = issued_key(
        &app,
        &fresh_token(&pool, issuer.id).await,
        usage_account,
        alpha,
        "valid-key",
    )
    .await;
    assert_eq!(
        call(
            &app,
            "PUT",
            &format!("/api/users/{}", issuer.id),
            Some(&root),
            json!({"role":"viewer"})
        )
        .await
        .0,
        StatusCode::OK
    );
    let jwt = fresh_token(&pool, issuer.id).await;
    let path = format!("/api/me/api-keys/{key}");
    let (status, updated) = call(
        &app,
        "PUT",
        &path,
        Some(&jwt),
        json!({"name":"renamed-valid","expires_at":"2030-01-01T00:00:00Z"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["name"], "renamed-valid");
    assert_eq!(updated["created_by"], issuer.id.to_string());
    assert_eq!(updated["usage_account_id"], usage_account.to_string());
    assert_eq!(updated["organization_id"], alpha.to_string());
    assert_eq!(updated["expires_at"], "2030-01-01T00:00:00+00:00");
    assert!(updated.get("key_hash").is_none());
    assert!(updated.get("key").is_none());
    assert_eq!(
        call(&app, "DELETE", &path, Some(&jwt), Value::Null).await.0,
        StatusCode::NO_CONTENT
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM api_keys WHERE id=?")
        .bind(key.to_string())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn key_mutation_same_self_scope_issuer_can_update_expiry_and_delete() {
    assert_same_scope_issuer_can_mutate(false).await;
}

#[tokio::test]
async fn key_mutation_same_organization_scope_issuer_can_update_and_delete() {
    assert_same_scope_issuer_can_mutate(true).await;
}

#[tokio::test]
async fn key_mutation_same_organization_other_issuer_remains_read_only() {
    let (app, pool, root, _) = fixture().await;
    let alpha = organization(&app, &root, "Read Only Alpha").await;
    let (account, jwt) = member(&app, &pool, &root, "organization-admin", alpha, "admin").await;
    let key = issued_key(&app, &root, account, alpha, "other-issuer").await;
    let (status, list) = call(&app, "GET", "/api/me/api-keys", Some(&jwt), Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert!(list["api_keys"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["id"] == key.to_string()));
    for method in ["PUT", "DELETE"] {
        assert_eq!(
            call(
                &app,
                method,
                &format!("/api/me/api-keys/{key}"),
                Some(&jwt),
                json!({"name":"forbidden"})
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
    }
}

#[tokio::test]
async fn key_mutation_unassigned_issuer_requires_membership() {
    let (app, pool, _, _) = fixture().await;
    let user = llmlb::db::users::create(
        &pool,
        "unassigned-key-issuer",
        "hash",
        UserRole::Viewer,
        false,
    )
    .await
    .unwrap();
    let key = llmlb::db::api_keys::create(
        &pool,
        "unassigned-legacy",
        user.id,
        None,
        vec![ApiKeyPermission::OpenaiInference],
    )
    .await
    .unwrap();
    let jwt = fresh_token(&pool, user.id).await;
    for method in ["PUT", "DELETE"] {
        assert_eq!(
            call(
                &app,
                method,
                &format!("/api/me/api-keys/{}", key.id),
                Some(&jwt),
                json!({"name":"forbidden"})
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
}

#[tokio::test]
async fn key_mutation_global_legacy_duplicate_expiry_preserves_duplicate_name_compatibility() {
    let (app, pool, root, issuer) = fixture().await;
    let original = llmlb::db::api_keys::create(
        &pool,
        "legacy-name",
        issuer,
        None,
        vec![ApiKeyPermission::OpenaiInference],
    )
    .await
    .unwrap();
    let duplicate = llmlb::db::api_keys::create(
        &pool,
        "unique-before-migration",
        issuer,
        None,
        vec![ApiKeyPermission::OpenaiInference],
    )
    .await
    .unwrap();
    sqlx::query("DROP TRIGGER trg_api_keys_reject_duplicate_name_update")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE api_keys SET name='legacy-name' WHERE id=?")
        .bind(duplicate.id.to_string())
        .execute(&pool)
        .await
        .unwrap();
    // An old duplicate-protection trigger must not prevent an expiry-only update.
    sqlx::query("CREATE TRIGGER trg_api_keys_reject_duplicate_name_update BEFORE UPDATE OF name,created_by ON api_keys FOR EACH ROW WHEN EXISTS (SELECT 1 FROM api_keys WHERE created_by=NEW.created_by AND name=NEW.name AND id<>OLD.id) BEGIN SELECT RAISE(ABORT,'API key name already exists for this user'); END;").execute(&pool).await.unwrap();
    let path = format!("/api/me/api-keys/{}", duplicate.id);
    let (status, updated) = call(
        &app,
        "PUT",
        &path,
        Some(&root),
        json!({"name":"legacy-name","expires_at":"2030-01-01T00:00:00Z"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["usage_account_id"], Value::Null);
    assert_eq!(updated["organization_id"], Value::Null);
    assert!(updated.get("key_hash").is_none());
    let (status, _) = call(
        &app,
        "PUT",
        &format!("/api/me/api-keys/{}", original.id),
        Some(&root),
        json!({"name":"unique-name"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        call(
            &app,
            "PUT",
            &path,
            Some(&root),
            json!({"name":"unique-name"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(&app, "DELETE", &path, Some(&root), Value::Null)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
}
