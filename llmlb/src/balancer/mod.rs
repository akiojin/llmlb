//! ロードバランサーモジュール
//!
//! エンドポイントのメトリクスとリクエスト統計を集約し、
//! TPS優先のロードバランシングを提供する。
//!
//! # EndpointRegistry統合
//!
//! このモジュールはEndpointRegistryを使用してエンドポイント情報を管理します。
//! 負荷分散はTPS優先、同一TPS時はラウンドロビンで行われます。

pub mod lease;
pub mod types;

// Re-export all public types for backward compatibility
pub use lease::RequestLease;
pub use types::{
    EndpointLoadSnapshot, EndpointTpsSummary, MetricsUpdate, ModelTpsInfo, ModelTpsState,
    RequestHistoryPoint, RequestOutcome, SystemSummary, WaitResult,
};

mod manager;

pub use manager::LoadManager;
