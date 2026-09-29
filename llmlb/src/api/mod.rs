//! REST APIハンドラー
//!
//! ノード登録、ヘルスチェック、プロキシAPI

pub mod anthropic;
pub mod api_keys;
pub mod audio;
/// 監査ログAPI (SPEC-8301d106)
pub mod audit_log;
pub mod auth;
pub mod benchmarks;
/// カタログ検索API（HuggingFaceラッパー）
pub mod catalog;
pub mod cloud_models;
/// クラウドプロバイダプロキシ（CloudProvider trait）
pub mod cloud_proxy;
pub mod dashboard;
pub mod dashboard_ws;
/// エンドポイント管理API
pub mod endpoints;
/// APIエラーレスポンス型
pub mod error;
pub mod health;
pub mod images;
pub mod invitations;
pub mod logs;
/// モデル名のパース（量子化サフィックス対応）
pub mod model_name;
pub mod models;
pub mod openai;
/// OpenAI互換APIユーティリティ
pub mod openai_util;
pub mod proxy;
/// Open Responses API (SPEC-0f1de549)
pub mod responses;
/// System API (self-update)
pub mod system;
pub mod users;

/// ルーティング（ドメイン別 Router, FR-012）
mod router;

pub use router::create_app;
