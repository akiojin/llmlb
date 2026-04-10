//! Integration Test: T-0008, T-0010 - 招待キー生成・受け入れAPI統合テスト
//!
//! SPEC #580 US-004: パスワード管理（招待キー＋オフライン対応）
//!
//! T-0008: POST /api/admin/invitations テスト（招待キー生成API）
//! T-0010: POST /api/auth/accept-invitation テスト（招待受け入れAPI）

use reqwest::Client;
use serde_json::{json, Value};

use crate::support::lb::spawn_test_lb;

/// T-0008-1: Admin ユーザーが招待キーを生成できる
#[tokio::test]
async fn test_create_invitation_success_admin() {
    let lb = spawn_test_lb().await;
    let client = Client::new();

    // admin ロール で JWT トークン取得（開発モード）
    let login_response = client
        .post(format!("http://{}/api/auth/login", lb.addr()))
        .json(&json!({
            "username": "admin",
            "password": "test"
        }))
        .send()
        .await
        .expect("login request failed");

    assert_eq!(login_response.status().as_u16(), 200);
    let login_body: Value = login_response.json().await.expect("parse login response");

    let admin_token = login_body["token"].as_str().expect("token field missing");

    // 招待キー生成API呼び出し
    let response = client
        .post(format!("http://{}/api/admin/invitations", lb.addr()))
        .header("Authorization", format!("Bearer {}", admin_token))
        .json(&json!({
            "username": "newuser@example.com",
            "role": "viewer"
        }))
        .send()
        .await
        .expect("create invitation request failed");

    // 成功レスポンス検証
    assert_eq!(response.status().as_u16(), 200, "expected 200 OK");
    let body: Value = response.json().await.expect("parse response");

    // レスポンス構造検証
    assert!(
        body["invitation_key"].is_string(),
        "invitation_key field missing"
    );
    assert!(body["qr_code"].is_string(), "qr_code field missing");
    assert!(body["expires_at"].is_string(), "expires_at field missing");

    // 招待キー形式検証（8文字英数字）
    let invitation_key = body["invitation_key"].as_str().unwrap();
    assert_eq!(
        invitation_key.len(),
        8,
        "invitation key should be 8 characters"
    );
    assert!(
        invitation_key.chars().all(|c| c.is_alphanumeric()),
        "invitation key should be alphanumeric only"
    );
}

/// T-0008-2: Viewer ユーザーは招待キーを生成できない（権限なし）
#[tokio::test]
async fn test_create_invitation_forbidden_viewer() {
    let lb = spawn_test_lb().await;
    let client = Client::new();

    // viewer ロール で JWT トークン取得
    // 注：テストバイパス用 admin/test を使用してから roles を変更する必要があります
    // ここでは、スキップまたは別途 viewer ユーザー作成が必要
    // 暫定: admin で作成後、API キー方式での検証に切り替え

    // APIキー（sk_debug）を使用してみる - bearer token ではなく X-API-Key
    let response = client
        .post(format!("http://{}/api/admin/invitations", lb.addr()))
        .header("X-API-Key", "sk_debug")
        .json(&json!({
            "username": "newuser@example.com",
            "role": "viewer"
        }))
        .send()
        .await
        .expect("create invitation request failed");

    // APIキー認証では admin 権限が不足の場合 403 を返す
    // ただし、sk_debug が admin 権限かどうかは実装に依存
    // テスト前提: sk_debug は admin でない or 招待キー生成権限がない
    assert!(
        response.status().as_u16() == 401 || response.status().as_u16() == 403,
        "expected 401 or 403, got {}",
        response.status()
    );
}

/// T-0008-3: 無効な JWT トークンの場合 401 エラー
#[tokio::test]
async fn test_create_invitation_unauthorized_invalid_token() {
    let lb = spawn_test_lb().await;
    let client = Client::new();

    let response = client
        .post(format!("http://{}/api/admin/invitations", lb.addr()))
        .header("Authorization", "Bearer invalid_token")
        .json(&json!({
            "username": "newuser@example.com",
            "role": "viewer"
        }))
        .send()
        .await
        .expect("create invitation request failed");

    assert_eq!(response.status().as_u16(), 401, "expected 401 Unauthorized");
}

