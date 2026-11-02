//! Issue #947: every pre-existing statistics/history read must respect receipt scope.

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use llmlb::{
    common::{
        auth::UserRole,
        protocol::{RequestResponseRecord, RequestType},
        usage::{UsageAttribution, UsageTokens},
    },
    db::{
        organizations::{self, OrganizationRole},
        request_history::RequestHistoryStorage,
        users,
    },
};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use tower::ServiceExt;
use uuid::Uuid;

struct Fixture {
    app: Router,
    manager: llmlb::balancer::LoadManager,
    pool: SqlitePool,
    global: String,
    org_admin: String,
    member: String,
    unassigned: String,
    alpha: Uuid,
    member_id: Uuid,
    beta_record: Uuid,
    legacy_record: Uuid,
    endpoint: Uuid,
    foreign_key: Uuid,
}

fn token(account: Uuid, role: UserRole) -> String {
    llmlb::auth::jwt::create_jwt(
        &account.to_string(),
        role,
        &crate::support::lb::test_jwt_secret(),
        false,
        0,
    )
    .unwrap()
}

async fn member(pool: &SqlitePool, name: &str, org: Option<Uuid>, role: OrganizationRole) -> Uuid {
    let user = users::create(pool, name, "hash", UserRole::Viewer, false)
        .await
        .unwrap();
    organizations::set_membership(pool, user.id, org, role)
        .await
        .unwrap();
    user.id
}

async fn fixture() -> Fixture {
    let pool = crate::support::lb::create_test_db_pool().await;
    let global_id = users::create(&pool, "global", "hash", UserRole::Admin, false)
        .await
        .unwrap()
        .id;
    let alpha = organizations::create(&pool, "Alpha").await.unwrap().id;
    let beta = organizations::create(&pool, "Beta").await.unwrap().id;
    let admin_id = member(&pool, "alpha-admin", Some(alpha), OrganizationRole::Admin).await;
    let member_id = member(&pool, "alpha-member", Some(alpha), OrganizationRole::Member).await;
    let peer_id = member(&pool, "alpha-peer", Some(alpha), OrganizationRole::Member).await;
    let foreign_id = member(&pool, "beta-member", Some(beta), OrganizationRole::Member).await;
    let unassigned = member(&pool, "unassigned", None, OrganizationRole::Member).await;
    let endpoint = llmlb::types::endpoint::Endpoint::new(
        "shared inventory".into(),
        format!("http://127.0.0.1/{}", Uuid::new_v4()),
        llmlb::types::endpoint::EndpointType::OpenaiCompatible,
    );
    llmlb::db::endpoints::create_endpoint(&pool, &endpoint)
        .await
        .unwrap();
    let storage = RequestHistoryStorage::new(pool.clone());
    let alpha_admin_key = Uuid::new_v4();
    let alpha_member_key = Uuid::new_v4();
    let foreign_key = Uuid::new_v4();
    let mut beta_record = Uuid::nil();
    let mut legacy_record = Uuid::nil();
    let cases = [
        (
            Some(alpha),
            Some(admin_id),
            "alpha-private",
            "192.0.2.10",
            5,
            alpha_admin_key,
        ),
        (
            Some(alpha),
            Some(member_id),
            "alpha-private",
            "192.0.2.10",
            5,
            alpha_member_key,
        ),
        (
            Some(alpha),
            Some(member_id),
            "alpha-private",
            "192.0.2.10",
            5,
            alpha_member_key,
        ),
        (
            Some(alpha),
            Some(peer_id),
            "alpha-private",
            "192.0.2.10",
            5,
            alpha_admin_key,
        ),
        (
            Some(beta),
            Some(foreign_id),
            "beta-secret",
            "192.0.2.10",
            50,
            foreign_key,
        ),
        (
            Some(beta),
            Some(foreign_id),
            "beta-secret",
            "192.0.2.99",
            50,
            foreign_key,
        ),
        (
            None,
            None,
            "legacy-secret",
            "192.0.2.200",
            500,
            Uuid::new_v4(),
        ),
    ];
    for (org, account, marker, ip, total, key) in cases {
        let attribution = UsageAttribution {
            request_id: Uuid::new_v4(),
            account_id: account,
            organization_id: org,
            received_at: chrono::Utc::now(),
            api_key_id: Some(key),
            auth_source: "api_key".into(),
        };
        let mut record = RequestResponseRecord::new(
            endpoint.id,
            endpoint.name.clone(),
            "127.0.0.1".parse().unwrap(),
            marker.into(),
            RequestType::Chat,
            json!({"model":marker,"prompt":marker}),
            StatusCode::OK,
            std::time::Duration::from_millis(100),
            Some(ip.parse().unwrap()),
            Some(key),
        );
        record.response_body = Some(json!({"content": marker}));
        let tokens = UsageTokens::provider(Some(total * 2 / 5), Some(total * 3 / 5), Some(total));
        storage.accept_usage(&attribution, &record).await.unwrap();
        storage
            .finish_usage(&attribution, &record, &tokens)
            .await
            .unwrap();
        llmlb::db::endpoints::increment_request_counters(&pool, endpoint.id, true)
            .await
            .unwrap();
        llmlb::db::endpoint_daily_stats::upsert_daily_stats(
            &pool,
            endpoint.id,
            marker,
            &chrono::Utc::now().format("%Y-%m-%d").to_string(),
            true,
            total * 3 / 5,
            100,
        )
        .await
        .unwrap();
        if org == Some(beta) {
            beta_record = attribution.request_id;
        }
        if org.is_none() {
            legacy_record = attribution.request_id;
        }
    }
    let (app, manager) = crate::support::lb::build_test_router_with_manager(pool.clone()).await;
    Fixture {
        app,
        manager,
        pool,
        global: token(global_id, UserRole::Admin),
        org_admin: token(admin_id, UserRole::Viewer),
        member: token(member_id, UserRole::Viewer),
        unassigned: token(unassigned, UserRole::Viewer),
        alpha,
        member_id,
        beta_record,
        legacy_record,
        endpoint: endpoint.id,
        foreign_key,
    }
}

