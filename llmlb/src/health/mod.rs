//! ヘルスチェックモニター
//!
//! 定期的にエンドポイントの稼働状況を監視
//!
//! # SPEC-66555000: llmlb主導エンドポイント登録システム
//!
//! PULL型ヘルスチェックを提供する。llmlbが各エンドポイントの
//! `GET /api/health` を定期的にポーリングして状態を確認する。

pub mod endpoint_checker;
pub mod startup;

pub use endpoint_checker::EndpointHealthChecker;
pub use startup::run_startup_health_check;
