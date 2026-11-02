//! Organization isolation is enforced by the real router, before data is read.

use super::*;

mod regressions;

#[tokio::test]
async fn unassigned_viewer_cannot_read_usage_history_or_client_analytics() {
    let state = test_state().await;
    let viewer = crate::db::users::create(
        &state.db_pool,
        "unassigned-scope-viewer",
        "unused-password-hash",
        UserRole::Viewer,
        false,
    )
    .await
    .unwrap();
    let token = crate::auth::jwt::create_jwt(
        &viewer.id.to_string(),
        UserRole::Viewer,
        &state.auth.jwt_secret,
        false,
        0,
    )
    .unwrap();
    let mut app = create_app(state);
    for uri in [
        "/api/dashboard/request-responses",
        "/api/dashboard/request-responses/export",
        "/api/dashboard/stats/tokens",
        "/api/dashboard/stats/tokens/daily",
        "/api/dashboard/stats/tokens/monthly",
        "/api/dashboard/clients",
        "/api/dashboard/clients/timeline",
        "/api/dashboard/clients/models",
        "/api/dashboard/clients/heatmap",
        "/api/dashboard/clients/192.0.2.1/detail",
        "/api/dashboard/clients/192.0.2.1/api-keys",
    ] {
        let response = app
            .call(
                Request::builder()
                    .uri(uri)
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{uri}");
    }
}

#[tokio::test]
async fn api_key_cannot_read_endpoint_usage_counters() {
    let state = test_state().await;
    let mut app = create_app(state);
    let endpoint = uuid::Uuid::new_v4();
    for resource in ["today-stats", "daily-stats", "model-stats", "model-tps"] {
        let response = app
            .call(
                Request::builder()
                    .uri(format!("/api/endpoints/{endpoint}/{resource}"))
                    .header("authorization", "Bearer sk_debug")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{resource}");
    }
}

#[tokio::test]
async fn usage_totals_are_filtered_before_group_pagination_and_use_inclusive_utc_dates() {
    let state = test_state().await;
    let admin = crate::db::users::create(
        &state.db_pool,
        "usage-global-admin",
        "unused-password-hash",
        UserRole::Admin,
        false,
    )
    .await
    .unwrap();
    let organization_a = uuid::Uuid::new_v4().to_string();
    let organization_b = uuid::Uuid::new_v4().to_string();
    for (organization, account, date, count) in [
        (&organization_a, uuid::Uuid::new_v4(), "2026-10-01", 2_i64),
        (&organization_a, uuid::Uuid::new_v4(), "2026-10-31", 3),
        (&organization_b, uuid::Uuid::new_v4(), "2026-10-15", 5),
        (&organization_a, uuid::Uuid::new_v4(), "2026-11-01", 100),
    ] {
        sqlx::query("INSERT INTO usage_daily (date, organization_id, account_id, request_count, input_tokens, output_tokens, total_tokens, provider_count) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(date)
            .bind(organization)
            .bind(account.to_string())
            .bind(count)
            .bind(count * 4)
            .bind(count * 6)
            .bind(count * 10)
            .bind(count)
            .execute(&state.db_pool)
            .await
            .unwrap();
    }
    let token = crate::auth::jwt::create_jwt(
        &admin.id.to_string(),
        UserRole::Admin,
        &state.auth.jwt_secret,
        false,
        0,
    )
    .unwrap();
    let mut app = create_app(state);
    for (filter, expected_count, expected_requests) in [
        (String::new(), 3_u64, 10_u64),
        (format!("&organization_id={organization_a}"), 2, 5),
    ] {
        let response = app
            .call(
                Request::builder()
                    .uri(format!("/api/dashboard/usage?group_by=account&from=2026-10-01&to=2026-10-31&page=2&per_page=1{filter}"))
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let page: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(page["total_count"], expected_count);
        assert_eq!(page["rows"].as_array().unwrap().len(), 1);
        assert_eq!(page["totals"]["request_count"], expected_requests);
        assert_eq!(page["totals"]["total_tokens"], expected_requests * 10);
        assert_eq!(page["totals"]["measured_requests"], expected_requests);
        assert_eq!(page["to_exclusive"], "2026-11-01");
        assert!(page["coverage_start"].is_string());
    }
}

async fn tenant_read(app: &mut axum::Router, token: &str, uri: &str) -> (StatusCode, String) {
    let response = app
        .call(
            Request::builder()
                .uri(uri)
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

#[tokio::test]
async fn usage_scope_header_tracks_current_membership_on_success_and_handler_error() {
    use crate::db::organizations::{self, OrganizationRole};
    use uuid::Uuid;
    let state = test_state().await;
    let org_a: Uuid = organizations::create(&state.db_pool, "Header A")
        .await
        .unwrap()
        .id
        .parse()
        .unwrap();
    let org_b: Uuid = organizations::create(&state.db_pool, "Header B")
        .await
        .unwrap()
        .id
        .parse()
        .unwrap();
    let user = crate::db::users::create(
        &state.db_pool,
        "scope-header-member",
        "unused",
        UserRole::Viewer,
        false,
    )
    .await
    .unwrap();
    organizations::set_membership(
        &state.db_pool,
        user.id,
        Some(org_a),
        OrganizationRole::Member,
        None,
    )
    .await
    .unwrap();
    let token = crate::auth::jwt::create_jwt(
        &user.id.to_string(),
        UserRole::Viewer,
        &state.auth.jwt_secret,
        false,
        0,
    )
    .unwrap();
    let mut app = create_app(state.clone());
    for (organization, uri, status) in [
        (org_a, "/api/dashboard/usage".to_string(), StatusCode::OK),
        (
            org_b,
            format!("/api/dashboard/usage?organization_id={org_a}"),
            StatusCode::FORBIDDEN,
        ),
    ] {
        organizations::set_membership(
            &state.db_pool,
            user.id,
            Some(organization),
            OrganizationRole::Member,
            None,
        )
        .await
        .unwrap();
        let response = app
            .call(
                Request::builder()
                    .uri(uri)
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        let expected = format!("account:{organization}:{}", user.id);
        assert_eq!(
            response
                .headers()
                .get("x-llmlb-usage-scope")
                .and_then(|value| value.to_str().ok()),
            Some(expected.as_str())
        );
        assert!(
            response
                .headers()
                .get("cache-control")
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.contains("no-store")),
            "scope-specific responses must not enter a shared HTTP cache"
        );
    }
    for (name, role, expected) in [
        ("scope-header-global", UserRole::Admin, "global".to_string()),
        (
            "scope-header-org-admin",
            UserRole::Viewer,
            format!("organization:{org_a}"),
        ),
    ] {
        let account = crate::db::users::create(&state.db_pool, name, "unused", role, false)
            .await
            .unwrap();
        if role == UserRole::Viewer {
            organizations::set_membership(
                &state.db_pool,
                account.id,
                Some(org_a),
                OrganizationRole::Admin,
                None,
            )
            .await
            .unwrap();
        }
        let token = crate::auth::jwt::create_jwt(
            &account.id.to_string(),
            role,
            &state.auth.jwt_secret,
            false,
            0,
        )
        .unwrap();
        let response = app
            .call(
                Request::builder()
                    .uri("/api/dashboard/usage")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("x-llmlb-usage-scope")
                .and_then(|value| value.to_str().ok()),
            Some(expected.as_str())
        );
    }
}

#[tokio::test]
async fn two_organization_history_export_usage_and_client_counts_follow_server_scope() {
    use crate::common::{
        protocol::{RequestResponseRecord, RequestType},
        usage::{TokenSource, UsageAttribution, UsageTokens},
    };
    use crate::db::organizations::{self, OrganizationRole};
    use uuid::Uuid;
    let state = test_state().await;
    let org_a: Uuid = organizations::create(&state.db_pool, "Scope A")
        .await
        .unwrap()
        .id
        .parse()
        .unwrap();
    let org_b: Uuid = organizations::create(&state.db_pool, "Scope B")
        .await
        .unwrap()
        .id
        .parse()
        .unwrap();
    let mut users = Vec::new();
    for (name, org, role) in [
        ("scope-a-admin", org_a, OrganizationRole::Admin),
        ("scope-a-one", org_a, OrganizationRole::Member),
        ("scope-a-two", org_a, OrganizationRole::Member),
        ("scope-b-admin", org_b, OrganizationRole::Admin),
        ("scope-b-one", org_b, OrganizationRole::Member),
    ] {
        let user =
            crate::db::users::create(&state.db_pool, name, "unused", UserRole::Viewer, false)
                .await
                .unwrap();
        organizations::set_membership(&state.db_pool, user.id, Some(org), role, None)
            .await
            .unwrap();
        users.push(user);
    }
    let mut ids = Vec::new();
    for (index, org) in [(1, org_a), (2, org_a), (4, org_b)] {
        let mut record = RequestResponseRecord::new(
            Uuid::new_v4(),
            "shared endpoint".into(),
            "127.0.0.1".parse().unwrap(),
            format!("private-model-{index}"),
            RequestType::Chat,
            serde_json::json!({"messages":[{"content":format!("private-body-{index}")}]}),
            StatusCode::OK,
            std::time::Duration::from_millis(1),
            None,
            None,
        );
        record.client_ip = Some("192.0.2.10".parse().unwrap());
        let attribution = UsageAttribution {
            request_id: record.id,
            received_at: record.timestamp,
            organization_id: Some(org),
            account_id: Some(users[index].id),
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
        ids.push(record.id);
    }
    let token_for = |index: usize| {
        crate::auth::jwt::create_jwt(
            &users[index].id.to_string(),
            UserRole::Viewer,
            &state.auth.jwt_secret,
            false,
            0,
        )
        .unwrap()
    };
    let endpoint = crate::types::endpoint::Endpoint::new(
        "shared-metrics".into(),
        "http://127.0.0.1:19999".into(),
        crate::types::endpoint::EndpointType::OpenaiCompatible,
    );
    let endpoint_id = endpoint.id;
    state
        .balancer
        .endpoint_registry
        .add(endpoint)
        .await
        .unwrap();
    state
        .balancer
        .load_manager
        .record_metrics(crate::balancer::MetricsUpdate {
            endpoint_id,
            cpu_usage: 0.0,
            memory_usage: 0.0,
            gpu_usage: None,
            gpu_memory_usage: None,
            gpu_memory_total_mb: None,
            gpu_memory_used_mb: None,
            gpu_temperature: None,
            gpu_model_name: None,
            gpu_compute_capability: None,
            gpu_capability_score: None,
            active_requests: 97,
            average_response_time_ms: Some(999.0),
            initializing: false,
            ready_models: None,
        })
        .await
        .unwrap();
    let mut app = create_app(state.clone());
    for (actor, visible) in [(0, 2_u64), (1, 1)] {
        let token = token_for(actor);
        let (status, body) = tenant_read(
            &mut app,
            &token,
            &format!("/api/dashboard/metrics/{endpoint_id}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&body).unwrap(),
            serde_json::json!([]),
            "shared health history must not expose global request counters"
        );
        for uri in [
            "/api/dashboard/request-responses?per_page=10&organization_id=spoofed",
            "/api/dashboard/request-responses/export?format=json",
        ] {
            let (status, body) = tenant_read(&mut app, &token, uri).await;
            assert_eq!(status, StatusCode::OK, "{uri}: {body}");
            assert!(
                !body.contains("private-model-4"),
                "foreign organization model leaked"
            );
            if actor == 1 {
                assert!(!body.contains("private-model-2"), "another account leaked");
            }
            let value: serde_json::Value = serde_json::from_str(&body).unwrap();
            if uri.contains("export") {
                assert_eq!(value.as_array().unwrap().len() as u64, visible);
            } else {
                assert_eq!(value["total_count"], visible);
            }
        }
        for uri in [
            "/api/dashboard/usage?group_by=account",
            "/api/dashboard/overview",
            "/api/dashboard/activity",
            "/api/dashboard/stats/tokens",
            "/api/dashboard/clients",
            "/api/dashboard/clients/192.0.2.10/detail",
        ] {
            let (status, body) = tenant_read(&mut app, &token, uri).await;
            assert_eq!(status, StatusCode::OK, "{uri}: {body}");
            assert!(!body.contains("private-model-4"), "{uri}");
            let value: serde_json::Value = serde_json::from_str(&body).unwrap();
            match uri {
                "/api/dashboard/usage?group_by=account" => {
                    assert_eq!(value["totals"]["request_count"], visible)
                }
                "/api/dashboard/overview" => {
                    assert_eq!(value["operations"]["total_requests"], visible)
                }
                "/api/dashboard/activity" => {
                    assert_eq!(
                        value["total"]["requests_per_second"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| v.as_u64().unwrap())
                            .sum::<u64>(),
                        visible
                    );
                    assert_eq!(value["total"]["active_requests"], 0);
                }
                "/api/dashboard/stats/tokens" => assert_eq!(value["total_tokens"], visible * 18),
                "/api/dashboard/clients" => {
                    assert_eq!(value["total_count"], 1);
                    assert_eq!(value["rankings"][0]["request_count"], visible);
                }
                _ => assert_eq!(value["total_requests"], visible),
            }
        }
        let (status, _) = tenant_read(
            &mut app,
            &token,
            &format!("/api/dashboard/request-responses/{}", ids[2]),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = tenant_read(
            &mut app,
            &token,
            &format!("/api/dashboard/usage?organization_id={org_b}"),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
    // Moving the account changes future reads, without reassigning its old rows.
    organizations::set_membership(
        &state.db_pool,
        users[1].id,
        Some(org_b),
        OrganizationRole::Member,
        None,
    )
    .await
    .unwrap();
    let token = token_for(1);
    let (status, body) = tenant_read(&mut app, &token, "/api/dashboard/request-responses").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body).unwrap()["total_count"],
        0
    );
    let (status, body) = tenant_read(
        &mut app,
        &token_for(3),
        "/api/dashboard/usage?group_by=account",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body).unwrap()["totals"]["request_count"],
        1
    );
}
