//! Integration Test: ノードライフサイクル
//!
//! ノード登録 → ヘルスチェック → オフライン検知
//! このテストはRED状態であることが期待されます（T027-T049で実装後にGREENになる）

#[tokio::test]
async fn test_agent_registers_and_sends_heartbeat() {
    // Arrange: Coordinatorサーバー起動
    // let coordinator = start_test_coordinator().await;

    // Act: ノード登録
    // let node_id = register_test_agent(&coordinator).await;

    // Assert: ノードが登録された
    // let nodes = coordinator.list_agents().await;
    // assert_eq!(nodes.len(), 1);
    // assert_eq!(nodes[0].id, node_id);
    // assert_eq!(nodes[0].status, "online");

    // Act: ヘルスチェック送信
    // send_heartbeat(&coordinator, node_id).await;

    // Assert: last_seenが更新された
    // let nodes = coordinator.list_agents().await;
    // assert!(nodes[0].last_seen > initial_last_seen);

    // TODO: T027-T049で実装後にアンコメント
    panic!("RED: ノードライフサイクルが未実装");
}

#[tokio::test]
async fn test_node_timeout_detection() {
    // Arrange: Coordinatorサーバー起動、ノード登録
    // let coordinator = start_test_coordinator().await;
    // let node_id = register_test_agent(&coordinator).await;

    // Act: 60秒以上ヘルスチェックを送信しない（タイムアウトシミュレーション）
    // tokio::time::sleep(Duration::from_secs(61)).await;

    // Assert: ノードがオフラインになった
    // let nodes = coordinator.list_agents().await;
    // assert_eq!(nodes[0].status, "offline");

    // TODO: T027-T049で実装後にアンコメント
    panic!("RED: ノードタイムアウト検知が未実装");
}

#[tokio::test]
async fn test_agent_auto_reconnect() {
    // Arrange: Coordinatorサーバー起動、ノード登録後にオフライン
    // let coordinator = start_test_coordinator().await;
    // let node_id = register_test_agent(&coordinator).await;
    // simulate_agent_offline(&coordinator, node_id).await;

    // Act: ノード再登録
    // let new_node_id = register_test_agent(&coordinator).await;

    // Assert: ノードがオンラインに戻った
    // assert_eq!(node_id, new_node_id); // 同じIDで再登録
    // let nodes = coordinator.list_agents().await;
    // assert_eq!(nodes[0].status, "online");

    // TODO: T027-T049で実装後にアンコメント
    panic!("RED: ノード自動再接続が未実装");
}
