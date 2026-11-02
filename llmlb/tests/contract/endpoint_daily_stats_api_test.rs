//! エンドポイント日次統計API の Contract Tests
//!
//! SPEC-8c32349f Phase 6 T020: GET /api/endpoints/:id/daily-stats
//! 期間指定で日次集計データの配列を返すことを確認。

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use llmlb::common::auth::UserRole;
use llmlb::common::protocol::{RequestResponseRecord, RequestType};
use llmlb::common::usage::{UsageAttribution, UsageTokens};
use llmlb::db::request_history::RequestHistoryStorage;
use serde_json::Value;
use serial_test::serial;
use sqlx::SqlitePool;
use tower::ServiceExt;
use uuid::Uuid;

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

fn admin_request(jwt: &str) -> axum::http::request::Builder {
    Request::builder().header("authorization", format!("Bearer {}", jwt))
}

async fn create_endpoint(
    db_pool: &SqlitePool,
    name: &str,
    status: llmlb::types::endpoint::EndpointStatus,
) -> Uuid {
    let unique_url = format!("http://127.0.0.1/{}", Uuid::new_v4());
    let mut endpoint = llmlb::types::endpoint::Endpoint::new(
        name.to_string(),
        unique_url,
        llmlb::types::endpoint::EndpointType::OpenaiCompatible,
    );
    endpoint.status = status;
    let endpoint_id = endpoint.id;
    llmlb::db::endpoints::create_endpoint(db_pool, &endpoint)
        .await
        .expect("create endpoint");
    endpoint_id
}

async fn record_request(db_pool: &SqlitePool, endpoint_id: Uuid, model_id: &str, success: bool) {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    record_request_on(db_pool, endpoint_id, model_id, &today, success)
        .await
        .expect("persist accepted request");
}

async fn record_request_on(
    db_pool: &SqlitePool,
    endpoint_id: Uuid,
    model_id: &str,
    day: &str,
    success: bool,
) -> Result<(), llmlb::common::error::LbError> {
    let attribution = UsageAttribution {
        request_id: Uuid::new_v4(),
        account_id: None,
        organization_id: None,
        received_at: format!("{day}T12:00:00Z").parse().unwrap(),
        api_key_id: None,
        auth_source: "api_key".into(),
    };
    let record = RequestResponseRecord::new(
        endpoint_id,
        "fixture endpoint".into(),
        "127.0.0.1".parse().unwrap(),
        model_id.into(),
        RequestType::Chat,
        serde_json::json!({"model": model_id}),
        if success {
            StatusCode::OK
        } else {
            StatusCode::BAD_GATEWAY
        },
        std::time::Duration::ZERO,
        None,
        None,
    );
    let storage = RequestHistoryStorage::new(db_pool.clone());
    storage.accept_usage(&attribution, &record).await?;
    storage
        .finish_usage(&attribution, &record, &UsageTokens::missing())
        .await
}

