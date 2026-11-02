use crate::common::auth::{ApiKeyPermission, UserRole};
use crate::db::organizations::{self, OrganizationRole};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use tower::ServiceExt;
use uuid::Uuid;

struct Fixture {
    app: Router,
    pool: SqlitePool,
    root: Uuid,
    alice: Uuid,
    bob: Uuid,
    organization_admin: Uuid,
    foreign: Uuid,
    unassigned: Uuid,
    alpha: Uuid,
    beta: Uuid,
}

fn jwt(id: Uuid, role: UserRole) -> String {
    crate::auth::jwt::create_jwt(&id.to_string(), role, "test-secret", false, 0).unwrap()
}

async fn fixture() -> Fixture {
    let state = crate::db::test_utils::TestAppStateBuilder::new()
        .await
        .build()
        .await;
    let pool = state.db_pool.clone();
    let root = crate::db::users::create(&pool, "root", "hash", UserRole::Admin, false)
        .await
        .unwrap()
        .id;
    let alpha = organizations::create(&pool, "Alpha").await.unwrap().id;
    let beta = organizations::create(&pool, "Beta").await.unwrap().id;
    let mut ids = Vec::new();
    for (name, org, role) in [
        ("alice", Some(alpha), OrganizationRole::Member),
        ("bob", Some(alpha), OrganizationRole::Member),
        ("org-admin", Some(alpha), OrganizationRole::Admin),
        ("foreign", Some(beta), OrganizationRole::Member),
        ("unassigned", None, OrganizationRole::Member),
    ] {
        let id = crate::db::users::create(&pool, name, "hash", UserRole::Viewer, false)
            .await
            .unwrap()
            .id;
        organizations::set_membership(&pool, id, org, role)
            .await
            .unwrap();
        ids.push(id);
    }
    Fixture {
        app: crate::api::create_app(state),
        pool,
        root,
        alice: ids[0],
        bob: ids[1],
        organization_admin: ids[2],
        foreign: ids[3],
        unassigned: ids[4],
        alpha,
        beta,
    }
}

