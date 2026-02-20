//! IP記録E2Eテスト
//!
//! SPEC-62ac4b68 T009: 推論リクエスト送信後に
//! request_historyテーブルのclient_ipとapi_key_idが記録されることを検証

use axum::{response::IntoResponse, routing::post, Json, Router};
use reqwest::Client;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::support;
use support::http::spawn_lb;

/// スタブエンドポイント: /v1/chat/completionsに200で応答
async fn stub_chat_handler(Json(_payload): Json<Value>) -> impl IntoResponse {
    Json(json!({
        "id": "chatcmpl-test",
        "object": "chat.completion",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "Hello from stub"
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15
        }
    }))
}

/// スタブエンドポイント: /v1/modelsに応答
async fn stub_models_handler() -> impl IntoResponse {
    Json(json!({
        "data": [{"id": "test-model", "object": "model"}],
        "object": "list"
    }))
}

/// テスト用スタブノードサーバーを起動
async fn spawn_stub_node() -> support::http::TestServer {
    let app = Router::new()
        .route("/v1/chat/completions", post(stub_chat_handler))
        .route("/v1/models", axum::routing::get(stub_models_handler));
    spawn_lb(app).await
}

/// エンドポイントを登録してモデルを同期
async fn register_and_sync(
    lb: &support::http::TestServer,
    stub: &support::http::TestServer,
) -> String {
    let client = Client::new();

    let resp = client
        .post(format!("http://{}/api/endpoints", lb.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "ip-test-stub",
            "base_url": format!("http://{}", stub.addr())
        }))
        .send()
        .await
        .expect("endpoint create");
    let body: Value = resp.json().await.unwrap();
    let endpoint_id = body["id"].as_str().unwrap().to_string();

    let _ = client
        .post(format!(
            "http://{}/api/endpoints/{}/test",
            lb.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("endpoint test");

    let _ = client
        .post(format!(
            "http://{}/api/endpoints/{}/sync",
            lb.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("endpoint sync");

    endpoint_id
}

/// request_historyから最新レコードのclient_ipとapi_key_idを取得
async fn get_latest_record(
    db_pool: &SqlitePool,
) -> Option<(Option<String>, Option<String>)> {
    sqlx::query_as::<_, (Option<String>, Option<String>)>(
        "SELECT client_ip, api_key_id FROM request_history ORDER BY rowid DESC LIMIT 1",
    )
    .fetch_optional(db_pool)
    .await
    .unwrap()
}

/// api_keysテーブルから最新のAPIキーUUIDを取得
async fn get_latest_api_key_id(db_pool: &SqlitePool) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT id FROM api_keys ORDER BY created_at DESC LIMIT 1")
        .fetch_optional(db_pool)
        .await
        .unwrap()
}

/// T009: /v1/chat/completions送信後にclient_ipとapi_key_idが記録される
#[tokio::test]
async fn test_ip_and_api_key_recorded_on_inference_request() {
    // 1. スタブノードとテストサーバーを起動
    let stub = spawn_stub_node().await;
    let (lb, db_pool) = support::lb::spawn_test_lb_with_db().await;

    // 2. APIキーを作成
    let api_key = support::lb::create_test_api_key(lb.addr(), &db_pool).await;

    // 3. APIキーのUUIDを取得
    let expected_api_key_id = get_latest_api_key_id(&db_pool).await;
    assert!(
        expected_api_key_id.is_some(),
        "APIキーのUUIDが取得できること"
    );

    // 4. エンドポイントを登録してモデル同期
    let _endpoint_id = register_and_sync(&lb, &stub).await;

    // 5. /v1/chat/completionsリクエストを送信
    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/chat/completions", lb.addr()))
        .header("authorization", format!("Bearer {}", api_key))
        .json(&json!({
            "model": "test-model",
            "messages": [{"role": "user", "content": "hello"}]
        }))
        .send()
        .await
        .expect("リクエスト送信成功");
    assert!(
        response.status().is_success(),
        "推論リクエストが成功すること（status: {}）",
        response.status()
    );

    // 非同期保存を待つ
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // 6. request_historyのclient_ipとapi_key_idを検証
    let record = get_latest_record(&db_pool).await;
    assert!(record.is_some(), "request_historyにレコードが記録されること");

    let (client_ip, api_key_id) = record.unwrap();

    // client_ipがNULLでないこと（127.0.0.1が記録される）
    assert!(
        client_ip.is_some(),
        "client_ipがNULLでないこと（実際: {:?}）",
        client_ip
    );
    let ip = client_ip.unwrap();
    assert!(
        ip == "127.0.0.1" || ip == "::1",
        "client_ipがlocalhostであること（実際: {}）",
        ip
    );

    // api_key_idが認証に使用したキーのUUIDと一致すること
    assert!(
        api_key_id.is_some(),
        "api_key_idがNULLでないこと（実際: {:?}）",
        api_key_id
    );
    assert_eq!(
        api_key_id.unwrap(),
        expected_api_key_id.unwrap(),
        "api_key_idが認証に使用したキーのUUIDと一致すること"
    );

    stub.stop().await;
    lb.stop().await;
}
