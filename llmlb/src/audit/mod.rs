//! 監査ログシステム (SPEC-8301d106)
//!
//! 全HTTP操作のメタデータを自動記録し、改ざん防止チェーンで保護する

/// 監査ログの型定義
pub mod types;

/// 非同期バッファライター
pub mod writer;

/// 監査ログミドルウェア
pub mod middleware;

/// SHA-256バッチハッシュチェーン（改ざん検知）
pub mod hash_chain;
