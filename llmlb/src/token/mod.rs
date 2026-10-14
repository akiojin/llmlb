//! トークン抽出モジュール
//!
//! OpenAI互換レスポンスからトークン数を抽出し、
//! usageフィールドがない場合はtiktokenで推定する。

mod usage;

pub use usage::{
    estimate_tokens, extract_or_estimate_tokens, extract_usage_from_response,
    StreamingTokenAccumulator, TokenUsage,
};
