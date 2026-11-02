//! Cross-scope retention and cache identity regression contracts.

use super::*;
use crate::common::{
    protocol::{RequestResponseRecord, RequestType},
    usage::{TokenSource, UsageAttribution, UsageTokens},
};
use crate::db::organizations::{self, OrganizationRole};
use uuid::Uuid;

async fn account(
    state: &AppState,
    name: &str,
    role: UserRole,
    organization: Option<(Uuid, OrganizationRole)>,
) -> (Uuid, String) {
    let user = crate::db::users::create(&state.db_pool, name, "unused", role, false)
        .await
        .unwrap();
    if let Some((id, organization_role)) = organization {
        organizations::set_membership(&state.db_pool, user.id, Some(id), organization_role, None)
            .await
            .unwrap();
    }
    let token =
        crate::auth::jwt::create_jwt(&user.id.to_string(), role, &state.auth.jwt_secret, false, 0)
            .unwrap();
    (user.id, token)
}

fn history_record(timestamp: chrono::DateTime<chrono::Utc>) -> RequestResponseRecord {
    let mut record = RequestResponseRecord::new(
        Uuid::new_v4(),
        "retention endpoint".into(),
        "127.0.0.1".parse().unwrap(),
        "retention-model".into(),
        RequestType::Chat,
        serde_json::json!({"messages":[]}),
        StatusCode::OK,
        std::time::Duration::from_millis(1),
        None,
        None,
    );
    record.timestamp = timestamp;
    record.completed_at = timestamp;
    record.input_tokens = Some(11);
    record.output_tokens = Some(7);
    record.total_tokens = Some(18);
    record
}

#[tokio::test]
async fn durable_token_apis_keep_global_org_account_totals_after_raw_cleanup() {
    let state = test_state().await;
    let organization: Uuid = organizations::create(&state.db_pool, "Retention organization")
        .await
        .unwrap()
        .id
        .parse()
        .unwrap();
    let (_, global) = account(&state, "retention-global", UserRole::Admin, None).await;
    let (_, org) = account(
        &state,
        "retention-org",
        UserRole::Viewer,
        Some((organization, OrganizationRole::Admin)),
    )
    .await;
    let (member, own) = account(
        &state,
        "retention-member",
        UserRole::Viewer,
        Some((organization, OrganizationRole::Member)),
    )
    .await;
    let timestamp = chrono::Utc::now() - chrono::Duration::days(10);
    sqlx::query("UPDATE settings SET value = ? WHERE key = 'usage_observation_started_at'")
        .bind((timestamp - chrono::Duration::days(1)).to_rfc3339())
        .execute(&state.db_pool)
        .await
        .unwrap();
    let record = history_record(timestamp);
    let attribution = UsageAttribution {
        request_id: record.id,
        received_at: timestamp,
        organization_id: Some(organization),
        account_id: Some(member),
        api_key_id: None,
        auth_source: "jwt".into(),
    };
    let tokens = UsageTokens::from_usage(
        &crate::token::TokenUsage::new(Some(11), Some(7), Some(18)),
        TokenSource::Provider,
    );
    state
        .balancer
        .request_history
        .save_attributed_record(&record, &attribution, &tokens, true)
        .await
        .unwrap();
    let mut app = create_app(state.clone());
    for cleaned in [false, true] {
        if cleaned {
            state
                .balancer
                .request_history
                .cleanup_old_records(chrono::Duration::days(7))
                .await
                .unwrap();
            assert!(state
                .balancer
                .request_history
                .load_records()
                .await
                .unwrap()
                .is_empty());
        }
        for (scope, token) in [
            ("global", &global),
            ("organization", &org),
            ("account", &own),
        ] {
            for uri in [
                "/api/dashboard/stats/tokens",
                "/api/dashboard/stats/tokens/daily?days=30",
                "/api/dashboard/stats/tokens/monthly?months=2",
                "/api/dashboard/overview",
                "/api/dashboard/stats",
            ] {
                let (status, body) = tenant_read(&mut app, token, uri).await;
                assert_eq!(status, StatusCode::OK, "{scope} {uri}: {body}");
                let value: serde_json::Value = serde_json::from_str(&body).unwrap();
                if uri.ends_with("/overview") {
                    assert_eq!(
                        value["operations"]["total_requests"], 1,
                        "global and tenant request counters must share the observed ledger"
                    );
                } else if uri == "/api/dashboard/stats" {
                    assert_eq!(
                        value["total_requests"], 1,
                        "global and tenant request counters must share the observed ledger"
                    );
                }
                let total = if uri.contains("/daily") || uri.contains("/monthly") {
                    value
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|row| row["total_tokens"].as_u64().unwrap())
                        .sum::<u64>()
                } else if uri.ends_with("/overview") {
                    value["operations"]["total_tokens"].as_u64().unwrap()
                } else {
                    value["total_tokens"].as_u64().unwrap()
                };
                assert_eq!(total, 18, "scope={scope}, raw_cleaned={cleaned}, uri={uri}");
            }
            let usage = format!(
                "/api/dashboard/usage?group_by=account&from={}&to={}",
                timestamp.date_naive(),
                chrono::Utc::now().date_naive()
            );
            let (status, body) = tenant_read(&mut app, token, &usage).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&body).unwrap()["totals"]["total_tokens"],
                18
            );
        }
    }
}

