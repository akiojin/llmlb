//! Integration Test: US11 - 手動タイプ指定
//!
//! SPEC-66555000: エンドポイントタイプ自動判別機能
//!
//! 管理者として、タイプを手動で指定・変更したい
//! （誤判別時の修正、またはオフラインエンドポイントの事前設定）。

use llmlb::common::auth::{ApiKeyPermission, UserRole};
use reqwest::Client;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::support::lb::spawn_test_lb_with_db;

async fn create_admin_api_key(db_pool: &SqlitePool) -> String {
    let password_hash = llmlb::auth::password::hash_password("password123").unwrap();
    let created = llmlb::db::users::create(db_pool, "admin", &password_hash, UserRole::Admin).await;
    let admin_id = match created {
        Ok(user) => user.id,
        Err(_) => {
            llmlb::db::users::find_by_username(db_pool, "admin")
                .await
                .unwrap()
                .unwrap()
                .id
        }
    };

    let api_key = llmlb::db::api_keys::create(
        db_pool,
        "test-admin-key",
        admin_id,
        None,
        ApiKeyPermission::all(),
    )
    .await
    .expect("create admin api key");
    api_key.key
}

/// US11-シナリオ1: 登録時に手動でタイプを指定
#[tokio::test]
async fn test_manual_type_on_registration() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // タイプを手動指定してエンドポイント登録
    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Manual xLLM",
            "base_url": "http://localhost:8080",
            "endpoint_type": "xllm",  // 手動指定
            "endpoint_type_reason": "manual for testing"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 201);

    let body: Value = response.json().await.unwrap();

    // 手動指定したタイプが反映されていることを確認
    assert_eq!(
        body["endpoint_type"], "xllm",
        "Manual type should be applied"
    );
    assert_eq!(body["endpoint_type_source"], "manual");
    assert_eq!(body["endpoint_type_reason"], "manual for testing");
    assert!(body["endpoint_type_detected_at"].is_string());
}

/// US11-シナリオ2: 既存エンドポイントのタイプを手動変更（PUT）
#[tokio::test]
async fn test_manual_type_update() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // エンドポイント登録（タイプはunknown）
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Test Endpoint",
            "base_url": "http://localhost:9999"
        }))
        .send()
        .await
        .unwrap();

    let body: Value = register_response.json().await.unwrap();
    let endpoint_id = body["id"].as_str().unwrap();

    // タイプをxLLMに手動変更
    let update_response = client
        .put(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Test Endpoint",
            "base_url": "http://localhost:9999",
            "endpoint_type": "xllm",
            "endpoint_type_reason": "override during update"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(update_response.status().as_u16(), 200);

    // 詳細取得で確認
    let detail_response = client
        .get(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", format!("Bearer {}", admin_key))
        .send()
        .await
        .unwrap();

    let detail: Value = detail_response.json().await.unwrap();
    assert_eq!(
        detail["endpoint_type"], "xllm",
        "Type should be updated to xllm"
    );
    assert_eq!(detail["endpoint_type_source"], "manual");
    assert_eq!(detail["endpoint_type_reason"], "override during update");
    assert!(detail["endpoint_type_detected_at"].is_string());
}

/// US11-シナリオ3: 手動指定は自動判別より優先される
#[tokio::test]
async fn test_manual_type_overrides_auto_detection() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // Ollamaエンドポイント（モック）を手動でxLLMとして登録
    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Manual Override",
            "base_url": "http://localhost:11434",  // Ollamaポート
            "endpoint_type": "xllm"  // 手動でxLLMを指定
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 201);

    let body: Value = response.json().await.unwrap();

    // 自動判別ではなく手動指定が優先される
    assert_eq!(
        body["endpoint_type"], "xllm",
        "Manual type should override auto-detection"
    );
    assert_eq!(body["endpoint_type_source"], "manual");
    assert!(body["endpoint_type_reason"].is_string());
    assert!(body["endpoint_type_detected_at"].is_string());
}

/// US11-シナリオ4: 不正なタイプ指定はエラー
#[tokio::test]
async fn test_invalid_type_specification() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // 不正なタイプを指定
    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Invalid Type",
            "base_url": "http://localhost:8080",
            "endpoint_type": "invalid_type"  // 不正なタイプ
        }))
        .send()
        .await
        .unwrap();

    // 400 Bad Request または 422 Unprocessable Entity を期待
    let status = response.status().as_u16();
    assert!(
        status == 400 || status == 422,
        "invalid endpoint_type should be rejected with 400 or 422, got {status}"
    );
}

/// US11-シナリオ5: 全ての有効なタイプを手動指定可能
#[tokio::test]
async fn test_all_valid_types_can_be_specified() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let valid_types = [
        "xllm",
        "ollama",
        "vllm",
        "lm_studio",
        "openai_compatible",
        "unknown",
    ];

    for (i, endpoint_type) in valid_types.iter().enumerate() {
        let response = client
            .post(format!("http://{}/api/endpoints", server.addr()))
            .header("authorization", format!("Bearer {}", admin_key))
            .json(&json!({
                "name": format!("Endpoint Type {}", endpoint_type),
                "base_url": format!("http://localhost:{}", 8000 + i),
                "endpoint_type": endpoint_type
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(
            response.status().as_u16(),
            201,
            "Should accept type: {}",
            endpoint_type
        );

        let body: Value = response.json().await.unwrap();
        assert_eq!(
            body["endpoint_type"].as_str().unwrap(),
            *endpoint_type,
            "Type should match: {}",
            endpoint_type
        );
    }
}

/// US11-シナリオ6: タイプ変更後も他のフィールドは保持される
#[tokio::test]
async fn test_type_update_preserves_other_fields() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // エンドポイント登録（メモ付き）
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Test Endpoint",
            "base_url": "http://localhost:9999",
            "notes": "Important server"
        }))
        .send()
        .await
        .unwrap();

    let body: Value = register_response.json().await.unwrap();
    let endpoint_id = body["id"].as_str().unwrap();

    // タイプを変更（他のフィールドも指定）
    let update_response = client
        .put(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Test Endpoint",
            "base_url": "http://localhost:9999",
            "endpoint_type": "xllm",
            "notes": "Important server"  // メモを保持
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(update_response.status().as_u16(), 200);

    // 詳細取得でメモが保持されていることを確認
    let detail_response = client
        .get(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", format!("Bearer {}", admin_key))
        .send()
        .await
        .unwrap();

    let detail: Value = detail_response.json().await.unwrap();
    assert_eq!(detail["endpoint_type"], "xllm");
    assert_eq!(
        detail["notes"], "Important server",
        "Notes should be preserved"
    );
}
