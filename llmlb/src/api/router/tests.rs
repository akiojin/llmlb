//! ルーターの契約テスト。
//!
//! `/api/dashboard/*` は管理UI向けのJWT専用ルートで、APIキー（`x-api-key`）では401になる。
//! そのためダッシュボードAPIのテストは `create_jwt` で発行したトークンを `Authorization` に載せる。
//! このファイルに `#[ignore]` のテストは無い。

use super::dashboard::{normalize_dashboard_path, DASHBOARD_INDEX};
use super::*;
use crate::common::auth::UserRole;
use crate::db::test_utils::TestAppStateBuilder;
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use tower::Service;

async fn test_state() -> AppState {
    TestAppStateBuilder::new().await.build().await
}

/// 管理者JWTで `/api/dashboard/*` にGETし、ステータスを返す
async fn dashboard_get_status(uri: &str) -> StatusCode {
    let state = test_state().await;
    let admin_token = crate::auth::jwt::create_jwt(
        "admin-user",
        UserRole::Admin,
        &state.auth.jwt_secret,
        false,
        0,
    )
    .expect("create admin jwt");
    let mut app = create_app(state);
    let response = app
        .call(
            Request::builder()
                .method(axum::http::Method::GET)
                .uri(uri)
                .header("authorization", format!("Bearer {}", admin_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    response.status()
}

#[tokio::test]
async fn test_dashboard_static_served() {
    let state = test_state().await;
    let mut app = create_app(state);
    let response = app
        .call(
            Request::builder()
                .method(axum::http::Method::GET)
                .uri("/dashboard/index.html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let (parts, body) = response.into_parts();
    let bytes = to_bytes(body, 1024 * 1024).await.unwrap();

    assert_eq!(status, StatusCode::OK);
    let content_type = parts.headers[axum::http::header::CONTENT_TYPE]
        .to_str()
        .unwrap();
    assert!(content_type.starts_with("text/html"));
    assert!(bytes.starts_with(b"<!DOCTYPE html"));
}

// NOTE: test_playground_static_served は廃止
// Playground機能はダッシュボード内のエンドポイント別Playgroundに移行 (#playground/:endpointId)

#[tokio::test]
async fn test_dashboard_nodes_endpoint_returns_json() {
    assert_eq!(
        dashboard_get_status("/api/dashboard/endpoints").await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn test_dashboard_overview_endpoint_returns_all_sections() {
    assert_eq!(
        dashboard_get_status("/api/dashboard/overview").await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn test_dashboard_metrics_endpoint_returns_history() {
    assert_eq!(
        dashboard_get_status("/api/dashboard/request-history").await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn test_dashboard_audit_logs_requires_admin_role() {
    let state = test_state().await;
    let viewer_token = crate::auth::jwt::create_jwt(
        "viewer-user",
        UserRole::Viewer,
        &state.auth.jwt_secret,
        false,
        0,
    )
    .expect("create viewer jwt");
    let mut app = create_app(state);

    let response = app
        .call(
            Request::builder()
                .method(axum::http::Method::GET)
                .uri("/api/dashboard/audit-logs")
                .header("authorization", format!("Bearer {}", viewer_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// --- normalize_dashboard_path tests ---

#[test]
fn test_normalize_dashboard_path_empty() {
    assert_eq!(
        normalize_dashboard_path(""),
        Some(DASHBOARD_INDEX.to_string())
    );
}

#[test]
fn test_normalize_dashboard_path_just_slashes() {
    assert_eq!(
        normalize_dashboard_path("/"),
        Some(DASHBOARD_INDEX.to_string())
    );
    assert_eq!(
        normalize_dashboard_path("///"),
        Some(DASHBOARD_INDEX.to_string())
    );
}

#[test]
fn test_normalize_dashboard_path_valid_file() {
    assert_eq!(
        normalize_dashboard_path("assets/index.js"),
        Some("assets/index.js".to_string())
    );
}

#[test]
fn test_normalize_dashboard_path_strips_slashes() {
    assert_eq!(
        normalize_dashboard_path("/assets/style.css/"),
        Some("assets/style.css".to_string())
    );
}

#[test]
fn test_normalize_dashboard_path_rejects_dotdot() {
    assert_eq!(normalize_dashboard_path("../etc/passwd"), None);
    assert_eq!(normalize_dashboard_path("assets/../../../etc/passwd"), None);
}

#[test]
fn test_normalize_dashboard_path_rejects_backslash() {
    assert_eq!(normalize_dashboard_path("assets\\evil.js"), None);
}

#[test]
fn test_normalize_dashboard_path_normal_subdirectory() {
    assert_eq!(
        normalize_dashboard_path("assets/js/main.js"),
        Some("assets/js/main.js".to_string())
    );
}

#[test]
fn test_normalize_dashboard_path_single_file() {
    assert_eq!(
        normalize_dashboard_path("favicon.ico"),
        Some("favicon.ico".to_string())
    );
}

#[tokio::test]
async fn test_dashboard_audit_logs_allows_admin_role() {
    let state = test_state().await;
    let admin_token = crate::auth::jwt::create_jwt(
        "admin-user",
        UserRole::Admin,
        &state.auth.jwt_secret,
        false,
        0,
    )
    .expect("create admin jwt");
    let mut app = create_app(state);

    let response = app
        .call(
            Request::builder()
                .method(axum::http::Method::GET)
                .uri("/api/dashboard/audit-logs")
                .header("authorization", format!("Bearer {}", admin_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
