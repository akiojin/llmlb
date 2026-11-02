//! Issue #947: SQL-scoped rollups through the real JWT handler and SQLite.

use axum::{
    body::{to_bytes, Body},
    http::{HeaderMap, Request, StatusCode},
    Router,
};
use chrono::{Datelike, TimeZone, Utc};
use llmlb::{
    common::{
        auth::UserRole,
        usage::{UsageAttribution, UsageTokens},
    },
    db::{
        organizations::{self, OrganizationRole},
        usage, users,
    },
};
use serde_json::Value;
use sqlx::SqlitePool;
use tower::ServiceExt;
use uuid::Uuid;

struct Fixture {
    app: Router,
    pool: SqlitePool,
    global: String,
    alpha: Uuid,
    beta: Uuid,
    alpha_admin: Uuid,
    alpha_member: Uuid,
    alpha_peer: Uuid,
    beta_member: Uuid,
    alpha_admin_jwt: String,
    alpha_member_jwt: String,
    beta_admin_jwt: String,
    unassigned_jwt: String,
}

fn token(account_id: Uuid, role: UserRole) -> String {
    llmlb::auth::jwt::create_jwt(
        &account_id.to_string(),
        role,
        &crate::support::lb::test_jwt_secret(),
        false,
        0,
    )
    .unwrap()
}

async fn account(pool: &SqlitePool, name: &str, org: Option<Uuid>, role: OrganizationRole) -> Uuid {
    let user = users::create(pool, name, "fixture-hash", UserRole::Viewer, false)
        .await
        .unwrap();
    organizations::set_membership(pool, user.id, org, role)
        .await
        .unwrap();
    user.id
}

async fn receipt(
    pool: &SqlitePool,
    organization_id: Option<Uuid>,
    account_id: Option<Uuid>,
    at: &str,
    tokens: Option<UsageTokens>,
) {
    let attribution = UsageAttribution {
        request_id: Uuid::new_v4(),
        organization_id,
        account_id,
        received_at: at.parse().unwrap(),
        api_key_id: None,
        auth_source: if organization_id.is_some() {
            "jwt"
        } else {
            "api_key"
        }
        .into(),
    };
    usage::accept(pool, &attribution).await.unwrap();
    if let Some(tokens) = tokens {
        usage::finish(pool, attribution.request_id, &tokens)
            .await
            .unwrap();
    }
}

