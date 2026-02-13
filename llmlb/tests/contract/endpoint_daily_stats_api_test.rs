//! エンドポイント日次統計API の Contract Tests
//!
//! SPEC-76643000 Phase 6 T020: GET /api/endpoints/:id/daily-stats
//! 期間指定で日次集計データの配列を返すことを確認。

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use llmlb::common::auth::UserRole;
use llmlb::db::endpoint_daily_stats;
use serde_json::Value;
use serial_test::serial;
use sqlx::SqlitePool;
use tower::ServiceExt;
use uuid::Uuid;

async fn build_app() -> (Router, SqlitePool, String) {
    let (app, db_pool) = crate::support::lb::create_test_lb().await;

    let password_hash = llmlb::auth::password::hash_password("password123").unwrap();
    let admin_user = llmlb::db::users::create(&db_pool, "admin", &password_hash, UserRole::Admin)
        .await
        .expect("create admin user");
    let jwt = llmlb::auth::jwt::create_jwt(
        &admin_user.id.to_string(),
        UserRole::Admin,
        &crate::support::lb::test_jwt_secret(),
    )
    .expect("create admin jwt");

    (app, db_pool, jwt)
}

fn admin_request(jwt: &str) -> axum::http::request::Builder {
    Request::builder().header("authorization", format!("Bearer {}", jwt))
}

/// T020: 日次統計API - データなしの場合は空配列を返す
#[tokio::test]
#[serial]
async fn test_endpoint_daily_stats_empty() {
    let (app, _db_pool, jwt) = build_app().await;
    let endpoint_id = Uuid::new_v4();

    let response = app
        .oneshot(
            admin_request(&jwt)
                .method("GET")
                .uri(format!("/api/endpoints/{}/daily-stats?days=7", endpoint_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();

    assert!(body.is_array());
    assert_eq!(body.as_array().unwrap().len(), 0);
}

/// T020: 日次統計API - データがある場合は日付昇順で返す
#[tokio::test]
#[serial]
async fn test_endpoint_daily_stats_with_data() {
    let (app, db_pool, jwt) = build_app().await;
    let endpoint_id = Uuid::new_v4();

    // 最近の日付を使用（days=365のクランプ内に収まるように）
    let today = chrono::Local::now();
    let yesterday = (today - chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    let day_before = (today - chrono::Duration::days(2))
        .format("%Y-%m-%d")
        .to_string();

    // テストデータを挿入
    endpoint_daily_stats::upsert_daily_stats(&db_pool, endpoint_id, "llama3:8b", &day_before, true)
        .await
        .unwrap();
    endpoint_daily_stats::upsert_daily_stats(&db_pool, endpoint_id, "llama3:8b", &day_before, true)
        .await
        .unwrap();
    endpoint_daily_stats::upsert_daily_stats(&db_pool, endpoint_id, "gpt-4", &day_before, false)
        .await
        .unwrap();
    endpoint_daily_stats::upsert_daily_stats(&db_pool, endpoint_id, "llama3:8b", &yesterday, true)
        .await
        .unwrap();

    let response = app
        .oneshot(
            admin_request(&jwt)
                .method("GET")
                .uri(format!("/api/endpoints/{}/daily-stats?days=7", endpoint_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();

    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 2);

    // day_before: 3件 (成功2, 失敗1)
    assert_eq!(arr[0]["date"], day_before);
    assert_eq!(arr[0]["total_requests"], 3);
    assert_eq!(arr[0]["successful_requests"], 2);
    assert_eq!(arr[0]["failed_requests"], 1);

    // yesterday: 1件 (成功1, 失敗0)
    assert_eq!(arr[1]["date"], yesterday);
    assert_eq!(arr[1]["total_requests"], 1);
    assert_eq!(arr[1]["successful_requests"], 1);
    assert_eq!(arr[1]["failed_requests"], 0);
}

/// T020: 日次統計API - daysパラメータのデフォルト値（7日）
#[tokio::test]
#[serial]
async fn test_endpoint_daily_stats_default_days() {
    let (app, _db_pool, jwt) = build_app().await;
    let endpoint_id = Uuid::new_v4();

    let response = app
        .oneshot(
            admin_request(&jwt)
                .method("GET")
                .uri(format!("/api/endpoints/{}/daily-stats", endpoint_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // daysパラメータなしでも正常にレスポンスを返す
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert!(body.is_array());
}

/// T020: 日次統計API - daysが365を超える場合は365にクランプ
#[tokio::test]
#[serial]
async fn test_endpoint_daily_stats_max_days_clamp() {
    let (app, _db_pool, jwt) = build_app().await;
    let endpoint_id = Uuid::new_v4();

    let response = app
        .oneshot(
            admin_request(&jwt)
                .method("GET")
                .uri(format!(
                    "/api/endpoints/{}/daily-stats?days=999",
                    endpoint_id
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // 大きな値でもエラーにならず正常レスポンスを返す
    assert_eq!(response.status(), StatusCode::OK);
}
