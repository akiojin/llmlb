//! Integration Test: T270 - GET /api/system 認証不要テスト
//!
//! SPEC-a6e55b37 FR-006: ダッシュボードヘッダーに現在実行中バージョンを
//! 更新有無に関係なく常時表示する。
//!
//! GET /api/system はバージョン情報とアップデート状態を返す読み取り専用APIであり、
//! JWT認証なしでもアクセス可能でなければならない（リリースビルドでの401回避）。

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;

use crate::support;

/// GET /api/system が認証なしで200を返すこと
#[tokio::test]
async fn get_system_returns_ok_without_auth() {
    let (app, _pool) = support::lb::create_test_lb().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/system")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "GET /api/system should not require authentication"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // version フィールドが存在すること
    assert!(
        json.get("version").is_some(),
        "response should contain 'version' field: {json}"
    );
    // update フィールドが存在すること
    assert!(
        json.get("update").is_some(),
        "response should contain 'update' field: {json}"
    );
}

/// POST /api/system/update/check は認証なしで401を返すこと（変異操作は保護される）
#[tokio::test]
async fn check_update_requires_auth() {
    let (app, _pool) = support::lb::create_test_lb().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/system/update/check")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "POST /api/system/update/check should require authentication"
    );
}

/// POST /api/system/update/apply は認証なしで401を返すこと
#[tokio::test]
async fn apply_update_requires_auth() {
    let (app, _pool) = support::lb::create_test_lb().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/system/update/apply")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "POST /api/system/update/apply should require authentication"
    );
}
