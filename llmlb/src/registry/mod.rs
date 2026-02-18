//! エンドポイント登録管理
//!
//! SPEC-e8e9326e: llmlb主導エンドポイント登録システム
//!
//! エンドポイントの状態をメモリ内で管理し、SQLiteと同期

pub mod endpoints;
pub mod models;

pub use endpoints::EndpointRegistry;
