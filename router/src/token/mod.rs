//! トークン抽出モジュール
//!
//! OpenAI互換レスポンスからトークン数を抽出し、
//! usageフィールドがない場合はtiktokenで推定する。

use serde_json::Value;
use tiktoken_rs::cl100k_base;

/// トークン使用量
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TokenUsage {
    /// 入力トークン数
    pub input_tokens: Option<u32>,
    /// 出力トークン数
    pub output_tokens: Option<u32>,
    /// 総トークン数
    pub total_tokens: Option<u32>,
}

impl TokenUsage {
    /// 新しいTokenUsageを作成
    pub fn new(input: Option<u32>, output: Option<u32>, total: Option<u32>) -> Self {
        Self {
            input_tokens: input,
            output_tokens: output,
            total_tokens: total,
        }
    }

    /// 空のTokenUsageかどうか
    pub fn is_empty(&self) -> bool {
        self.input_tokens.is_none() && self.output_tokens.is_none() && self.total_tokens.is_none()
    }
}

/// OpenAI互換レスポンスのusageフィールドからトークン数を抽出
///
/// # Arguments
/// * `response_body` - OpenAI互換APIレスポンスのJSON
///
/// # Returns
/// * `Some(TokenUsage)` - usageフィールドが存在する場合
/// * `None` - usageフィールドが存在しない場合
pub fn extract_usage_from_response(response_body: &Value) -> Option<TokenUsage> {
    let usage = response_body.get("usage")?;

    // usageオブジェクトが存在する場合は、各フィールドを抽出
    let input_tokens = usage
        .get("prompt_tokens")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    let output_tokens = usage
        .get("completion_tokens")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    let total_tokens = usage
        .get("total_tokens")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    Some(TokenUsage::new(input_tokens, output_tokens, total_tokens))
}

/// tiktokenを使用してテキストのトークン数を推定
///
/// # Arguments
/// * `text` - トークン数を推定するテキスト
/// * `_model` - モデル名（現在は未使用、将来的にモデル別エンコーディングに対応）
///
/// # Returns
/// * `Some(u32)` - 推定トークン数
/// * `None` - 推定できない場合
pub fn estimate_tokens(text: &str, _model: &str) -> Option<u32> {
    // cl100k_base エンコーディングを使用（GPT-4, GPT-3.5-turbo互換）
    // llama系モデルも概ね近い値になるため、フォールバックとして使用
    let bpe = cl100k_base().ok()?;
    let tokens = bpe.encode_with_special_tokens(text);
    Some(tokens.len() as u32)
}

