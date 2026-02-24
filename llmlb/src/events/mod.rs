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
    /// アップデート状態変更イベント
    ///
    /// アップデートチェック・適用・ロールバック・スケジュール操作後に発行
    UpdateStateChanged,
    /// TPS更新イベント（SPEC-4bb5b55f）
    TpsUpdated {
        /// エンドポイントID
        endpoint_id: Uuid,
        /// モデルID
        model_id: String,
        /// TPS（tokens/sec）
        tps: f64,
        /// 出力トークン数
        output_tokens: u32,
        /// 処理時間（ミリ秒）
        duration_ms: u64,
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

    // T017: DashboardEvent::TpsUpdated シリアライゼーションテスト（SPEC-4bb5b55f Phase 4）

    #[test]
    fn test_tps_updated_event_serialization() {
        let endpoint_id = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
        let event = DashboardEvent::TpsUpdated {
            endpoint_id,
            model_id: "llama3.2:3b".to_string(),
            tps: 42.5,
            output_tokens: 100,
            duration_ms: 2353,
        };

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "TpsUpdated");
        let data = &json["data"];
        assert_eq!(data["endpoint_id"], "12345678-1234-1234-1234-123456789abc");
        assert_eq!(data["model_id"], "llama3.2:3b");
        assert!((data["tps"].as_f64().unwrap() - 42.5).abs() < 0.01);
        assert_eq!(data["output_tokens"], 100);
        assert_eq!(data["duration_ms"], 2353);
    }

    #[test]
    fn test_update_state_changed_event_serialization() {
        let event = DashboardEvent::UpdateStateChanged;

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "UpdateStateChanged");
    }

    #[tokio::test]
    async fn test_update_state_changed_event_broadcast() {
        let bus = DashboardEventBus::new();
        let mut receiver = bus.subscribe();

        bus.publish(DashboardEvent::UpdateStateChanged);

        let received = receiver.recv().await.unwrap();
        match received {
            DashboardEvent::UpdateStateChanged => {}
            _ => panic!("Expected UpdateStateChanged event"),
        }
    }

    #[tokio::test]
    async fn test_tps_updated_event_broadcast() {
        let bus = DashboardEventBus::new();
        let mut receiver = bus.subscribe();

        bus.publish(DashboardEvent::TpsUpdated {
            endpoint_id: Uuid::new_v4(),
            model_id: "test-model".to_string(),
            tps: 50.0,
            output_tokens: 200,
            duration_ms: 4000,
        });

        let received = receiver.recv().await.unwrap();
        match received {
            DashboardEvent::TpsUpdated { model_id, tps, .. } => {
                assert_eq!(model_id, "test-model");
                assert!((tps - 50.0).abs() < 0.01);
            }
            _ => panic!("Expected TpsUpdated event"),
        }
    }
}