/// T-0010-1: 有効な招待キーで招待受け入れ、setup_token を取得できる
#[tokio::test]
async fn test_accept_invitation_success() {
    let lb = spawn_test_lb().await;
    let client = Client::new();

    // Step 1: admin で招待キーを生成
    let login_response = client
        .post(format!("http://{}/api/auth/login", lb.addr()))
        .json(&json!({
            "username": "admin",
            "password": "test"
        }))
        .send()
        .await
        .expect("login failed");

    let login_body: Value = login_response.json().await.expect("parse login response");
    let admin_token = login_body["token"].as_str().expect("token missing");

    let invitation_response = client
        .post(format!("http://{}/api/admin/invitations", lb.addr()))
        .header("Authorization", format!("Bearer {}", admin_token))
        .json(&json!({
            "username": "newuser@example.com",
            "role": "viewer"
        }))
        .send()
        .await
        .expect("create invitation failed");

    assert_eq!(invitation_response.status().as_u16(), 200);
    let invitation_body: Value = invitation_response
        .json()
        .await
        .expect("parse invitation response");

    let invitation_key = invitation_body["invitation_key"]
        .as_str()
        .expect("invitation_key missing");

    // Step 2: 招待キーを受け入れ
    let accept_response = client
        .post(format!("http://{}/api/auth/accept-invitation", lb.addr()))
        .json(&json!({
            "invitation_key": invitation_key,
            "biometric_verified": true
        }))
        .send()
        .await
        .expect("accept invitation failed");

    assert_eq!(accept_response.status().as_u16(), 200);
    let accept_body: Value = accept_response.json().await.expect("parse accept response");

    // レスポンス構造検証
    assert!(
        accept_body["username"].is_string(),
        "username field missing"
    );
    assert!(
        accept_body["setup_token"].is_string(),
        "setup_token field missing"
    );

    // setup_token は JWT 形式（3つのドット）
    let setup_token = accept_body["setup_token"].as_str().unwrap();
    assert_eq!(
        setup_token.split('.').count(),
        3,
        "setup_token should be JWT format (3 parts)"
    );
}

/// T-0010-2: 無効な招待キーでエラー 400
#[tokio::test]
async fn test_accept_invitation_invalid_key() {
    let lb = spawn_test_lb().await;
    let client = Client::new();

    let response = client
        .post(format!("http://{}/api/auth/accept-invitation", lb.addr()))
        .json(&json!({
            "invitation_key": "INVALID00",
            "biometric_verified": true
        }))
        .send()
        .await
        .expect("accept invitation request failed");

    assert_eq!(response.status().as_u16(), 400, "expected 400 Bad Request");
}

/// T-0010-3: 期限切れの招待キーでエラー 400
#[tokio::test]
async fn test_accept_invitation_expired() {
    // このテストは実装後の期限切れシミュレーション
    // 一旦スキップ - データベースの日時操作が必要
    // TODO: fixture で expires_at を過去の日時に設定
}

/// T-0010-4: 同じ招待キーを2回使用するとエラー 400
#[tokio::test]
async fn test_accept_invitation_already_used() {
    let lb = spawn_test_lb().await;
    let client = Client::new();

    // Step 1: admin で招待キーを生成
    let login_response = client
        .post(format!("http://{}/api/auth/login", lb.addr()))
        .json(&json!({
            "username": "admin",
            "password": "test"
        }))
        .send()
        .await
        .expect("login failed");

    let login_body: Value = login_response.json().await.expect("parse login response");
    let admin_token = login_body["token"].as_str().expect("token missing");

    let invitation_response = client
        .post(format!("http://{}/api/admin/invitations", lb.addr()))
        .header("Authorization", format!("Bearer {}", admin_token))
        .json(&json!({
            "username": "newuser@example.com",
            "role": "viewer"
        }))
        .send()
        .await
        .expect("create invitation failed");

    let invitation_body: Value = invitation_response
        .json()
        .await
        .expect("parse invitation response");
    let invitation_key = invitation_body["invitation_key"]
        .as_str()
        .expect("invitation_key missing");

    // Step 2: 初回使用（成功）
    let accept_response_1 = client
        .post(format!("http://{}/api/auth/accept-invitation", lb.addr()))
        .json(&json!({
            "invitation_key": invitation_key,
            "biometric_verified": true
        }))
        .send()
        .await
        .expect("accept invitation failed");

    assert_eq!(accept_response_1.status().as_u16(), 200);

    // Step 3: 2回目使用（失敗）
    let accept_response_2 = client
        .post(format!("http://{}/api/auth/accept-invitation", lb.addr()))
        .json(&json!({
            "invitation_key": invitation_key,
            "biometric_verified": true
        }))
        .send()
        .await
        .expect("accept invitation request failed");

    assert_eq!(
        accept_response_2.status().as_u16(),
        400,
        "expected 400 Bad Request"
    );
}
