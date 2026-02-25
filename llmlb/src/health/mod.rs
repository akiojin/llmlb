//! ヘルスチェックモニター
//!
//! 定期的にエンドポイントの稼働状況を監視
//!
//! # SPEC-e8e9326e: llmlb主導エンドポイント登録システム
//!
//! PULL型ヘルスチェックを提供する。llmlbは各エンドポイントの健康状態を確認する
//! （xLLMのみ `/api/health` を優先利用し、非xLLMは `/v1/models` を用いてヘルスチェックする）。

pub mod endpoint_checker;

pub use endpoint_checker::EndpointHealthChecker;
