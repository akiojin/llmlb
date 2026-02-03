//! Integration Test: LLM runtimeプロキシ
//!
//! リクエスト振り分け → LLM runtime転送 → レスポンス返却
//! このテストはRED状態であることが期待されます（T036-T039で実装後にGREENになる）

use serde_json::json;

#[tokio::test]
#[ignore = "TDD RED: LLM runtimeプロキシ未実装"]
async fn test_proxy_request_to_single_node() {
    // Arrange: Routerサーバー起動、1台のノード登録、モックLLM runtime起動
    // let lb = start_test_lb().await;
    // let mock_runtime = start_mock_runtime().await;
    // register_test_node(&lb, mock_runtime.url()).await;

    // Act: チャットリクエスト送信
    // let request = json!({
    //     "model": "llama2",
    //     "messages": [{"role": "user", "content": "Hello"}]
    // });
    // let response = app.post("/v1/chat/completions", request).await;

    // Assert: 正常にレスポンスが返された
    // assert_eq!(response.status(), 200);
    // let body: serde_json::Value = response.json();
    // assert!(body["message"].is_object());

    // TODO: T036-T039で実装後にアンコメント
    panic!("RED: LLM runtimeプロキシが未実装");
}

#[tokio::test]
#[ignore = "TDD RED: LLM runtimeプロキシ未実装"]
async fn test_proxy_no_nodes_returns_503() {
    // Arrange: Routerサーバー起動（ノード未登録）
    // let lb = start_test_lb().await;

    // Act: チャットリクエスト送信
    // let request = json!({
    //     "model": "llama2",
    //     "messages": [{"role": "user", "content": "Hello"}]
    // });
    // let response = app.post("/v1/chat/completions", request).await;

    // Assert: 503 Service Unavailable
    // assert_eq!(response.status(), 503);

    // TODO: T036-T039で実装後にアンコメント
    panic!("RED: LLM runtimeプロキシが未実装");
}
