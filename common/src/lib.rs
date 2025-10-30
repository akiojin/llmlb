//! Ollama Coordinator Common Library
//!
//! 共通型定義、プロトコル、設定、エラー型を提供

#![warn(missing_docs)]

/// 共通型定義
pub mod types {
    //! Agent, HealthMetrics, Request等の型定義
    // TODO: T020で実装
}

/// 通信プロトコル定義
pub mod protocol {
    //! RegisterRequest, HealthCheckRequest等のプロトコル定義
    // TODO: T021で実装
}

/// 設定管理
pub mod config {
    //! CoordinatorConfig, AgentConfig等の設定構造体
    // TODO: T022で実装
}

/// エラー型定義
pub mod error {
    //! 統一エラー型定義
    // TODO: T023で実装
}