/// トークン抽出（usageフィールド優先、フォールバックでtiktoken推定）
///
/// # Arguments
/// * `response_body` - OpenAI互換APIレスポンスのJSON
/// * `request_text` - リクエストテキスト（フォールバック用）
/// * `response_text` - レスポンステキスト（フォールバック用）
/// * `model` - モデル名
///
/// # Returns
/// * `TokenUsage` - 抽出または推定されたトークン使用量
pub fn extract_or_estimate_tokens(
    response_body: &Value,
    request_text: Option<&str>,
    response_text: Option<&str>,
    model: &str,
) -> TokenUsage {
    // まずusageフィールドから抽出を試みる
    if let Some(usage) = extract_usage_from_response(response_body) {
        return usage;
    }

    // usageがない場合はtiktokenで推定
    let input_tokens = request_text.and_then(|text| estimate_tokens(text, model));
    let output_tokens = response_text.and_then(|text| estimate_tokens(text, model));

    // total_tokensは入力と出力の合計
    let total_tokens = match (input_tokens, output_tokens) {
        (Some(i), Some(o)) => Some(i + o),
        (Some(i), None) => Some(i),
        (None, Some(o)) => Some(o),
        (None, None) => None,
    };

    TokenUsage::new(input_tokens, output_tokens, total_tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // T-3: usageフィールドからのトークン抽出テスト
    #[test]
    fn test_extract_usage_from_response_with_usage_field() {
        let response = json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "choices": [
                {
                    "message": {
                        "role": "assistant",
                        "content": "Hello!"
                    }
                }
            ],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        });

        let usage = extract_usage_from_response(&response);
        assert!(usage.is_some(), "usageフィールドがある場合はSomeを返すべき");
        let usage = usage.unwrap();
        assert_eq!(usage.input_tokens, Some(10));
        assert_eq!(usage.output_tokens, Some(5));
        assert_eq!(usage.total_tokens, Some(15));
    }

    #[test]
    fn test_extract_usage_from_response_without_usage_field() {
        let response = json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "choices": [
                {
                    "message": {
                        "role": "assistant",
                        "content": "Hello!"
                    }
                }
            ]
        });

        let usage = extract_usage_from_response(&response);
        assert!(usage.is_none(), "usageフィールドがない場合はNoneを返すべき");
    }

    #[test]
    fn test_extract_usage_from_response_partial_usage() {
        // prompt_tokensのみの場合
        let response = json!({
            "usage": {
                "prompt_tokens": 10
            }
        });

        let usage = extract_usage_from_response(&response);
        assert!(usage.is_some());
        let usage = usage.unwrap();
        assert_eq!(usage.input_tokens, Some(10));
        assert_eq!(usage.output_tokens, None);
        assert_eq!(usage.total_tokens, None);
    }

    // T-4: tiktoken推定テスト
    #[test]
    fn test_estimate_tokens_with_known_model() {
        let text = "Hello, world!";
        let model = "gpt-4";

        let tokens = estimate_tokens(text, model);
        assert!(
            tokens.is_some(),
            "既知のモデルではトークン数を推定できるべき"
        );
        // "Hello, world!" は通常4トークン程度
        let token_count = tokens.unwrap();
        assert!(
            token_count > 0 && token_count < 10,
            "トークン数は妥当な範囲内であるべき: {}",
            token_count
        );
    }

    #[test]
    fn test_estimate_tokens_with_llama_model() {
        let text = "こんにちは、世界！";
        let model = "llama-3.1-8b";

        let tokens = estimate_tokens(text, model);
        // llama系もcl100k_baseでフォールバック推定
        assert!(
            tokens.is_some(),
            "llama系モデルでもトークン推定が可能であるべき"
        );
    }

    #[test]
    fn test_estimate_tokens_empty_text() {
        let text = "";
        let model = "gpt-4";

        let tokens = estimate_tokens(text, model);
        assert!(tokens.is_some());
        assert_eq!(tokens.unwrap(), 0, "空文字は0トークン");
    }

    // T-5: usageフィールド欠如時のフォールバックテスト
    #[test]
    fn test_extract_or_estimate_with_usage_field() {
        let response = json!({
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 50,
                "total_tokens": 150
            }
        });

        let usage =
            extract_or_estimate_tokens(&response, Some("What is 2+2?"), Some("2+2=4"), "gpt-4");

        // usageフィールドがある場合はそれを使用
        assert_eq!(usage.input_tokens, Some(100));
        assert_eq!(usage.output_tokens, Some(50));
        assert_eq!(usage.total_tokens, Some(150));
    }

    #[test]
    fn test_extract_or_estimate_fallback_to_tiktoken() {
        let response = json!({
            "choices": [{"message": {"content": "The answer is 4."}}]
        });

        let usage = extract_or_estimate_tokens(
            &response,
            Some("What is 2+2?"),
            Some("The answer is 4."),
            "gpt-4",
        );

        // usageがない場合はtiktokenで推定
        assert!(usage.input_tokens.is_some(), "入力トークンが推定されるべき");
        assert!(
            usage.output_tokens.is_some(),
            "出力トークンが推定されるべき"
        );
        // total_tokensは入力+出力
        assert!(usage.total_tokens.is_some());
    }

    #[test]
    fn test_extract_or_estimate_no_text() {
        let response = json!({});

        let usage = extract_or_estimate_tokens(&response, None, None, "gpt-4");

        // テキストもusageもない場合は空
        assert!(usage.is_empty(), "情報がない場合は空のTokenUsageを返すべき");
    }

    #[test]
    fn test_token_usage_is_empty() {
        let empty = TokenUsage::default();
        assert!(empty.is_empty());

        let with_input = TokenUsage::new(Some(10), None, None);
        assert!(!with_input.is_empty());

        let with_output = TokenUsage::new(None, Some(5), None);
        assert!(!with_output.is_empty());

        let with_total = TokenUsage::new(None, None, Some(15));
        assert!(!with_total.is_empty());
    }
}
