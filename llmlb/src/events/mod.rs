//! ダッシュボードイベントバス
//!
//! エンドポイント登録・状態変化・メトリクス更新などのイベントを
//! WebSocketクライアントにブロードキャストするための基盤

use crate::types::endpoint::EndpointStatus;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

/// イベントバスのチャネル容量
const EVENT_CHANNEL_CAPACITY: usize = 1024;

/// ダッシュボードイベント
///
/// WebSocketクライアントに送信されるイベントの種類
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum DashboardEvent {
    /// ノード登録イベント
    NodeRegistered {
        /// ランタイムID
        runtime_id: Uuid,
        /// マシン名
        machine_name: String,
        /// IPアドレス
        ip_address: String,
        /// ステータス
        status: EndpointStatus,
    },
    /// ノード状態変化イベント
    EndpointStatusChanged {
        /// ランタイムID
        runtime_id: Uuid,
        /// 旧ステータス
        old_status: EndpointStatus,
        /// 新ステータス
        new_status: EndpointStatus,
    },
    /// メトリクス更新イベント
    MetricsUpdated {
        /// ランタイムID
        runtime_id: Uuid,
        /// CPU使用率
        cpu_usage: Option<f32>,
        /// メモリ使用率
        memory_usage: Option<f32>,
        /// GPU使用率
        gpu_usage: Option<f32>,
    },
    /// ノード削除イベント
    NodeRemoved {
        /// ランタイムID
        runtime_id: Uuid,
    },
}

/// ダッシュボードイベントバス
///
/// ノード状態変化などのイベントをWebSocketクライアントにブロードキャストする
#[derive(Clone)]
pub struct DashboardEventBus {
    sender: broadcast::Sender<DashboardEvent>,
}

impl Default for DashboardEventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl DashboardEventBus {
    /// 新しいイベントバスを作成
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self { sender }
    }

    /// イベントバスを購読
    ///
    /// WebSocketハンドラーがイベントを受信するために使用
    pub fn subscribe(&self) -> broadcast::Receiver<DashboardEvent> {
        self.sender.subscribe()
    }

    /// イベントを発行
    ///
    /// 購読者がいない場合でもエラーにはならない
    pub fn publish(&self, event: DashboardEvent) {
        // 購読者がいない場合は送信に失敗するが、無視する
        let _ = self.sender.send(event);
    }

    /// 現在の購読者数を取得
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

/// Arc でラップされたイベントバス
pub type SharedEventBus = Arc<DashboardEventBus>;

/// 共有可能なイベントバスを作成
pub fn create_shared_event_bus() -> SharedEventBus {
    Arc::new(DashboardEventBus::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_bus_publish_subscribe() {
        let bus = DashboardEventBus::new();
        let mut receiver = bus.subscribe();

        let event = DashboardEvent::NodeRegistered {
            runtime_id: Uuid::new_v4(),
            machine_name: "test-node".to_string(),
            ip_address: "127.0.0.1".to_string(),
            status: EndpointStatus::Online,
        };

        bus.publish(event.clone());

        let received = receiver.recv().await.unwrap();
        match received {
            DashboardEvent::NodeRegistered { machine_name, .. } => {
                assert_eq!(machine_name, "test-node");
            }
            _ => panic!("Unexpected event type"),
        }
    }

    #[test]
    fn test_event_bus_no_subscribers() {
        let bus = DashboardEventBus::new();

        // 購読者がいなくてもパニックしないことを確認
        bus.publish(DashboardEvent::NodeRemoved {
            runtime_id: Uuid::new_v4(),
        });
    }

    #[test]
    fn test_subscriber_count() {
        let bus = DashboardEventBus::new();
        assert_eq!(bus.subscriber_count(), 0);

        let _r1 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 1);

        let _r2 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 2);
    }
}
