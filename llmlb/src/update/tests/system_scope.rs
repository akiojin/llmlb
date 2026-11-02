use super::*;
use crate::common::auth::UserRole;
use crate::db::organizations::{self, OrganizationRole};
use crate::db::test_utils::TestAppStateBuilder;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    response::Response,
};
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

struct SystemFixture {
    state: crate::AppState,
    _lease: crate::inference_gate::InFlightTestGuard,
    global: String,
    tenant: String,
    tenant_id: Uuid,
}

async fn fixture() -> SystemFixture {
    let state = TestAppStateBuilder::new().await.build().await;
    let global = crate::db::users::create(
        &state.db_pool,
        "system-global",
        "hash",
        UserRole::Admin,
        false,
    )
    .await
    .unwrap();
    let tenant = crate::db::users::create(
        &state.db_pool,
        "system-tenant",
        "hash",
        UserRole::Viewer,
        false,
    )
    .await
    .unwrap();
    let organization = organizations::create(&state.db_pool, "System scope fixture")
        .await
        .unwrap();
    organizations::set_membership(
        &state.db_pool,
        tenant.id,
        Some(organization.id.parse().unwrap()),
        OrganizationRole::Admin,
        None,
    )
    .await
    .unwrap();
    let jwt = |id: Uuid, role| {
        crate::auth::jwt::create_jwt(&id.to_string(), role, &state.auth.jwt_secret, false, 0)
            .unwrap()
    };
    let global_token = jwt(global.id, UserRole::Admin);
    let tenant_token = jwt(tenant.id, UserRole::Viewer);
    let lease = state.lifecycle.inference_gate.begin_for_test();
    *state.lifecycle.update_manager.inner.state.write().await = UpdateState::Draining {
        latest: "99.0.0".into(),
        in_flight: 1,
        requested_at: Utc::now(),
        timeout_at: Utc::now() + chrono::Duration::minutes(5),
    };
    SystemFixture {
        state,
        _lease: lease,
        global: global_token,
        tenant: tenant_token,
        tenant_id: tenant.id,
    }
}

async fn get(fixture: &SystemFixture, header: Option<(&str, String)>) -> Response {
    let mut request = Request::builder().uri("/api/system");
    if let Some((name, value)) = header {
        request = request.header(name, value);
    }
    crate::api::create_app(fixture.state.clone())
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn body(response: Response) -> Value {
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn public_system_scope_hides_top_level_and_draining_counts_from_anonymous_tenant_and_key() {
    let fixture = fixture().await;
    for header in [
        None,
        Some(("authorization", format!("Bearer {}", fixture.tenant))),
        Some(("authorization", "Bearer sk_debug".into())),
    ] {
        let response = get(&fixture, header).await;
        let cache = response.headers().get("cache-control").cloned();
        let json = body(response).await;
        assert!(json["version"].is_string());
        assert_eq!(json["update"]["state"], "draining");
        assert!(json["in_flight"].is_null(), "global count leaked: {json}");
        assert!(
            json["update"]["in_flight"].is_null(),
            "nested global count leaked: {json}"
        );
        assert_eq!(
            cache.as_ref().and_then(|header| header.to_str().ok()),
            Some("private, no-store")
        );
    }
    assert_eq!(fixture.state.lifecycle.inference_gate.in_flight(), 1);
    assert!(matches!(
        fixture.state.lifecycle.update_manager.state().await,
        UpdateState::Draining { in_flight: 1, .. }
    ));
}

#[tokio::test]
async fn public_system_scope_exposes_real_counts_only_to_current_global_jwt_with_no_store() {
    let fixture = fixture().await;
    assert_eq!(fixture.state.lifecycle.inference_gate.in_flight(), 1);
    for header in [
        ("authorization", format!("Bearer {}", fixture.global)),
        (
            "cookie",
            format!("{}={}", crate::auth::DASHBOARD_JWT_COOKIE, fixture.global),
        ),
    ] {
        let response = get(&fixture, Some(header)).await;
        let cache = response.headers().get("cache-control").cloned();
        let json = body(response).await;
        assert_eq!(json["in_flight"], 1);
        assert_eq!(json["update"]["in_flight"], 1);
        assert_eq!(
            cache.as_ref().and_then(|header| header.to_str().ok()),
            Some("private, no-store")
        );
    }
}

#[tokio::test]
async fn public_system_scope_does_not_trust_claimed_admin_or_missing_database_actor() {
    let fixture = fixture().await;
    for id in [fixture.tenant_id, Uuid::nil(), Uuid::new_v4()] {
        let token = crate::auth::jwt::create_jwt(
            &id.to_string(),
            UserRole::Admin,
            &fixture.state.auth.jwt_secret,
            false,
            0,
        )
        .unwrap();
        let json =
            body(get(&fixture, Some(("authorization", format!("Bearer {token}")))).await).await;
        assert!(json["in_flight"].is_null());
        assert!(json["update"]["in_flight"].is_null());
        assert_eq!(json["update"]["state"], "draining");
    }
}
