//! ダッシュボードイベントの型とブロードキャストバス

use crate::types::endpoint::EndpointStatus;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;
use tokio::sync::broadcast;
use uuid::Uuid;

/// イベントバスのチャネル容量
const EVENT_CHANNEL_CAPACITY: usize = 1024;

/// `MetricsUpdated` の集約窓（SPEC #582 FR-048d）
///
/// リクエスト完了のたびに発生する更新を、エンドポイントごとにこの間隔で 1 回の配信へまとめる。
/// ダッシュボードのポーリング間隔（5 秒）より短く、負荷時でも配信数がリクエスト数ではなく
/// エンドポイント数に比例する値として 1 秒を選んでいる。
const METRICS_UPDATED_COALESCE_WINDOW: Duration = Duration::from_secs(1);

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
    /// 集約窓が開いているエンドポイントと、窓の終端で発行する最新の `MetricsUpdated`
    pending_metrics: Arc<Mutex<HashMap<Uuid, DashboardEvent>>>,
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
        Self {
            sender,
            pending_metrics: Arc::new(Mutex::new(HashMap::new())),
        }
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

    /// 高頻度で発生するイベントを集約して発行する
    ///
    /// `MetricsUpdated` はエンドポイントごとに集約する。最初の更新から
    /// `METRICS_UPDATED_COALESCE_WINDOW` 後に、その時点で最新の更新を 1 回だけ発行する。
    /// 窓の中で届いた更新は最新の 1 件に置き換わるため、発行はエンドポイントごとに
    /// 窓 1 つにつき 1 回に収まる。それ以外のイベントは即時に発行する。
    ///
    /// 更新の直後ではなく窓の終端で発行するのは、通知を受けたクライアントの再取得が、
    /// 窓の中で確定したすべての更新を読めるようにするため。
    ///
    /// 窓の終端を待つタスクを起動するため、tokio ランタイム上で呼び出すこと。
    pub fn publish_coalesced(&self, event: DashboardEvent) {
        let DashboardEvent::MetricsUpdated { runtime_id, .. } = &event else {
            self.publish(event);
            return;
        };
        let runtime_id = *runtime_id;

        let window_already_open = self
            .lock_pending_metrics()
            .insert(runtime_id, event)
            .is_some();
        if window_already_open {
            return;
        }

        let bus = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(METRICS_UPDATED_COALESCE_WINDOW).await;
            let latest = bus.lock_pending_metrics().remove(&runtime_id);
            if let Some(event) = latest {
                bus.publish(event);
            }
        });
    }

    fn lock_pending_metrics(&self) -> MutexGuard<'_, HashMap<Uuid, DashboardEvent>> {
        self.pending_metrics
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
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

    // --- DashboardEventBus additional tests ---

    #[test]
    fn test_event_bus_default() {
        let bus = DashboardEventBus::default();
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[test]
    fn test_subscriber_count_decreases_on_drop() {
        let bus = DashboardEventBus::new();
        let r1 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 1);
        drop(r1);
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[test]
    fn test_create_shared_event_bus() {
        let shared = create_shared_event_bus();
        assert_eq!(shared.subscriber_count(), 0);
        let _r = shared.subscribe();
        assert_eq!(shared.subscriber_count(), 1);
    }

    #[test]
    fn test_shared_event_bus_clone() {
        let shared = create_shared_event_bus();
        let shared2 = shared.clone();
        let _r = shared.subscribe();
        // Cloned bus sees same subscribers
        assert_eq!(shared2.subscriber_count(), 1);
    }

    #[test]
    fn test_event_bus_clone_shares_channel() {
        let bus1 = DashboardEventBus::new();
        let bus2 = bus1.clone();
        let _r = bus1.subscribe();
        assert_eq!(bus2.subscriber_count(), 1);
    }

    // --- DashboardEvent serialization tests ---

    #[test]
    fn test_node_registered_event_serialization() {
        let id = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
        let event = DashboardEvent::NodeRegistered {
            runtime_id: id,
            machine_name: "gpu-server-01".to_string(),
            ip_address: "192.168.1.100".to_string(),
            status: EndpointStatus::Online,
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "NodeRegistered");
        let data = &json["data"];
        assert_eq!(data["runtime_id"], "12345678-1234-1234-1234-123456789abc");
        assert_eq!(data["machine_name"], "gpu-server-01");
        assert_eq!(data["ip_address"], "192.168.1.100");
        assert_eq!(data["status"], "online");
    }

    #[test]
    fn test_endpoint_status_changed_event_serialization() {
        let id = Uuid::parse_str("abcdef12-3456-7890-abcd-ef1234567890").unwrap();
        let event = DashboardEvent::EndpointStatusChanged {
            runtime_id: id,
            old_status: EndpointStatus::Online,
            new_status: EndpointStatus::Offline,
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "EndpointStatusChanged");
        let data = &json["data"];
        assert_eq!(data["old_status"], "online");
        assert_eq!(data["new_status"], "offline");
    }

    #[test]
    fn test_metrics_updated_event_serialization_full() {
        let id = Uuid::new_v4();
        let event = DashboardEvent::MetricsUpdated {
            runtime_id: id,
            cpu_usage: Some(75.5),
            memory_usage: Some(60.0),
            gpu_usage: Some(90.0),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "MetricsUpdated");
        let data = &json["data"];
        assert!((data["cpu_usage"].as_f64().unwrap() - 75.5).abs() < 0.01);
        assert!((data["memory_usage"].as_f64().unwrap() - 60.0).abs() < 0.01);
        assert!((data["gpu_usage"].as_f64().unwrap() - 90.0).abs() < 0.01);
    }

    #[test]
    fn test_metrics_updated_event_serialization_with_nulls() {
        let id = Uuid::new_v4();
        let event = DashboardEvent::MetricsUpdated {
            runtime_id: id,
            cpu_usage: None,
            memory_usage: None,
            gpu_usage: None,
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "MetricsUpdated");
        let data = &json["data"];
        assert!(data["cpu_usage"].is_null());
        assert!(data["memory_usage"].is_null());
        assert!(data["gpu_usage"].is_null());
    }

    #[test]
    fn test_node_removed_event_serialization() {
        let id = Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap();
        let event = DashboardEvent::NodeRemoved { runtime_id: id };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "NodeRemoved");
        assert_eq!(
            json["data"]["runtime_id"],
            "11111111-2222-3333-4444-555555555555"
        );
    }

    // --- Multiple subscriber broadcast tests ---

    #[tokio::test]
    async fn test_multiple_subscribers_receive_same_event() {
        let bus = DashboardEventBus::new();
        let mut r1 = bus.subscribe();
        let mut r2 = bus.subscribe();
        let mut r3 = bus.subscribe();

        bus.publish(DashboardEvent::UpdateStateChanged);

        let e1 = r1.recv().await.unwrap();
        let e2 = r2.recv().await.unwrap();
        let e3 = r3.recv().await.unwrap();

        assert!(matches!(e1, DashboardEvent::UpdateStateChanged));
        assert!(matches!(e2, DashboardEvent::UpdateStateChanged));
        assert!(matches!(e3, DashboardEvent::UpdateStateChanged));
    }

    #[tokio::test]
    async fn test_multiple_events_in_sequence() {
        let bus = DashboardEventBus::new();
        let mut receiver = bus.subscribe();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        bus.publish(DashboardEvent::NodeRemoved { runtime_id: id1 });
        bus.publish(DashboardEvent::NodeRemoved { runtime_id: id2 });

        let e1 = receiver.recv().await.unwrap();
        let e2 = receiver.recv().await.unwrap();

        match e1 {
            DashboardEvent::NodeRemoved { runtime_id } => assert_eq!(runtime_id, id1),
            _ => panic!("Expected NodeRemoved"),
        }
        match e2 {
            DashboardEvent::NodeRemoved { runtime_id } => assert_eq!(runtime_id, id2),
            _ => panic!("Expected NodeRemoved"),
        }
    }

    #[tokio::test]
    async fn test_node_registered_event_broadcast_fields() {
        let bus = DashboardEventBus::new();
        let mut receiver = bus.subscribe();

        let id = Uuid::new_v4();
        bus.publish(DashboardEvent::NodeRegistered {
            runtime_id: id,
            machine_name: "test-machine".to_string(),
            ip_address: "10.0.0.1".to_string(),
            status: EndpointStatus::Pending,
        });

        let received = receiver.recv().await.unwrap();
        match received {
            DashboardEvent::NodeRegistered {
                runtime_id,
                machine_name,
                ip_address,
                status,
            } => {
                assert_eq!(runtime_id, id);
                assert_eq!(machine_name, "test-machine");
                assert_eq!(ip_address, "10.0.0.1");
                assert_eq!(status, EndpointStatus::Pending);
            }
            _ => panic!("Expected NodeRegistered"),
        }
    }

    #[tokio::test]
    async fn test_endpoint_status_changed_broadcast() {
        let bus = DashboardEventBus::new();
        let mut receiver = bus.subscribe();

        let id = Uuid::new_v4();
        bus.publish(DashboardEvent::EndpointStatusChanged {
            runtime_id: id,
            old_status: EndpointStatus::Pending,
            new_status: EndpointStatus::Online,
        });

        let received = receiver.recv().await.unwrap();
        match received {
            DashboardEvent::EndpointStatusChanged {
                old_status,
                new_status,
                ..
            } => {
                assert_eq!(old_status, EndpointStatus::Pending);
                assert_eq!(new_status, EndpointStatus::Online);
            }
            _ => panic!("Expected EndpointStatusChanged"),
        }
    }

    #[tokio::test]
    async fn test_metrics_updated_broadcast() {
        let bus = DashboardEventBus::new();
        let mut receiver = bus.subscribe();

        bus.publish(DashboardEvent::MetricsUpdated {
            runtime_id: Uuid::new_v4(),
            cpu_usage: Some(42.0),
            memory_usage: Some(55.5),
            gpu_usage: None,
        });

        let received = receiver.recv().await.unwrap();
        match received {
            DashboardEvent::MetricsUpdated {
                cpu_usage,
                memory_usage,
                gpu_usage,
                ..
            } => {
                assert_eq!(cpu_usage, Some(42.0));
                assert_eq!(memory_usage, Some(55.5));
                assert!(gpu_usage.is_none());
            }
            _ => panic!("Expected MetricsUpdated"),
        }
    }

    // --- publish_coalesced（SPEC #582 FR-048d）---

    fn metrics_updated(runtime_id: Uuid, cpu_usage: f32) -> DashboardEvent {
        DashboardEvent::MetricsUpdated {
            runtime_id,
            cpu_usage: Some(cpu_usage),
            memory_usage: None,
            gpu_usage: None,
        }
    }

    /// 集約窓が閉じるまで待ち、それまでに発行されたイベントをすべて取り出す
    async fn drain_after_window(
        receiver: &mut broadcast::Receiver<DashboardEvent>,
    ) -> Vec<DashboardEvent> {
        tokio::time::sleep(METRICS_UPDATED_COALESCE_WINDOW + Duration::from_millis(1)).await;
        let mut events = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
        events
    }

    #[tokio::test(start_paused = true)]
    async fn test_publish_coalesced_emits_latest_update_once_per_window() {
        let bus = DashboardEventBus::new();
        let mut receiver = bus.subscribe();
        let id = Uuid::new_v4();

        bus.publish_coalesced(metrics_updated(id, 1.0));
        bus.publish_coalesced(metrics_updated(id, 2.0));
        bus.publish_coalesced(metrics_updated(id, 3.0));

        // 窓が閉じるまでは発行しない
        assert!(receiver.try_recv().is_err());

        let events = drain_after_window(&mut receiver).await;
        assert_eq!(events.len(), 1, "events: {events:?}");
        match &events[0] {
            DashboardEvent::MetricsUpdated {
                runtime_id,
                cpu_usage,
                ..
            } => {
                assert_eq!(*runtime_id, id);
                assert_eq!(*cpu_usage, Some(3.0));
            }
            other => panic!("Expected MetricsUpdated, got {other:?}"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn test_publish_coalesced_opens_new_window_after_flush() {
        let bus = DashboardEventBus::new();
        let mut receiver = bus.subscribe();
        let id = Uuid::new_v4();

        bus.publish_coalesced(metrics_updated(id, 1.0));
        assert_eq!(drain_after_window(&mut receiver).await.len(), 1);

        // 窓が閉じた後の更新は取りこぼさず、次の窓で発行する
        bus.publish_coalesced(metrics_updated(id, 2.0));
        assert!(receiver.try_recv().is_err());
        assert_eq!(drain_after_window(&mut receiver).await.len(), 1);

        // 更新が無ければ何も発行しない
        assert!(drain_after_window(&mut receiver).await.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn test_publish_coalesced_windows_are_independent_per_endpoint() {
        let bus = DashboardEventBus::new();
        let mut receiver = bus.subscribe();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        bus.publish_coalesced(metrics_updated(id1, 1.0));
        bus.publish_coalesced(metrics_updated(id2, 2.0));
        bus.publish_coalesced(metrics_updated(id1, 3.0));

        let mut received: Vec<(Uuid, Option<f32>)> = drain_after_window(&mut receiver)
            .await
            .into_iter()
            .map(|event| match event {
                DashboardEvent::MetricsUpdated {
                    runtime_id,
                    cpu_usage,
                    ..
                } => (runtime_id, cpu_usage),
                other => panic!("Expected MetricsUpdated, got {other:?}"),
            })
            .collect();
        received.sort_by_key(|(runtime_id, _)| *runtime_id);
        let mut expected = vec![(id1, Some(3.0)), (id2, Some(2.0))];
        expected.sort_by_key(|(runtime_id, _)| *runtime_id);
        assert_eq!(received, expected);
    }

    #[tokio::test(start_paused = true)]
    async fn test_publish_coalesced_bounds_sustained_burst_to_one_event_per_window() {
        let bus = DashboardEventBus::new();
        let mut receiver = bus.subscribe();
        let id = Uuid::new_v4();

        // 10 ミリ秒間隔で 5 秒間（500 回）更新し続ける
        for i in 0..500 {
            bus.publish_coalesced(metrics_updated(id, i as f32));
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        let events = drain_after_window(&mut receiver).await;
        assert_eq!(events.len(), 5, "500 updates over 5s must yield 5 events");
    }

    #[test]
    fn test_publish_coalesced_publishes_other_events_immediately() {
        let bus = DashboardEventBus::new();
        let mut receiver = bus.subscribe();

        bus.publish_coalesced(DashboardEvent::UpdateStateChanged);

        assert!(matches!(
            receiver.try_recv(),
            Ok(DashboardEvent::UpdateStateChanged)
        ));
    }

    // --- TpsUpdated edge cases ---

    #[test]
    fn test_tps_updated_zero_values_serialization() {
        let event = DashboardEvent::TpsUpdated {
            endpoint_id: Uuid::nil(),
            model_id: "".to_string(),
            tps: 0.0,
            output_tokens: 0,
            duration_ms: 0,
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "TpsUpdated");
        let data = &json["data"];
        assert_eq!(data["tps"], 0.0);
        assert_eq!(data["output_tokens"], 0);
        assert_eq!(data["duration_ms"], 0);
    }

    #[test]
    fn test_tps_updated_large_values_serialization() {
        let event = DashboardEvent::TpsUpdated {
            endpoint_id: Uuid::new_v4(),
            model_id: "large-model".to_string(),
            tps: 99999.99,
            output_tokens: u32::MAX,
            duration_ms: u64::MAX,
        };
        let json = serde_json::to_value(&event).unwrap();
        let data = &json["data"];
        assert_eq!(data["output_tokens"], u32::MAX);
    }

    // --- Event channel capacity ---

    #[test]
    fn test_event_channel_capacity_constant() {
        assert_eq!(EVENT_CHANNEL_CAPACITY, 1024);
    }
}