async fn raw(fixture: &Fixture, path: &str, jwt: &str) -> (StatusCode, String) {
    let response = fixture
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(path)
                .header("authorization", format!("Bearer {jwt}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

async fn get(fixture: &Fixture, path: &str, jwt: &str) -> (StatusCode, Value) {
    let (status, body) = raw(fixture, path, jwt).await;
    (status, serde_json::from_str(&body).unwrap_or(Value::Null))
}

#[tokio::test]
async fn read_outcomes_survive_raw_cleanup_and_router_restart() {
    let mut fixture = fixture().await;
    for (jwt, expected) in [
        (&fixture.global, 7),
        (&fixture.org_admin, 4),
        (&fixture.member, 2),
    ] {
        let (_, stats) = get(&fixture, "/api/dashboard/stats", jwt).await;
        assert_eq!(stats["successful_requests"], expected);
    }
    RequestHistoryStorage::new(fixture.pool.clone())
        .cleanup_old_records(chrono::Duration::zero())
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM request_history")
            .fetch_one(&fixture.pool)
            .await
            .unwrap(),
        0
    );
    fixture.app = crate::support::lb::build_test_router(fixture.pool.clone()).await;
    for (jwt, expected) in [
        (&fixture.global, 7),
        (&fixture.org_admin, 4),
        (&fixture.member, 2),
    ] {
        let (status, stats) = get(&fixture, "/api/dashboard/stats", jwt).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(stats["total_requests"], expected);
        assert_eq!(
            stats["successful_requests"], expected,
            "raw retention must not erase known terminal outcomes"
        );
        assert_eq!(stats["failed_requests"], 0);
    }
}

#[tokio::test]
async fn read_outcomes_minute_series_excludes_pending_error_sentinel() {
    let fixture = fixture().await;
    let attribution = UsageAttribution {
        request_id: Uuid::new_v4(),
        account_id: Some(fixture.member_id),
        organization_id: Some(fixture.alpha),
        received_at: chrono::Utc::now(),
        api_key_id: None,
        auth_source: "jwt".into(),
    };
    let record = RequestResponseRecord::error(
        "pending".into(),
        RequestType::Chat,
        json!({"model":"pending"}),
        "Accepted request has not completed".into(),
        0,
        None,
        None,
    );
    RequestHistoryStorage::new(fixture.pool.clone())
        .accept_usage(&attribution, &record)
        .await
        .unwrap();
    let (status, history) = get(&fixture, "/api/dashboard/request-history", &fixture.global).await;
    assert_eq!(status, StatusCode::OK);
    let rows = history.as_array().unwrap();
    assert_eq!(
        rows.iter()
            .map(|row| row["success"].as_u64().unwrap())
            .sum::<u64>(),
        7
    );
    assert_eq!(
        rows.iter()
            .map(|row| row["error"].as_u64().unwrap())
            .sum::<u64>(),
        0,
        "a pending receipt is not a completed failure"
    );
}

#[tokio::test]
async fn read_outcomes_recovered_unknown_is_not_an_endpoint_failure() {
    let fixture = fixture().await;
    let attribution = UsageAttribution {
        request_id: Uuid::new_v4(),
        account_id: Some(fixture.member_id),
        organization_id: Some(fixture.alpha),
        received_at: chrono::Utc::now(),
        api_key_id: None,
        auth_source: "jwt".into(),
    };
    let mut record = RequestResponseRecord::new(
        fixture.endpoint,
        "shared".into(),
        "127.0.0.1".parse().unwrap(),
        "recovered-unknown".into(),
        RequestType::Chat,
        json!({"model":"recovered-unknown"}),
        StatusCode::OK,
        std::time::Duration::ZERO,
        None,
        None,
    );
    record.status = llmlb::common::protocol::RecordStatus::Error {
        message: "Accepted request has not completed".into(),
    };
    RequestHistoryStorage::new(fixture.pool.clone())
        .accept_usage(&attribution, &record)
        .await
        .unwrap();
    llmlb::db::usage::recover_incomplete(&fixture.pool)
        .await
        .unwrap();
    let (_, stats) = get(&fixture, "/api/dashboard/stats", &fixture.global).await;
    assert_eq!(stats["total_requests"], 8);
    assert_eq!(stats["successful_requests"], 7);
    assert_eq!(stats["failed_requests"], 0);
    assert_eq!(stats["unknown_outcome_requests"], 1);
    let (_, today) = get(
        &fixture,
        &format!("/api/endpoints/{}/today-stats", fixture.endpoint),
        &fixture.global,
    )
    .await;
    assert_eq!(today["total_requests"], 8);
    assert_eq!(today["successful_requests"], 7);
    assert_eq!(
        today["failed_requests"], 0,
        "a recovered unknown outcome must not inherit the raw pending error sentinel"
    );
    let (_, models) = get(
        &fixture,
        &format!("/api/endpoints/{}/model-stats", fixture.endpoint),
        &fixture.global,
    )
    .await;
    let row = models
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["model_id"] == "recovered-unknown")
        .unwrap();
    assert_eq!(row["successful_requests"], 0);
    assert_eq!(row["failed_requests"], 0);
    assert_eq!(row["total_duration_ms"], 0);
}

#[tokio::test]
async fn read_boundaries_history_count_detail_search_and_export_are_scoped() {
    let fixture = fixture().await;
    for (jwt, expected) in [(&fixture.org_admin, 4), (&fixture.member, 2)] {
        let (status, list) = get(
            &fixture,
            "/api/dashboard/request-responses?page=1&per_page=10",
            jwt,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{list}");
        assert_eq!(list["total_count"], expected);
        assert_eq!(list["records"].as_array().unwrap().len(), expected as usize);
        assert!(!list.to_string().contains("beta-secret"));
        assert!(!list.to_string().contains("legacy-secret"));
        let (_, search) = get(
            &fixture,
            "/api/dashboard/request-responses?model=beta-secret",
            jwt,
        )
        .await;
        assert_eq!(search["total_count"], 0);
        for id in [fixture.beta_record, fixture.legacy_record, Uuid::new_v4()] {
            assert_eq!(
                get(
                    &fixture,
                    &format!("/api/dashboard/request-responses/{id}"),
                    jwt
                )
                .await
                .0,
                StatusCode::NOT_FOUND
            );
        }
        let (status, export) = get(
            &fixture,
            "/api/dashboard/request-responses/export?format=json",
            jwt,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{export}");
        assert_eq!(export.as_array().unwrap().len(), expected as usize);
        let (status, csv) = raw(
            &fixture,
            "/api/dashboard/request-responses/export?format=csv",
            jwt,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(!csv.contains("beta-secret"));
        assert!(!csv.contains("legacy-secret"));
    }
    assert_eq!(
        get(
            &fixture,
            &format!("/api/dashboard/request-responses/{}", fixture.beta_record),
            &fixture.global
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn read_boundaries_global_org_and_self_token_views_survive_raw_cleanup() {
    let fixture = fixture().await;
    for cleanup in [false, true] {
        if cleanup {
            sqlx::query("DELETE FROM request_history")
                .execute(&fixture.pool)
                .await
                .unwrap();
        }
        for (jwt, total, requests) in [
            (&fixture.global, 620, 7),
            (&fixture.org_admin, 20, 4),
            (&fixture.member, 10, 2),
        ] {
            let (status, tokens) = get(&fixture, "/api/dashboard/stats/tokens", jwt).await;
            assert_eq!(status, StatusCode::OK, "{tokens}");
            assert_eq!(tokens["total_tokens"], total, "cleanup={cleanup}");
            for (path, date_field) in [
                ("/api/dashboard/stats/tokens/daily", "date"),
                ("/api/dashboard/stats/tokens/monthly", "month"),
            ] {
                let (status, rows) = get(&fixture, path, jwt).await;
                assert_eq!(status, StatusCode::OK, "{rows}");
                let rows = rows.as_array().unwrap();
                assert!(rows.iter().all(|row| row[date_field].is_string()));
                assert_eq!(
                    rows.iter()
                        .map(|row| row["total_tokens"].as_u64().unwrap())
                        .sum::<u64>(),
                    total as u64
                );
                assert_eq!(
                    rows.iter()
                        .map(|row| row["request_count"].as_u64().unwrap())
                        .sum::<u64>(),
                    requests as u64
                );
            }
        }
    }
}

#[tokio::test]
async fn read_boundaries_overview_stats_and_minute_history_never_use_global_fallback() {
    let fixture = fixture().await;
    // Populate any global persisted/memory cache before making scoped requests.
    assert_eq!(
        get(&fixture, "/api/dashboard/overview", &fixture.global)
            .await
            .0,
        StatusCode::OK
    );
    for (jwt, count, total) in [(&fixture.org_admin, 4, 20), (&fixture.member, 2, 10)] {
        let (status, stats) = get(&fixture, "/api/dashboard/stats", jwt).await;
        assert_eq!(status, StatusCode::OK, "{stats}");
        assert_eq!(stats["total_requests"], count);
        assert_eq!(stats["total_tokens"], total);
        let (status, overview) = get(&fixture, "/api/dashboard/overview", jwt).await;
        assert_eq!(status, StatusCode::OK, "{overview}");
        assert_eq!(overview["operations"]["total_requests"], count);
        assert_eq!(overview["operations"]["total_tokens"], total);
        let (status, activity) = get(&fixture, "/api/dashboard/activity", jwt).await;
        assert_eq!(status, StatusCode::OK, "{activity}");
        assert_eq!(
            activity["total"]["requests_per_second"]
                .as_array()
                .unwrap()
                .iter()
                .map(|point| point.as_u64().unwrap())
                .sum::<u64>(),
            count as u64
        );
        let (status, history) = get(&fixture, "/api/dashboard/request-history", jwt).await;
        assert_eq!(status, StatusCode::OK, "{history}");
        assert_eq!(
            history
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row["success"].as_u64().unwrap_or(0)
                    + row["error"].as_u64().unwrap_or(0))
                .sum::<u64>(),
            count as u64
        );
    }
}

#[tokio::test]
async fn read_boundaries_clients_rankings_counts_distributions_and_key_counts_are_scoped() {
    let fixture = fixture().await;
    for (jwt, count, keys) in [(&fixture.org_admin, 4, 2), (&fixture.member, 2, 1)] {
        let (status, rankings) = get(&fixture, "/api/dashboard/clients?per_page=1", jwt).await;
        assert_eq!(status, StatusCode::OK, "{rankings}");
        assert_eq!(rankings["total_count"], 1);
        assert_eq!(rankings["rankings"][0]["request_count"], count);
        assert_eq!(rankings["rankings"][0]["api_key_count"], keys);
        let (_, detail) = get(&fixture, "/api/dashboard/clients/192.0.2.10/detail", jwt).await;
        assert_eq!(detail["total_requests"], count);
        assert!(!detail.to_string().contains("beta-secret"));
        let (_, key_rows) = get(&fixture, "/api/dashboard/clients/192.0.2.10/api-keys", jwt).await;
        assert_eq!(key_rows.as_array().unwrap().len(), keys);
        assert!(!key_rows
            .to_string()
            .contains(&fixture.foreign_key.to_string()));
        let (_, foreign) = get(&fixture, "/api/dashboard/clients/192.0.2.99/detail", jwt).await;
        assert_eq!(foreign["total_requests"], 0);
        let (_, models) = get(&fixture, "/api/dashboard/clients/models", jwt).await;
        assert!(!models.to_string().contains("beta-secret"));
        assert!(!models.to_string().contains("legacy-secret"));
        assert_eq!(
            models
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row["request_count"].as_u64().unwrap())
                .sum::<u64>(),
            count as u64
        );
        let (_, heatmap) = get(&fixture, "/api/dashboard/clients/heatmap", jwt).await;
        assert_eq!(
            heatmap
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row["count"].as_u64().unwrap())
                .sum::<u64>(),
            count as u64
        );
        let (_, timeline) = get(&fixture, "/api/dashboard/clients/timeline", jwt).await;
        assert_eq!(
            timeline
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row["unique_ips"].as_u64().unwrap())
                .sum::<u64>(),
            1
        );
    }
}

#[tokio::test]
async fn read_boundaries_endpoint_model_counters_are_scoped_and_keys_cannot_read_counters() {
    let fixture = fixture().await;
    for (jwt, count) in [(&fixture.org_admin, 4), (&fixture.member, 2)] {
        let (status, today) = get(
            &fixture,
            &format!("/api/endpoints/{}/today-stats", fixture.endpoint),
            jwt,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{today}");
        assert_eq!(today["total_requests"], count);
        for path in [
            format!("/api/endpoints/{}/model-stats", fixture.endpoint),
            "/api/dashboard/model-stats".into(),
        ] {
            let (status, models) = get(&fixture, &path, jwt).await;
            assert_eq!(status, StatusCode::OK, "{models}");
            assert!(!models.to_string().contains("beta-secret"));
            assert_eq!(
                models
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|row| row["total_requests"].as_u64().unwrap())
                    .sum::<u64>(),
                count as u64
            );
        }
    }
    for suffix in ["today-stats", "daily-stats", "model-stats", "model-tps"] {
        let status = get(
            &fixture,
            &format!("/api/endpoints/{}/{suffix}", fixture.endpoint),
            "sk_debug",
        )
        .await
        .0;
        assert!(
            matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN),
            "{suffix}: {status}"
        );
    }
    let (status, inventory) = get(&fixture, "/api/endpoints", "sk_debug").await;
    assert_eq!(status, StatusCode::OK, "{inventory}");
    assert!(!inventory.to_string().contains("\"total_requests\":7"));
}

#[tokio::test]
async fn read_boundaries_unassigned_and_membership_changes_cannot_reveal_previous_records() {
    let fixture = fixture().await;
    for path in [
        "/api/dashboard/stats",
        "/api/dashboard/overview",
        "/api/dashboard/request-responses",
        "/api/dashboard/stats/tokens",
        "/api/dashboard/clients",
    ] {
        assert_eq!(
            get(&fixture, path, &fixture.unassigned).await.0,
            StatusCode::FORBIDDEN,
            "{path}"
        );
    }
    organizations::set_membership(
        &fixture.pool,
        fixture.member_id,
        None,
        OrganizationRole::Member,
    )
    .await
    .unwrap();
    assert_eq!(
        get(
            &fixture,
            "/api/dashboard/request-responses",
            &fixture.member
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert!(organizations::membership(&fixture.pool, fixture.member_id)
        .await
        .unwrap()
        .unwrap()
        .organization_id
        .is_none());
    assert!(organizations::find(&fixture.pool, fixture.alpha)
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn read_boundaries_websocket_does_not_trust_stale_system_admin_claim() {
    let fixture = fixture().await;
    let forged_admin = token(fixture.member_id, UserRole::Admin);
    let server = crate::support::http::spawn_lb_guarded(fixture.app.clone()).await;
    let response = reqwest::Client::new()
        .get(format!("http://{}/ws/dashboard", server.addr()))
        .header("connection", "Upgrade")
        .header("upgrade", "websocket")
        .header("sec-websocket-version", "13")
        .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
        .header("authorization", format!("Bearer {forged_admin}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 403);
    drop(response);
    server.stop().await;
}

#[tokio::test]
async fn read_boundaries_hardware_metrics_omit_global_usage_observations() {
    let fixture = fixture().await;
    fixture
        .manager
        .record_metrics(llmlb::balancer::MetricsUpdate {
            endpoint_id: fixture.endpoint,
            cpu_usage: 12.0,
            memory_usage: 34.0,
            gpu_usage: None,
            gpu_memory_usage: None,
            gpu_memory_total_mb: None,
            gpu_memory_used_mb: None,
            gpu_temperature: None,
            gpu_model_name: None,
            gpu_compute_capability: None,
            gpu_capability_score: None,
            active_requests: 99,
            average_response_time_ms: Some(999.0),
            initializing: false,
            ready_models: None,
        })
        .await
        .unwrap();
    let path = format!("/api/dashboard/metrics/{}", fixture.endpoint);
    let (status, global) = get(&fixture, &path, &fixture.global).await;
    assert_eq!(status, StatusCode::OK, "{global}");
    assert_eq!(global[0]["active_requests"], 99);
    for jwt in [&fixture.org_admin, &fixture.member] {
        let (status, metrics) = get(&fixture, &path, jwt).await;
        assert_eq!(status, StatusCode::OK, "{metrics}");
        assert_eq!(metrics[0]["cpu_usage"], 12.0);
        assert!(metrics[0].get("active_requests").is_none());
        assert!(metrics[0].get("total_requests").is_none());
        assert!(metrics[0].get("average_response_time_ms").is_none());
    }
}

#[tokio::test]
async fn read_boundaries_large_provider_counts_do_not_wrap_raw_history_dto() {
    let fixture = fixture().await;
    let storage = RequestHistoryStorage::new(fixture.pool.clone());
    let attribution = UsageAttribution {
        request_id: Uuid::new_v4(),
        account_id: Some(fixture.member_id),
        organization_id: Some(fixture.alpha),
        received_at: chrono::Utc::now(),
        api_key_id: None,
        auth_source: "jwt".into(),
    };
    let record = RequestResponseRecord::new(
        fixture.endpoint,
        "shared".into(),
        "127.0.0.1".parse().unwrap(),
        "large".into(),
        RequestType::Chat,
        json!({"model":"large"}),
        StatusCode::OK,
        std::time::Duration::from_millis(1),
        None,
        None,
    );
    let large = u64::from(u32::MAX) + 42;
    storage.accept_usage(&attribution, &record).await.unwrap();
    storage
        .finish_usage(
            &attribution,
            &record,
            &UsageTokens::provider(Some(large), Some(1), Some(large + 1)),
        )
        .await
        .unwrap();
    let (status, raw) = get(
        &fixture,
        &format!(
            "/api/dashboard/request-responses/{}",
            attribution.request_id
        ),
        &fixture.global,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{raw}");
    assert!(raw["input_tokens"].is_null(), "{raw}");
    assert!(raw["total_tokens"].is_null(), "{raw}");
    assert_eq!(raw["output_tokens"], 1);
    let (_, totals) = get(&fixture, "/api/dashboard/stats/tokens", &fixture.global).await;
    assert_eq!(totals["total_tokens"], 620 + large + 1);
}

#[tokio::test]
async fn read_boundaries_global_logs_and_benchmark_results_require_system_admin() {
    let fixture = fixture().await;
    let benchmark = format!("/api/benchmarks/tps/{}", Uuid::new_v4());
    for jwt in [&fixture.org_admin, &fixture.member] {
        for path in ["/api/dashboard/logs/lb", benchmark.as_str()] {
            assert_eq!(
                get(&fixture, path, jwt).await.0,
                StatusCode::FORBIDDEN,
                "{path}"
            );
        }
    }
    assert_eq!(
        get(&fixture, &benchmark, &fixture.global).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn read_boundaries_legacy_raw_is_global_history_without_inflating_new_usage_counters() {
    let fixture = fixture().await;
    let old = RequestResponseRecord::new(
        fixture.endpoint,
        "shared".into(),
        "127.0.0.1".parse().unwrap(),
        "before-attribution".into(),
        RequestType::Chat,
        json!({"model":"before-attribution"}),
        StatusCode::OK,
        std::time::Duration::from_millis(100),
        None,
        None,
    );
    RequestHistoryStorage::new(fixture.pool.clone())
        .save_record(&old)
        .await
        .unwrap();
    let (_, history) = get(
        &fixture,
        "/api/dashboard/request-responses",
        &fixture.global,
    )
    .await;
    assert_eq!(history["total_count"], 8);
    let (_, stats) = get(&fixture, "/api/dashboard/stats", &fixture.global).await;
    assert_eq!(stats["total_requests"], 7);
    assert_eq!(stats["successful_requests"], 7);
    let (_, today) = get(
        &fixture,
        &format!("/api/endpoints/{}/today-stats", fixture.endpoint),
        &fixture.global,
    )
    .await;
    assert_eq!(today["total_requests"], 7);
    let (_, models) = get(&fixture, "/api/dashboard/model-stats", &fixture.global).await;
    assert!(!models.to_string().contains("before-attribution"));

    let pending = UsageAttribution {
        request_id: Uuid::new_v4(),
        account_id: Some(fixture.member_id),
        organization_id: Some(fixture.alpha),
        received_at: chrono::Utc::now(),
        api_key_id: None,
        auth_source: "jwt".into(),
    };
    let record = RequestResponseRecord::new(
        fixture.endpoint,
        "shared".into(),
        "127.0.0.1".parse().unwrap(),
        "pending-only".into(),
        RequestType::Chat,
        json!({"model":"pending-only"}),
        StatusCode::OK,
        std::time::Duration::ZERO,
        None,
        None,
    );
    RequestHistoryStorage::new(fixture.pool.clone())
        .accept_usage(&pending, &record)
        .await
        .unwrap();
    let (_, stats) = get(&fixture, "/api/dashboard/stats", &fixture.global).await;
    assert_eq!(stats["total_requests"], 8);
    assert_eq!(stats["successful_requests"], 7);
    assert_eq!(stats["failed_requests"], 0);
    let (_, today) = get(
        &fixture,
        &format!("/api/endpoints/{}/today-stats", fixture.endpoint),
        &fixture.global,
    )
    .await;
    assert_eq!(today["total_requests"], 8);
    assert_eq!(today["successful_requests"], 7);
    assert_eq!(today["failed_requests"], 0);
}
