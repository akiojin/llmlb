//! Integration Test: ノードライフサイクル
//!
//! ノード登録 → ヘルスチェック → オフライン検知
//! このテストはRED状態であることが期待されます（T027-T049で実装後にGREENになる）

#[tokio::test]
#[ignore = "TDD RED: ノードライフサイクル未実装"]
async fn test_node_registers_and_sends_heartbeat() {
    // Arrange: Routerサーバー起動
    // let lb = start_test_lb().await;

    // Act: ノード登録
    // let node_id = register_test_node(&lb).await;

    // Assert: ノードが登録された
    // let nodes = lb.list_nodes().await;
    // assert_eq!(nodes.len(), 1);
    // assert_eq!(nodes[0].id, node_id);
    // assert_eq!(nodes[0].status, "online");

    // Act: ヘルスチェック送信
    // send_heartbeat(&lb, node_id).await;

    // Assert: last_seenが更新された
    // let nodes = lb.list_nodes().await;
    // assert!(nodes[0].last_seen > initial_last_seen);

    // TODO: T027-T049で実装後にアンコメント
    panic!("RED: ノードライフサイクルが未実装");
}

#[tokio::test]
#[ignore = "TDD RED: ノードタイムアウト検知未実装"]
async fn test_node_timeout_detection() {
    // Arrange: Routerサーバー起動、ノード登録
    // let lb = start_test_lb().await;
    // let node_id = register_test_node(&lb).await;

    // Act: 60秒以上ヘルスチェックを送信しない（タイムアウトシミュレーション）
    // tokio::time::sleep(Duration::from_secs(61)).await;

    // Assert: ノードがオフラインになった
    // let nodes = lb.list_nodes().await;
    // assert_eq!(nodes[0].status, "offline");

    // TODO: T027-T049で実装後にアンコメント
    panic!("RED: ノードタイムアウト検知が未実装");
}

#[tokio::test]
#[ignore = "TDD RED: ノード自動再接続未実装"]
async fn test_node_auto_reconnect() {
    // Arrange: Routerサーバー起動、ノード登録後にオフライン
    // let lb = start_test_lb().await;
    // let node_id = register_test_node(&lb).await;
    // simulate_node_offline(&lb, node_id).await;

    // Act: ノード再登録
    // let new_node_id = register_test_node(&lb).await;

    // Assert: ノードがオンラインに戻った
    // assert_eq!(node_id, new_node_id); // 同じIDで再登録
    // let nodes = lb.list_nodes().await;
    // assert_eq!(nodes[0].status, "online");

    // TODO: T027-T049で実装後にアンコメント
    panic!("RED: ノード自動再接続が未実装");
}
