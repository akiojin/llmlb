//! 型定義モジュール
//!
//! ドメインエンティティの型定義を提供

/// エンドポイント関連の型定義
pub mod endpoint;

pub use endpoint::{Endpoint, EndpointHealthCheck, EndpointModel, EndpointStatus, SupportedAPI};
