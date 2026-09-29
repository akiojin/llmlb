//! ダッシュボードイベントバス
//!
//! エンドポイント登録・状態変化・メトリクス更新などのイベントを
//! WebSocketクライアントにブロードキャストするための基盤

mod bus;

pub use bus::{create_shared_event_bus, DashboardEvent, DashboardEventBus, SharedEventBus};
