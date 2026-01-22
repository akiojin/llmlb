//! Contract Test: OpenAI API Logging
//!
//! SPEC-1970e39f: 構造化ロギング強化
//! - FR-001: リクエスト受信ログ
//! - FR-003: プロキシエラーログ
//! - FR-004: ノード選択失敗時の履歴保存

use crate::support::router::spawn_test_router_with_db;
use llmlb::db::models::ModelStorage;
use llmlb::db::request_history::RequestHistoryStorage;
use llmlb::registry::models::ModelInfo;
use llmlb_common::protocol::RecordStatus;
use reqwest::Client;
use serial_test::serial;
use sqlx::SqlitePool;
use std::sync::Arc;

/// テスト用のモデル情報を作成
fn create_test_model(name: &str) -> ModelInfo {
    ModelInfo::new(
        name.to_string(),
        0,
        format!("Test model: {}", name),
        0,
        vec![],
    )
}

/// SQLiteからリクエスト履歴を読み込む
async fn load_request_history_from_db(
    db_pool: &SqlitePool,
) -> Vec<llmlb_common::protocol::RequestResponseRecord> {
    let storage = Arc::new(RequestHistoryStorage::new(db_pool.clone()));
    storage.load_records().await.unwrap_or_default()
}

/// T002: chat_completionsリクエスト時にレスポンスが返ることを検証
/// (ログ出力自体はtracing subscriberで検証が複雑なため、
///  リクエストが正常に処理されることを確認)
#[tokio::test]
#[serial]
async fn test_chat_completions_request_processed() {
    let (router, db_pool) = spawn_test_router_with_db().await;
    let client = Client::new();

    // モデルをDBに登録（ノードは登録しない）
    let model = create_test_model("gpt-oss-20b");
    let storage = ModelStorage::new(db_pool.clone());
    storage.save_model(&model).await.unwrap();

    // ノードなしでリクエスト送信 → 503が返るはず
    let response = client
        .post(format!("http://{}/v1/chat/completions", router.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "model": "gpt-oss-20b",
            "messages": [{"role": "user", "content": "Hello"}]
        }))
        .send()
        .await
        .expect("request should be sent");

    // モデルは登録されているが、ノードがないので503（Service Unavailable）が返る
    assert_eq!(
        response.status().as_u16(),
        503,
        "Should return 503 when no nodes available"
    );
}

/// T003: ノード選択失敗時に適切なエラーレスポンスが返ることを検証
#[tokio::test]
#[serial]
async fn test_node_selection_failure_returns_error() {
    let (router, db_pool) = spawn_test_router_with_db().await;
    let client = Client::new();

    // モデルをDBに登録（ノードは登録しない）
    let model = create_test_model("test-model");
    let storage = ModelStorage::new(db_pool.clone());
    storage.save_model(&model).await.unwrap();

    // ノードなしでリクエスト送信
    let response = client
        .post(format!("http://{}/v1/chat/completions", router.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "model": "test-model",
            "messages": [{"role": "user", "content": "Test"}]
        }))
        .send()
        .await
        .expect("request should be sent");

    assert_eq!(response.status().as_u16(), 503);

    let body = response.text().await.expect("body should be readable");
    // エラーメッセージにノード関連の情報が含まれることを確認
    assert!(
        body.contains("No nodes available")
            || body.contains("nodes")
            || body.contains("No available nodes"),
        "Error message should mention nodes: {}",
        body
    );
}

/// T004: ノード選択失敗時にリクエスト履歴が保存されることを検証
///
/// 現在の実装では、select_available_node()が失敗すると?演算子で
/// 早期リターンし、save_request_record()が呼ばれない。
/// このテストは実装後にGREENになる。
#[tokio::test]
#[serial]
async fn test_node_selection_failure_saves_request_history() {
    // spawn_test_router_with_db()でルーターとDBプールを取得
    let (router, db_pool) = spawn_test_router_with_db().await;
    let client = Client::new();

    // モデルをDBに登録（ノードは登録しない）
    let model = create_test_model("test-model-for-history");
    let storage = ModelStorage::new(db_pool.clone());
    storage.save_model(&model).await.unwrap();

    // ノードなしでリクエスト送信
    let _response = client
        .post(format!("http://{}/v1/chat/completions", router.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "model": "test-model-for-history",
            "messages": [{"role": "user", "content": "Test history save"}]
        }))
        .send()
        .await
        .expect("request should be sent");

    // 非同期保存の完了を待つ（カバレッジビルドでは遅延があるためリトライ）
    let mut records = Vec::new();
    for _ in 0..10 {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        records = load_request_history_from_db(&db_pool).await;
        if !records.is_empty() {
            break;
        }
    }

    // FR-004: ノード選択失敗時もリクエスト履歴に記録する必要がある
    assert!(
        !records.is_empty(),
        "Request history should be saved even when node selection fails"
    );

    // 最新のレコードを確認
    let latest = records.last().expect("Should have at least one record");
    assert_eq!(latest.model, "test-model-for-history");

    // ステータスがエラーであることを確認
    match &latest.status {
        RecordStatus::Error { message } => {
            assert!(
                message.contains("Node selection failed")
                    || message.contains("No nodes")
                    || message.contains("No available nodes")
                    || message.contains("support model"),
                "Error message should indicate node selection failure: {}",
                message
            );
        }
        RecordStatus::Success => {
            panic!("Status should be Error, not Success");
        }
    }
}