async fn fixture() -> Fixture {
    let pool = crate::support::lb::create_test_db_pool().await;
    sqlx::query("UPDATE usage_observation SET started_at = '2026-01-01T00:00:00Z' WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();
    let global_id = users::create(&pool, "global", "hash", UserRole::Admin, false)
        .await
        .unwrap()
        .id;
    let alpha = organizations::create(&pool, "Alpha").await.unwrap().id;
    let beta = organizations::create(&pool, "Beta").await.unwrap().id;
    let alpha_admin = account(&pool, "alpha-admin", Some(alpha), OrganizationRole::Admin).await;
    let alpha_member = account(&pool, "alpha-member", Some(alpha), OrganizationRole::Member).await;
    let alpha_peer = account(&pool, "alpha-peer", Some(alpha), OrganizationRole::Member).await;
    let beta_admin = account(&pool, "beta-admin", Some(beta), OrganizationRole::Admin).await;
    let beta_member = account(&pool, "beta-member", Some(beta), OrganizationRole::Member).await;
    let unassigned = account(&pool, "unassigned", None, OrganizationRole::Member).await;
    for _ in 0..2 {
        receipt(
            &pool,
            Some(alpha),
            Some(alpha_admin),
            "2026-01-31T23:59:59.999999999Z",
            Some(UsageTokens::provider(Some(100), Some(200), Some(300))),
        )
        .await;
        receipt(
            &pool,
            Some(alpha),
            Some(alpha_member),
            "2026-01-01T00:00:00Z",
            Some(UsageTokens::provider(Some(1), Some(2), Some(3))),
        )
        .await;
        receipt(
            &pool,
            Some(beta),
            Some(beta_admin),
            "2026-01-15T12:00:00Z",
            Some(UsageTokens::provider(Some(7), Some(8), Some(15))),
        )
        .await;
    }
    receipt(
        &pool,
        Some(alpha),
        Some(alpha_member),
        "2026-01-15T12:00:00Z",
        Some(UsageTokens::estimated(Some(4), Some(5), Some(9))),
    )
    .await;
    receipt(
        &pool,
        Some(alpha),
        Some(alpha_peer),
        "2026-01-15T12:00:00Z",
        Some(UsageTokens::missing()),
    )
    .await;
    receipt(
        &pool,
        Some(beta),
        Some(beta_member),
        "2026-01-15T12:00:00Z",
        Some(UsageTokens::provider(Some(11), None, None)),
    )
    .await;
    receipt(
        &pool,
        None,
        None,
        "2026-01-15T12:00:00Z",
        Some(UsageTokens::provider(Some(50), Some(60), Some(110))),
    )
    .await;
    receipt(
        &pool,
        Some(alpha),
        Some(alpha_member),
        "2026-01-15T12:00:00Z",
        None,
    )
    .await;
    let app = crate::support::lb::build_test_router(pool.clone()).await;
    Fixture {
        app,
        pool,
        global: token(global_id, UserRole::Admin),
        alpha,
        beta,
        alpha_admin,
        alpha_member,
        alpha_peer,
        beta_member,
        alpha_admin_jwt: token(alpha_admin, UserRole::Viewer),
        alpha_member_jwt: token(alpha_member, UserRole::Viewer),
        beta_admin_jwt: token(beta_admin, UserRole::Viewer),
        unassigned_jwt: token(unassigned, UserRole::Viewer),
    }
}

async fn get(
    fixture: &Fixture,
    query: &str,
    token: Option<&str>,
) -> (StatusCode, HeaderMap, Value) {
    let mut request = Request::builder().uri(format!("/api/dashboard/usage{query}"));
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let response = fixture
        .app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        headers,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

const JANUARY: &str = "?from=2026-01-01&to=2026-01-31";

#[tokio::test]
async fn usage_scope_totals_pagination_and_coverage_are_computed_before_page_limit() {
    let fixture = fixture().await;
    let (status, headers, first) = get(
        &fixture,
        &format!("{JANUARY}&group_by=account&page=1&per_page=1"),
        Some(&fixture.alpha_admin_jwt),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(headers.get("cache-control").unwrap(), "no-store");
    assert_eq!(
        headers.get("x-llmlb-scope").unwrap().to_str().unwrap(),
        format!("organization:{}", fixture.alpha)
    );
    assert_eq!(first["total"], 3);
    assert_eq!(first["rows"].as_array().unwrap().len(), 1);
    assert_eq!(first["totals"]["request_count"], 7);
    assert_eq!(first["totals"]["input_tokens"], 206);
    assert_eq!(first["totals"]["output_tokens"], 409);
    assert_eq!(first["totals"]["total_tokens"], 615);
    assert_eq!(first["totals"]["measured_requests"], 4);
    assert_eq!(first["totals"]["estimated_requests"], 1);
    assert_eq!(first["totals"]["missing_requests"], 1);
    assert_eq!(first["totals"]["pending_requests"], 1);
    assert_eq!(first["totals"]["input_missing_count"], 1);
    assert_eq!(first["totals"]["output_missing_count"], 1);
    assert_eq!(first["totals"]["total_missing_count"], 1);
    assert_eq!(first["coverage_start"], "2026-01-01T00:00:00Z");
    let (_, _, second) = get(
        &fixture,
        &format!("{JANUARY}&group_by=account&page=2&per_page=1"),
        Some(&fixture.alpha_admin_jwt),
    )
    .await;
    assert_eq!(second["total"], 3);
    assert_eq!(second["totals"], first["totals"]);
    assert_ne!(
        first["rows"][0]["account_id"],
        second["rows"][0]["account_id"]
    );
    let (_, _, again) = get(
        &fixture,
        &format!("{JANUARY}&group_by=account&page=1&per_page=1"),
        Some(&fixture.alpha_admin_jwt),
    )
    .await;
    assert_eq!(again["rows"], first["rows"]);
    let (_, _, empty_page) = get(
        &fixture,
        &format!("{JANUARY}&group_by=account&page=99&per_page=1"),
        Some(&fixture.alpha_admin_jwt),
    )
    .await;
    assert!(empty_page["rows"].as_array().unwrap().is_empty());
    assert_eq!(empty_page["total"], 3);
    assert_eq!(empty_page["totals"], first["totals"]);
}

#[tokio::test]
async fn usage_scope_global_organization_and_self_totals_partition_without_legacy_leakage() {
    let fixture = fixture().await;
    let (status, _, global) = get(&fixture, JANUARY, Some(&fixture.global)).await;
    assert_eq!(status, StatusCode::OK, "{global}");
    assert_eq!(global["group_by"], "organization");
    assert_eq!(global["total"], 3);
    assert_eq!(global["totals"]["request_count"], 11);
    assert_eq!(global["totals"]["input_tokens"], 281);
    assert_eq!(global["totals"]["output_tokens"], 485);
    assert_eq!(global["totals"]["total_tokens"], 755);
    let legacy = global["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["organization_id"].is_null())
        .unwrap();
    assert_eq!(legacy["request_count"], 1);
    assert!(legacy["organization_name"].is_null());
    let (_, _, alpha) = get(&fixture, JANUARY, Some(&fixture.alpha_admin_jwt)).await;
    assert_eq!(alpha["total"], 1);
    assert_eq!(
        alpha["rows"][0]["organization_id"],
        fixture.alpha.to_string()
    );
    assert_eq!(alpha["rows"][0]["organization_name"], "Alpha");
    assert_eq!(alpha["totals"]["request_count"], 7);
    let (_, _, beta) = get(&fixture, JANUARY, Some(&fixture.beta_admin_jwt)).await;
    assert_eq!(beta["totals"]["request_count"], 3);
    assert_eq!(beta["totals"]["input_tokens"], 25);
    assert_eq!(beta["totals"]["output_missing_count"], 1);
    let (_, _, own) = get(
        &fixture,
        &format!("{JANUARY}&group_by=account"),
        Some(&fixture.alpha_member_jwt),
    )
    .await;
    assert_eq!(own["total"], 1);
    assert_eq!(
        own["rows"][0]["account_id"],
        fixture.alpha_member.to_string()
    );
    assert_eq!(own["totals"]["request_count"], 4);
    assert_eq!(own["totals"]["total_tokens"], 15);
}

#[tokio::test]
async fn usage_scope_cross_filters_are_rejected_and_keys_cannot_read_usage() {
    let fixture = fixture().await;
    for token in [&fixture.alpha_admin_jwt, &fixture.alpha_member_jwt] {
        assert_eq!(
            get(
                &fixture,
                &format!("{JANUARY}&organization_id={}", fixture.beta),
                Some(token)
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            get(
                &fixture,
                &format!("{JANUARY}&account_id={}", fixture.beta_member),
                Some(token)
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        get(
            &fixture,
            &format!("{JANUARY}&account_id={}", fixture.alpha_admin),
            Some(&fixture.alpha_member_jwt)
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        get(&fixture, JANUARY, Some(&fixture.unassigned_jwt))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        get(&fixture, JANUARY, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        get(&fixture, JANUARY, Some("sk_debug")).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, _, same) = get(
        &fixture,
        &format!("{JANUARY}&account_id={}", fixture.alpha_member),
        Some(&fixture.alpha_admin_jwt),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{same}");
    assert_eq!(same["total"], 1);
    assert_eq!(same["totals"]["request_count"], 4);
}

#[tokio::test]
async fn usage_scope_utc_dates_include_both_ui_endpoints_and_default_to_current_month() {
    let fixture = fixture().await;
    receipt(
        &fixture.pool,
        Some(fixture.alpha),
        Some(fixture.alpha_member),
        "2026-02-01T00:00:00Z",
        Some(UsageTokens::provider(Some(900), Some(1), Some(901))),
    )
    .await;
    let (status, _, last_day) = get(
        &fixture,
        "?from=2026-01-31&to=2026-01-31",
        Some(&fixture.alpha_admin_jwt),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{last_day}");
    assert_eq!(last_day["from"], "2026-01-31");
    assert_eq!(last_day["to"], "2026-01-31");
    assert_eq!(last_day["totals"]["request_count"], 2);
    assert_eq!(last_day["totals"]["total_tokens"], 600);
    let now = Utc::now();
    let first = Utc
        .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
        .unwrap();
    let end = if now.month() == 12 {
        Utc.with_ymd_and_hms(now.year() + 1, 1, 1, 0, 0, 0).unwrap()
    } else {
        Utc.with_ymd_and_hms(now.year(), now.month() + 1, 1, 0, 0, 0)
            .unwrap()
    };
    let (_, _, default) = get(&fixture, "", Some(&fixture.global)).await;
    assert_eq!(default["from"], first.date_naive().to_string());
    assert_eq!(
        default["to"],
        end.date_naive().pred_opt().unwrap().to_string()
    );
}

#[tokio::test]
async fn usage_scope_invalid_intervals_grouping_and_pagination_fail_closed() {
    let fixture = fixture().await;
    for query in [
        "?from=2026-02-30&to=2026-03-01",
        "?from=2026-1-01&to=2026-01-31",
        "?from=2026-01-02&to=2026-01-01",
        "?group_by=issuer",
        "?page=0",
        "?per_page=0",
        "?per_page=1001",
        "?from=2026-01-01T00:00:00Z",
    ] {
        assert_eq!(
            get(&fixture, query, Some(&fixture.global)).await.0,
            StatusCode::BAD_REQUEST,
            "{query}"
        );
    }
}

#[tokio::test]
async fn usage_scope_transfer_and_deletion_preserve_snapshots_without_following_current_membership()
{
    let fixture = fixture().await;
    organizations::set_membership(
        &fixture.pool,
        fixture.alpha_member,
        Some(fixture.beta),
        OrganizationRole::Member,
    )
    .await
    .unwrap();
    let (_, _, moved) = get(
        &fixture,
        &format!("{JANUARY}&group_by=account"),
        Some(&fixture.alpha_member_jwt),
    )
    .await;
    assert_eq!(moved["total"], 0);
    assert_eq!(moved["totals"]["request_count"], 0);
    let (_, _, beta) = get(&fixture, JANUARY, Some(&fixture.beta_admin_jwt)).await;
    assert_eq!(beta["totals"]["request_count"], 3);
    users::delete(&fixture.pool, fixture.alpha_peer)
        .await
        .unwrap();
    let (_, _, global) = get(
        &fixture,
        &format!(
            "{JANUARY}&group_by=account&organization_id={}",
            fixture.alpha
        ),
        Some(&fixture.global),
    )
    .await;
    assert_eq!(global["totals"]["request_count"], 7);
    let deleted = global["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["account_id"] == fixture.alpha_peer.to_string())
        .unwrap();
    assert_eq!(deleted["account_deleted"], true);
    assert!(deleted["account_name"].is_null());
    assert_eq!(deleted["organization_id"], fixture.alpha.to_string());
}

#[tokio::test]
async fn usage_scope_reads_rollups_without_raw_history_or_receipt_scans() {
    let fixture = fixture().await;
    sqlx::query("DELETE FROM request_history")
        .execute(&fixture.pool)
        .await
        .unwrap();
    // Receipt dedupe belongs to collection. Reporting must use the retained rollup.
    sqlx::query("DELETE FROM usage_receipts")
        .execute(&fixture.pool)
        .await
        .unwrap();
    let (status, _, report) = get(&fixture, JANUARY, Some(&fixture.global)).await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["totals"]["request_count"], 11);
    assert_eq!(report["totals"]["total_tokens"], 755);
}