async fn call(fixture: &Fixture, actor: Uuid, method: &str, body: Value) -> (StatusCode, Value) {
    let role = if actor == fixture.root {
        UserRole::Admin
    } else {
        UserRole::Viewer
    };
    let response = fixture
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri("/api/me/api-keys")
                .header("authorization", format!("Bearer {}", jwt(actor, role)))
                .header("content-type", "application/json")
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

async fn issue(fixture: &Fixture, target: Uuid, org: Uuid, name: &str) -> Value {
    let (status, body) = call(
        fixture,
        fixture.root,
        "POST",
        json!({"name":name,"usage_account_id":target,"organization_id":org,
               "permissions":["openai.inference","openai.models.read"]}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body
}

#[tokio::test]
async fn self_keys_use_current_membership_and_unassigned_actors_cannot_issue() {
    let fixture = fixture().await;
    let (status, key) = call(&fixture, fixture.alice, "POST", json!({"name":"self"})).await;
    assert_eq!(status, StatusCode::CREATED, "{key}");
    assert_eq!(key["usage_account_id"], fixture.alice.to_string());
    assert_eq!(key["organization_id"], fixture.alpha.to_string());
    for actor in [fixture.unassigned, fixture.root] {
        let (status, body) = call(
            &fixture,
            actor,
            "POST",
            if actor == fixture.root {
                json!({"name":"unassigned-root","permissions":["openai.inference"]})
            } else {
                json!({"name":"unassigned-member"})
            },
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }
}

#[tokio::test]
async fn organization_admins_and_members_cannot_issue_for_another_account() {
    let fixture = fixture().await;
    for (index, actor) in [fixture.alice, fixture.organization_admin]
        .into_iter()
        .enumerate()
    {
        let (status, body) = call(
            &fixture,
            actor,
            "POST",
            json!({"name":format!("forged-{index}"),"usage_account_id":fixture.bob,
                   "organization_id":fixture.alpha}),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM api_keys")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn global_issuer_cannot_grant_management_permissions_or_inactive_mismatched_membership() {
    let fixture = fixture().await;
    let (status, body) = call(
        &fixture,
        fixture.root,
        "POST",
        json!({"name":"managed","usage_account_id":fixture.alice,
               "organization_id":fixture.alpha,"permissions":["users.manage"]}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    organizations::set_disabled(&fixture.pool, fixture.alice, true)
        .await
        .unwrap();
    for (index, target, org) in [
        (0, fixture.foreign, fixture.alpha),
        (1, fixture.alice, fixture.alpha),
        (2, fixture.unassigned, fixture.alpha),
        (3, Uuid::new_v4(), fixture.alpha),
    ] {
        let (status, body) = call(
            &fixture,
            fixture.root,
            "POST",
            json!({"name":format!("invalid-{index}"),"usage_account_id":target,
                   "organization_id":org,"permissions":["openai.inference"]}),
        )
        .await;
        assert!(status.is_client_error(), "{status}: {body}");
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM api_keys")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(
        count, 0,
        "no partial unattributed key survives rejected issuance"
    );
}

#[tokio::test]
async fn key_listing_uses_usage_scope_instead_of_issuer_and_never_reveals_secrets() {
    let fixture = fixture().await;
    issue(&fixture, fixture.alice, fixture.alpha, "alpha-member").await;
    issue(&fixture, fixture.foreign, fixture.beta, "beta-member").await;
    let (status, body) = call(
        &fixture,
        fixture.organization_admin,
        "POST",
        json!({"name":"alpha-admin"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    crate::db::api_keys::create(
        &fixture.pool,
        "legacy",
        fixture.alice,
        None,
        vec![ApiKeyPermission::OpenaiInference],
    )
    .await
    .unwrap();
    for (actor, expected) in [
        (
            fixture.root,
            vec!["alpha-member", "beta-member", "alpha-admin", "legacy"],
        ),
        (
            fixture.organization_admin,
            vec!["alpha-member", "alpha-admin"],
        ),
        (fixture.alice, vec!["alpha-member"]),
        (fixture.foreign, vec!["beta-member"]),
    ] {
        let (status, response) = call(&fixture, actor, "GET", Value::Null).await;
        assert_eq!(status, StatusCode::OK, "{response}");
        let keys = response["api_keys"].as_array().unwrap();
        assert_eq!(keys.len(), expected.len(), "{response}");
        for name in expected {
            assert!(keys.iter().any(|key| key["name"] == name), "{response}");
        }
        for key in keys {
            assert!(key.get("key").is_none());
            assert!(key.get("key_hash").is_none());
            assert!(key.get("usage_account_id").is_some());
            assert!(key.get("organization_id").is_some());
        }
    }
    assert_eq!(
        call(&fixture, fixture.unassigned, "GET", Value::Null)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn transfer_keeps_old_key_attribution_and_self_issuance_uses_the_new_organization() {
    let fixture = fixture().await;
    let (status, old) = call(&fixture, fixture.alice, "POST", json!({"name":"old"})).await;
    assert_eq!(status, StatusCode::CREATED, "{old}");
    organizations::set_membership(
        &fixture.pool,
        fixture.alice,
        Some(fixture.beta),
        OrganizationRole::Member,
    )
    .await
    .unwrap();
    let (status, new) = call(&fixture, fixture.alice, "POST", json!({"name":"new"})).await;
    assert_eq!(status, StatusCode::CREATED, "{new}");
    assert_eq!(new["organization_id"], fixture.beta.to_string());
    let old_org: Option<String> =
        sqlx::query_scalar("SELECT organization_id FROM api_keys WHERE id = ?")
            .bind(old["id"].as_str().unwrap())
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(old_org, Some(fixture.alpha.to_string()));
    let (status, visible) = call(&fixture, fixture.alice, "GET", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{visible}");
    assert_eq!(visible["api_keys"].as_array().unwrap().len(), 1);
    assert_eq!(visible["api_keys"][0]["id"], new["id"]);
}