async fn add_endpoint_model(db_pool: &SqlitePool, endpoint_id: Uuid, model_id: &str) {
    let model = llmlb::types::endpoint::EndpointModel {
        endpoint_id,
        model_id: model_id.to_string(),
        capabilities: None,
        max_tokens: None,
        last_checked: None,
        supported_apis: vec![llmlb::types::endpoint::SupportedAPI::ChatCompletions],
        canonical_name: None,
    };
    llmlb::db::endpoints::add_endpoint_model(db_pool, &model)
        .await
        .expect("add endpoint model");
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
    let today = chrono::Utc::now();
    let yesterday = (today - chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    let day_before = (today - chrono::Duration::days(2))
        .format("%Y-%m-%d")
        .to_string();

    // テストデータを挿入
    record_request_on(&db_pool, endpoint_id, "llama3:8b", &day_before, true)
        .await
        .unwrap();
    record_request_on(&db_pool, endpoint_id, "llama3:8b", &day_before, true)
        .await
        .unwrap();
    record_request_on(&db_pool, endpoint_id, "gpt-4", &day_before, false)
        .await
        .unwrap();
    record_request_on(&db_pool, endpoint_id, "llama3:8b", &yesterday, true)
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

#[tokio::test]
#[serial]
async fn test_dashboard_usage_retains_deleted_endpoint_requests_within_raw_retention() {
    let (app, db_pool, jwt) = build_app().await;

    let active_endpoint = create_endpoint(
        &db_pool,
        "active-endpoint",
        llmlb::types::endpoint::EndpointStatus::Online,
    )
    .await;
    let deleted_endpoint = create_endpoint(
        &db_pool,
        "deleted-endpoint",
        llmlb::types::endpoint::EndpointStatus::Offline,
    )
    .await;

    record_request(&db_pool, active_endpoint, "kept-model", true).await;
    record_request(&db_pool, active_endpoint, "kept-model", true).await;
    record_request(&db_pool, deleted_endpoint, "deleted-model", false).await;

    llmlb::db::endpoints::delete_endpoint(&db_pool, deleted_endpoint)
        .await
        .expect("delete endpoint");

    let stats_response = app
        .clone()
        .oneshot(
            admin_request(&jwt)
                .method("GET")
                .uri("/api/dashboard/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stats_response.status(), StatusCode::OK);
    let stats_body = to_bytes(stats_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let stats_json: Value = serde_json::from_slice(&stats_body).unwrap();
    assert_eq!(stats_json["total_requests"], 3);

    let model_stats_response = app
        .oneshot(
            admin_request(&jwt)
                .method("GET")
                .uri("/api/dashboard/model-stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(model_stats_response.status(), StatusCode::OK);
    let model_stats_body = to_bytes(model_stats_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let model_stats_json: Value = serde_json::from_slice(&model_stats_body).unwrap();
    let entries = model_stats_json
        .as_array()
        .expect("model-stats should be array");

    let total_from_models: i64 = entries
        .iter()
        .map(|entry| {
            entry["total_requests"]
                .as_i64()
                .expect("model total_requests should be integer")
        })
        .sum();

    // Inventory deletion cannot erase accepted usage. Model breakdown remains
    // available while its scoped raw history is retained; the durable total
    // does not depend on that raw retention window.
    assert_eq!(total_from_models, 3);
    let deleted_model = entries
        .iter()
        .find(|entry| entry["model_id"] == "deleted-model")
        .expect("deleted endpoint's accepted model usage is retained");
    assert_eq!(deleted_model["total_requests"], 1);
    assert_eq!(deleted_model["successful_requests"], 0);
    assert_eq!(deleted_model["failed_requests"], 1);
}

#[tokio::test]
#[serial]
async fn test_dashboard_models_includes_non_online_endpoint_models() {
    let (app, db_pool, jwt) = build_app().await;

    let pending_endpoint = create_endpoint(
        &db_pool,
        "pending-endpoint",
        llmlb::types::endpoint::EndpointStatus::Pending,
    )
    .await;
    add_endpoint_model(&db_pool, pending_endpoint, "pending-only-model").await;

    let response = app
        .oneshot(
            admin_request(&jwt)
                .method("GET")
                .uri("/api/dashboard/models")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: Value = serde_json::from_slice(&body).unwrap();
    let models = body_json["data"]
        .as_array()
        .expect("dashboard/models data should be array");

    let model = models
        .iter()
        .find(|entry| entry["id"] == Value::String("pending-only-model".to_string()))
        .expect("pending-only-model should be listed on dashboard/models");

    assert_eq!(model["ready"], Value::Bool(false));
    assert!(
        model["endpoint_ids"]
            .as_array()
            .expect("endpoint_ids should be an array")
            .iter()
            .any(|id| id == &Value::String(pending_endpoint.to_string())),
        "pending endpoint id should be included"
    );
}

#[tokio::test]
#[serial]
async fn test_dashboard_models_preserves_image_input_supported_api() {
    let (app, db_pool, jwt) = build_app().await;

    let endpoint_id = create_endpoint(
        &db_pool,
        "vision-endpoint",
        llmlb::types::endpoint::EndpointStatus::Online,
    )
    .await;
    let model = llmlb::types::endpoint::EndpointModel {
        endpoint_id,
        model_id: "qwen/qwen3-vl-30b".to_string(),
        capabilities: Some(vec!["chat".to_string(), "image_input".to_string()]),
        max_tokens: None,
        last_checked: None,
        supported_apis: vec![
            llmlb::types::endpoint::SupportedAPI::ChatCompletions,
            llmlb::types::endpoint::SupportedAPI::ImageInput,
        ],
        canonical_name: None,
    };
    llmlb::db::endpoints::add_endpoint_model(&db_pool, &model)
        .await
        .expect("add vision endpoint model");

    let response = app
        .oneshot(
            admin_request(&jwt)
                .method("GET")
                .uri("/api/dashboard/models")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: Value = serde_json::from_slice(&body).unwrap();
    let models = body_json["data"]
        .as_array()
        .expect("dashboard/models data should be array");
    let model = models
        .iter()
        .find(|entry| entry["id"] == Value::String("qwen/qwen3-vl-30b".to_string()))
        .expect("vision model should be listed on dashboard/models");
    let supported_apis = model["supported_apis"]
        .as_array()
        .expect("supported_apis should be an array");

    assert!(
        supported_apis
            .iter()
            .any(|api| api == &Value::String("image_input".to_string())),
        "image_input should be preserved for Dashboard APIs badges"
    );
}