#[tokio::test]
async fn pre_observation_raw_history_is_visible_without_backfilling_usage_totals() {
    let state = test_state().await;
    let (_, global) = account(&state, "legacy-raw-global", UserRole::Admin, None).await;
    let record = history_record(chrono::Utc::now() - chrono::Duration::days(2));
    state
        .balancer
        .request_history
        .save_record(&record)
        .await
        .unwrap();
    // Model an existing pre-migration conversation: raw remains, ledger is absent.
    sqlx::query("DELETE FROM usage_receipts WHERE id = ?")
        .bind(record.id.to_string())
        .execute(&state.db_pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM usage_daily")
        .execute(&state.db_pool)
        .await
        .unwrap();
    let mut app = create_app(state);
    let (status, body) = tenant_read(&mut app, &global, "/api/dashboard/request-responses").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body).unwrap()["total_count"],
        1
    );
    let (status, body) = tenant_read(&mut app, &global, "/api/dashboard/stats/tokens").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body).unwrap()["total_tokens"],
        0,
        "unknown historical attribution must not be guessed into the observation ledger"
    );
}

#[tokio::test]
async fn organization_and_key_reads_stamp_the_same_current_scope_as_their_sql() {
    let state = test_state().await;
    let org_a: Uuid = organizations::create(&state.db_pool, "Header reads A")
        .await
        .unwrap()
        .id
        .parse()
        .unwrap();
    let org_b: Uuid = organizations::create(&state.db_pool, "Header reads B")
        .await
        .unwrap()
        .id
        .parse()
        .unwrap();
    let (user, token) = account(
        &state,
        "organization-read-header",
        UserRole::Viewer,
        Some((org_a, OrganizationRole::Admin)),
    )
    .await;
    let mut app = create_app(state.clone());
    for (organization, role, expected) in [
        (
            org_a,
            OrganizationRole::Admin,
            format!("organization:{org_a}"),
        ),
        (
            org_b,
            OrganizationRole::Member,
            format!("account:{org_b}:{user}"),
        ),
    ] {
        organizations::set_membership(&state.db_pool, user, Some(organization), role, None)
            .await
            .unwrap();
        for uri in [
            "/api/organizations".to_string(),
            format!("/api/organizations/{organization}/members"),
            "/api/me/api-keys".to_string(),
        ] {
            let response = app
                .call(
                    Request::builder()
                        .uri(&uri)
                        .header("authorization", format!("Bearer {token}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{uri}");
            assert_eq!(
                response
                    .headers()
                    .get("x-llmlb-usage-scope")
                    .and_then(|v| v.to_str().ok()),
                Some(expected.as_str()),
                "{uri}"
            );
            assert!(
                response
                    .headers()
                    .get("cache-control")
                    .and_then(|v| v.to_str().ok())
                    .is_some_and(|v| v.contains("no-store")),
                "{uri}"
            );
        }
    }
}

#[tokio::test]
async fn former_organization_usage_does_not_disclose_transferred_accounts_current_name() {
    let state = test_state().await;
    let org_a: Uuid = organizations::create(&state.db_pool, "Metadata A")
        .await
        .unwrap()
        .id
        .parse()
        .unwrap();
    let org_b: Uuid = organizations::create(&state.db_pool, "Metadata B")
        .await
        .unwrap()
        .id
        .parse()
        .unwrap();
    let (_, global) = account(&state, "metadata-global", UserRole::Admin, None).await;
    let (_, org_admin) = account(
        &state,
        "metadata-a-admin",
        UserRole::Viewer,
        Some((org_a, OrganizationRole::Admin)),
    )
    .await;
    let (member, _) = account(
        &state,
        "metadata-before-transfer",
        UserRole::Viewer,
        Some((org_a, OrganizationRole::Member)),
    )
    .await;
    let record = history_record(chrono::Utc::now());
    let attribution = UsageAttribution {
        request_id: record.id,
        received_at: record.timestamp,
        organization_id: Some(org_a),
        account_id: Some(member),
        api_key_id: None,
        auth_source: "jwt".into(),
    };
    let tokens = UsageTokens::from_usage(
        &crate::token::TokenUsage::new(Some(11), Some(7), Some(18)),
        TokenSource::Provider,
    );
    state
        .balancer
        .request_history
        .save_attributed_record(&record, &attribution, &tokens, true)
        .await
        .unwrap();
    let mut app = create_app(state);
    let uri = format!("/api/dashboard/usage?group_by=account&organization_id={org_a}");
    let (status, body) = tenant_read(&mut app, &org_admin, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body).unwrap()["rows"][0]["account_name"],
        "metadata-before-transfer"
    );
    for (path, body) in [
        (
            format!("/api/users/{member}/membership"),
            serde_json::json!({"organization_id":org_b,"organization_role":"member"}),
        ),
        (
            format!("/api/users/{member}"),
            serde_json::json!({"username":"metadata-b-private-current@example.com","email":"private-b@example.test"}),
        ),
    ] {
        let response = app
            .call(
                Request::builder()
                    .method("PUT")
                    .uri(path)
                    .header("authorization", format!("Bearer {global}"))
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
    let (status, body) = tenant_read(&mut app, &org_admin, &uri).await;
    assert_eq!(status, StatusCode::OK);
    let value: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(value["totals"]["request_count"], 1);
    assert_eq!(value["rows"][0]["account_id"], member.to_string());
    assert!(value["rows"][0]["account_name"].is_null(),"the former organization must not receive the destination organization's current private name: {body}");
    assert_eq!(
        value["rows"][0]["account_deleted"], false,
        "moving is distinct from account deletion"
    );
    assert!(!body.contains("private-b@example.test"));
    let (status, body) = tenant_read(&mut app, &global, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body).unwrap()["rows"][0]["account_name"],
        "metadata-b-private-current@example.com"
    );
}
