//! クラウドメトリクスエクスポートAPI の Contract Tests
//!
//! SPEC #582 US-005 (FR-034〜FR-038) / Issue #693
//! `GET /api/metrics/cloud/export?format=json|csv&days=1..90`
//! - openai / google / anthropic の UTC 日次 rollup を返す
//! - JSON / CSV の両形式
//! - days は 1..=90 のみ許可
//! - 90 日を超えるデータは保持しない（rolling retention）

use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
    Router,
};
use chrono::{Duration, Utc};
use llmlb::common::auth::UserRole;
use serde_json::{json, Value};
use serial_test::serial;
use sqlx::SqlitePool;
use tower::ServiceExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn build_app() -> (Router, SqlitePool, String) {
    let (app, db_pool) = crate::support::lb::create_test_lb().await;

    let password_hash = llmlb::auth::password::hash_password("password123").unwrap();
    let admin_user =
        llmlb::db::users::create(&db_pool, "admin", &password_hash, UserRole::Admin, false)
            .await
            .expect("create admin user");
    let jwt = llmlb::auth::jwt::create_jwt(
        &admin_user.id.to_string(),
        UserRole::Admin,
        &crate::support::lb::test_jwt_secret(),
        false,
        0,
    )
    .expect("create admin jwt");

    (app, db_pool, jwt)
}

fn utc_date(days_ago: i64) -> String {
    (Utc::now().date_naive() - Duration::days(days_ago))
        .format("%Y-%m-%d")
        .to_string()
}

async fn seed_sample(
    db_pool: &SqlitePool,
    date: &str,
    provider: &str,
    status_code: i64,
    latency_ms: i64,
) {
    sqlx::query(
        "INSERT INTO cloud_request_metrics (date, provider, status_code, latency_ms) \
         VALUES (?, ?, ?, ?)",
    )
    .bind(date)
    .bind(provider)
    .bind(status_code)
    .bind(latency_ms)
    .execute(db_pool)
    .await
    .expect("seed cloud request metric sample");
}

