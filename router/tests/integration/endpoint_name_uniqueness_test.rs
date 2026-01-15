//! Integration Test: T016a - 名前重複検証
//!
//! SPEC-66555000: ルーター主導エンドポイント登録システム
//!
//! エンドポイント名の一意性を検証する。

use reqwest::Client;
use serde_json::{json, Value};

use crate::support::router::spawn_test_router;

/// 同じ名前のエンドポイントを登録しようとすると400エラー
#[tokio::test]
#[ignore = "TDD RED: 名前重複チェック未実装"]
async fn test_duplicate_name_rejected() {
    let server = spawn_test_router().await;
    let client = Client::new();

    // 最初のエンドポイント登録
    let first_resp = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Production Ollama",
            "base_url": "http://localhost:11434"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(first_resp.status().as_u16(), 201);

    // 同じ名前で異なるURLのエンドポイントを登録（名前重複）
    let dup_resp = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Production Ollama",
            "base_url": "http://localhost:8000"
        }))
        .send()
        .await
        .unwrap();

    // 名前重複は400 Bad Request（URLの重複は409 Conflict）
    assert_eq!(dup_resp.status().as_u16(), 400);
}

/// 大文字小文字を区別して名前を検証
#[tokio::test]
async fn test_name_case_sensitivity() {
    let server = spawn_test_router().await;
    let client = Client::new();

    // 最初のエンドポイント登録
    let _ = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Production",
            "base_url": "http://localhost:11434"
        }))
        .send()
        .await
        .unwrap();

    // 大文字小文字が異なる名前は別物として扱う
    let diff_case_resp = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "PRODUCTION",
            "base_url": "http://localhost:8000"
        }))
        .send()
        .await
        .unwrap();

    // 大文字小文字が異なれば別のエンドポイントとして登録可能
    assert_eq!(diff_case_resp.status().as_u16(), 201);
}

/// 削除後に同じ名前を再利用可能
#[tokio::test]
async fn test_name_reusable_after_deletion() {
    let server = spawn_test_router().await;
    let client = Client::new();

    // エンドポイント登録
    let first_resp = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Reusable Name",
            "base_url": "http://localhost:11434"
        }))
        .send()
        .await
        .unwrap();

    let first_body: Value = first_resp.json().await.unwrap();
    let endpoint_id = first_body["id"].as_str().unwrap();

    // 削除
    let _ = client
        .delete(format!(
            "http://{}/v0/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    // 同じ名前で再登録可能
    let reuse_resp = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Reusable Name",
            "base_url": "http://localhost:8000"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(reuse_resp.status().as_u16(), 201);
}

/// 更新時に他のエンドポイントの名前と重複を防止
#[tokio::test]
#[ignore = "TDD RED: 名前重複チェック未実装"]
async fn test_update_name_uniqueness() {
    let server = spawn_test_router().await;
    let client = Client::new();

    // 2つのエンドポイントを登録
    let _ = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Endpoint A",
            "base_url": "http://localhost:11434"
        }))
        .send()
        .await
        .unwrap();

    let second_resp = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Endpoint B",
            "base_url": "http://localhost:8000"
        }))
        .send()
        .await
        .unwrap();

    let second_body: Value = second_resp.json().await.unwrap();
    let endpoint_b_id = second_body["id"].as_str().unwrap();

    // Endpoint Bの名前をEndpoint Aに変更しようとする
    let update_resp = client
        .put(format!(
            "http://{}/v0/endpoints/{}",
            server.addr(),
            endpoint_b_id
        ))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Endpoint A"
        }))
        .send()
        .await
        .unwrap();

    // 重複は400
    assert_eq!(update_resp.status().as_u16(), 400);
}
