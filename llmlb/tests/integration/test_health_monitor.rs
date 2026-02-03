//! Integration Test: ヘルスモニター
//!
//! 定期ヘルスチェック → タイムアウト検知 → 振り分け対象から除外
//! このテストはRED状態であることが期待されます（T033-T049で実装後にGREENになる）

#[tokio::test]
#[ignore = "TDD RED: ヘルスモニター未実装"]
async fn test_health_monitor_detects_timeout() {
    // Arrange: Routerサーバー起動（ヘルスモニター有効）、ノード登録
    // let lb = start_test_lb_with_health_monitor().await;
    // let node_id = register_test_node(&lb).await;

    // Act: 60秒以上ヘルスチェックを送信しない
    // tokio::time::sleep(Duration::from_secs(61)).await;

    // Assert: ノードが自動的にオフラインになった
    // let nodes = lb.list_nodes().await;
    // assert_eq!(nodes[0].status, "offline");

    // TODO: T033-T049で実装後にアンコメント
    panic!("RED: ヘルスモニターが未実装");
}

#[tokio::test]
#[ignore = "TDD RED: オフラインノード除外未実装"]
async fn test_offline_node_excluded_from_balancing() {
    // Arrange: Routerサーバー起動、2台のノード登録（1台はオフライン）
    // let lb = start_test_lb().await;
    // let node1 = register_test_node(&lb).await; // オンライン
    // let node2 = register_test_node(&lb).await; // オフライン
    // simulate_node_offline(&lb, node2).await;

    // Act: 5個のリクエストを送信
    // for _ in 0..5 {
    //     let request = json!({
    //         "model": "llama2",
    //         "messages": [{"role": "user", "content": "Hello"}]
    //     });
    //     lb.post("/v1/chat/completions", request).await;
    // }

    // Assert: オンラインのnode1のみが処理した
    // let metrics = lb.get_node_metrics().await;
    // assert_eq!(metrics[&node1].total_requests, 5);
    // assert_eq!(metrics[&node2].total_requests, 0);

    // TODO: T033-T049で実装後にアンコメント
    panic!("RED: オフラインノード除外が未実装");
}