async fn get(app: &Router, uri: &str, jwt: Option<&str>) -> (StatusCode, Option<String>, Vec<u8>) {
    let mut builder = Request::builder().method("GET").uri(uri);
    if let Some(jwt) = jwt {
        builder = builder.header("authorization", format!("Bearer {}", jwt));
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    (status, content_type, body)
}

async fn get_json_rows(app: &Router, uri: &str, jwt: &str) -> Vec<Value> {
    let (status, content_type, body) = get(app, uri, Some(jwt)).await;
    assert_eq!(status, StatusCode::OK, "GET {uri} should succeed");
    assert!(
        content_type
            .unwrap_or_default()
            .contains("application/json"),
        "JSON export should have application/json content type"
    );
    let body: Value = serde_json::from_slice(&body).expect("JSON body");
    body.as_array().expect("JSON export is an array").clone()
}

/// 認証なしのリクエストは reject される
#[tokio::test]
#[serial]
async fn cloud_metrics_export_requires_authentication() {
    let (app, _db_pool, _jwt) = build_app().await;

    let (status, _, _) = get(&app, "/api/metrics/cloud/export?format=json&days=7", None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// admin 以外（viewer）の JWT は reject される
#[tokio::test]
#[serial]
async fn cloud_metrics_export_rejects_viewer_jwt() {
    let (app, db_pool, _jwt) = build_app().await;
    let password_hash = llmlb::auth::password::hash_password("password123").unwrap();
    let viewer =
        llmlb::db::users::create(&db_pool, "viewer", &password_hash, UserRole::Viewer, false)
            .await
            .expect("create viewer user");
    let viewer_jwt = llmlb::auth::jwt::create_jwt(
        &viewer.id.to_string(),
        UserRole::Viewer,
        &crate::support::lb::test_jwt_secret(),
        false,
        0,
    )
    .expect("create viewer jwt");

    let (status, _, _) = get(
        &app,
        "/api/metrics/cloud/export?format=json&days=7",
        Some(&viewer_jwt),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// provider/day 単位で rollup され、全フィールドが返る（p95 は nearest-rank）
#[tokio::test]
#[serial]
async fn cloud_metrics_export_json_returns_provider_day_rollup() {
    let (app, db_pool, jwt) = build_app().await;
    let today = utc_date(0);
    let yesterday = utc_date(1);

    // google/today: latency 1..=20ms, うち 2 件がエラー
    for latency in 1..=20 {
        let status = if latency <= 2 { 500 } else { 200 };
        seed_sample(&db_pool, &today, "google", status, latency).await;
    }
    seed_sample(&db_pool, &today, "anthropic", 200, 300).await;
    seed_sample(&db_pool, &yesterday, "openai", 429, 40).await;
    seed_sample(&db_pool, &yesterday, "openai", 200, 60).await;

    let rows = get_json_rows(&app, "/api/metrics/cloud/export?format=json&days=30", &jwt).await;

    assert_eq!(
        rows,
        vec![
            json!({
                "date": yesterday,
                "provider": "openai",
                "request_count": 2,
                "success_count": 1,
                "error_count": 1,
                "avg_latency_ms": 50.0,
                "p95_latency_ms": 60,
            }),
            json!({
                "date": today,
                "provider": "anthropic",
                "request_count": 1,
                "success_count": 1,
                "error_count": 0,
                "avg_latency_ms": 300.0,
                "p95_latency_ms": 300,
            }),
            json!({
                "date": today,
                "provider": "google",
                "request_count": 20,
                "success_count": 18,
                "error_count": 2,
                "avg_latency_ms": 10.5,
                "p95_latency_ms": 19,
            }),
        ]
    );
}

/// format 省略時は JSON を返す
#[tokio::test]
#[serial]
async fn cloud_metrics_export_defaults_to_json() {
    let (app, db_pool, jwt) = build_app().await;
    seed_sample(&db_pool, &utc_date(0), "openai", 200, 10).await;

    let rows = get_json_rows(&app, "/api/metrics/cloud/export?days=7", &jwt).await;

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["provider"], "openai");
}

/// CSV はヘッダー付きで export される
#[tokio::test]
#[serial]
async fn cloud_metrics_export_csv_has_header_and_rows() {
    let (app, db_pool, jwt) = build_app().await;
    let today = utc_date(0);
    seed_sample(&db_pool, &today, "openai", 200, 100).await;
    seed_sample(&db_pool, &today, "openai", 502, 201).await;

    let (status, content_type, body) = get(
        &app,
        "/api/metrics/cloud/export?format=csv&days=7",
        Some(&jwt),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(content_type.unwrap_or_default().contains("text/csv"));
    let csv = String::from_utf8(body).expect("utf-8 csv");
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(
        lines,
        vec![
            "date,provider,request_count,success_count,error_count,avg_latency_ms,p95_latency_ms",
            &format!("{today},openai,2,1,1,150.5,201"),
        ]
    );
}

/// データが無い場合も CSV ヘッダーは返る
#[tokio::test]
#[serial]
async fn cloud_metrics_export_csv_empty_returns_header_only() {
    let (app, _db_pool, jwt) = build_app().await;

    let (status, _, body) = get(
        &app,
        "/api/metrics/cloud/export?format=csv&days=1",
        Some(&jwt),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        String::from_utf8(body).unwrap().trim_end(),
        "date,provider,request_count,success_count,error_count,avg_latency_ms,p95_latency_ms"
    );
}

/// days / format の validation
#[tokio::test]
#[serial]
async fn cloud_metrics_export_rejects_invalid_parameters() {
    let (app, _db_pool, jwt) = build_app().await;

    for uri in [
        "/api/metrics/cloud/export?days=91",
        "/api/metrics/cloud/export?format=json&days=0",
        "/api/metrics/cloud/export?format=csv&days=-1",
        "/api/metrics/cloud/export?format=json&days=abc",
        "/api/metrics/cloud/export?format=xml&days=7",
    ] {
        let (status, _, _) = get(&app, uri, Some(&jwt)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri} should be rejected");
    }

    for uri in [
        "/api/metrics/cloud/export?format=json&days=1",
        "/api/metrics/cloud/export?format=csv&days=90",
        "/api/metrics/cloud/export",
    ] {
        let (status, _, _) = get(&app, uri, Some(&jwt)).await;
        assert_eq!(status, StatusCode::OK, "{uri} should be accepted");
    }
}

/// days で指定した UTC 日数（当日を含む）の範囲だけが返る
#[tokio::test]
#[serial]
async fn cloud_metrics_export_limits_rows_to_requested_days() {
    let (app, db_pool, jwt) = build_app().await;
    seed_sample(&db_pool, &utc_date(0), "openai", 200, 10).await;
    seed_sample(&db_pool, &utc_date(6), "openai", 200, 10).await;
    seed_sample(&db_pool, &utc_date(7), "openai", 200, 10).await;

    let dates = |rows: Vec<Value>| -> Vec<String> {
        rows.iter()
            .map(|r| r["date"].as_str().unwrap().to_string())
            .collect()
    };

    let one_day = get_json_rows(&app, "/api/metrics/cloud/export?days=1", &jwt).await;
    assert_eq!(dates(one_day), vec![utc_date(0)]);

    let seven_days = get_json_rows(&app, "/api/metrics/cloud/export?days=7", &jwt).await;
    assert_eq!(dates(seven_days), vec![utc_date(6), utc_date(0)]);
}

/// 90 日を超えるデータは保持しない（rolling 90 days retention）
#[tokio::test]
#[serial]
async fn cloud_metrics_export_prunes_data_older_than_90_days() {
    let (app, db_pool, jwt) = build_app().await;
    seed_sample(&db_pool, &utc_date(89), "google", 200, 10).await;
    seed_sample(&db_pool, &utc_date(90), "google", 200, 10).await;
    seed_sample(&db_pool, &utc_date(120), "google", 200, 10).await;

    let rows = get_json_rows(&app, "/api/metrics/cloud/export?days=90", &jwt).await;
    let dates: Vec<&str> = rows.iter().map(|r| r["date"].as_str().unwrap()).collect();
    assert_eq!(dates, vec![utc_date(89).as_str()]);

    let remaining: Vec<String> =
        sqlx::query_scalar("SELECT date FROM cloud_request_metrics ORDER BY date")
            .fetch_all(&db_pool)
            .await
            .unwrap();
    assert_eq!(
        remaining,
        vec![utc_date(89)],
        "samples older than the 90-day window must be deleted"
    );
}

/// クラウドプロキシ経由のリクエストが日次 rollup に記録される
#[tokio::test]
#[serial]
async fn cloud_proxy_requests_are_recorded_in_daily_export() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl_test",
            "object": "chat.completion",
            "created": 0,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": { "role": "assistant", "content": "ok" },
                "finish_reason": "stop"
            }]
        })))
        .mount(&mock_server)
        .await;
    std::env::set_var("OPENAI_API_KEY", "sk-test");
    std::env::set_var("OPENAI_BASE_URL", mock_server.uri());

    let (app, _db_pool, jwt) = build_app().await;

    let response = app
        .clone()
        .oneshot(crate::support::lb::with_connect_info(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("x-api-key", "sk_debug")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "model": "openai:gpt-4o",
                        "stream": false,
                        "messages": [{ "role": "user", "content": "hello" }]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let _ = to_bytes(response.into_body(), usize::MAX).await.unwrap();

    std::env::remove_var("OPENAI_BASE_URL");
    std::env::remove_var("OPENAI_API_KEY");

    let mut rows = Vec::new();
    for _ in 0..50 {
        rows = get_json_rows(&app, "/api/metrics/cloud/export?days=1", &jwt).await;
        if !rows.is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    assert_eq!(rows.len(), 1, "one provider/day row expected: {rows:?}");
    assert_eq!(rows[0]["date"], utc_date(0));
    assert_eq!(rows[0]["provider"], "openai");
    assert_eq!(rows[0]["request_count"], 1);
    assert_eq!(rows[0]["success_count"], 1);
    assert_eq!(rows[0]["error_count"], 0);
}

/// 既存 Prometheus path は text export のまま維持される
#[tokio::test]
#[serial]
async fn cloud_metrics_prometheus_path_is_unchanged() {
    let (app, _db_pool, jwt) = build_app().await;
    llmlb::cloud_metrics::record("openai", 200, 10);

    let (status, content_type, body) = get(&app, "/api/metrics/cloud", Some(&jwt)).await;

    assert_eq!(status, StatusCode::OK);
    assert!(content_type.unwrap_or_default().contains("text/plain"));
    assert!(String::from_utf8(body)
        .unwrap()
        .contains("cloud_requests_total"));
}
