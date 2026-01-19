//! モデル一覧レスポンスパーサー
//!
//! OpenAI形式とOllama形式の両方をパース

use serde::Deserialize;

/// パースされたモデル情報
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedModel {
    /// モデルID/名前
    pub id: String,
}

/// OpenAI形式のモデルレスポンス
/// `{"data": [{"id": "model-name", "object": "model", ...}]}`
#[derive(Debug, Deserialize)]
pub struct OpenAiModelsResponse {
    /// モデルのリスト
    pub data: Vec<OpenAiModel>,
}

/// OpenAI形式の個別モデル
#[derive(Debug, Deserialize)]
pub struct OpenAiModel {
    /// モデルID
    pub id: String,
}

/// Ollama形式のモデルレスポンス
/// `{"models": [{"name": "llama3:latest", "model": "llama3:latest", ...}]}`
#[derive(Debug, Deserialize)]
pub struct OllamaModelsResponse {
    /// モデルのリスト
    pub models: Vec<OllamaModel>,
}

/// Ollama形式の個別モデル
#[derive(Debug, Deserialize)]
pub struct OllamaModel {
    /// モデル名
    pub name: Option<String>,
    /// モデル識別子（nameがない場合のフォールバック）
    pub model: Option<String>,
}

/// レスポンスの形式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseFormat {
    /// OpenAI形式: `{"data": [...]}`
    OpenAi,
    /// Ollama形式: `{"models": [...]}`
    Ollama,
    /// 不明な形式
    Unknown,
}

/// JSONレスポンスをパースしてモデル一覧を抽出
///
/// OpenAI形式とOllama形式の両方に対応
///
/// # Examples
///
/// ```
/// use llm_router::sync::parser::parse_models_response;
///
/// // OpenAI形式
/// let json = r#"{"data": [{"id": "gpt-4"}, {"id": "gpt-3.5-turbo"}]}"#;
/// let value: serde_json::Value = serde_json::from_str(json).unwrap();
/// let (models, format) = parse_models_response(&value);
/// assert_eq!(models.len(), 2);
///
/// // Ollama形式
/// let json = r#"{"models": [{"name": "llama3"}, {"model": "mistral"}]}"#;
/// let value: serde_json::Value = serde_json::from_str(json).unwrap();
/// let (models, format) = parse_models_response(&value);
/// assert_eq!(models.len(), 2);
/// ```
pub fn parse_models_response(json: &serde_json::Value) -> (Vec<ParsedModel>, ResponseFormat) {
    // OpenAI形式を試す
    if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
        let models = data
            .iter()
            .filter_map(|model| model.get("id").and_then(|id| id.as_str()))
            .map(|id| ParsedModel { id: id.to_string() })
            .collect();
        return (models, ResponseFormat::OpenAi);
    }

    // Ollama形式を試す
    if let Some(models_array) = json.get("models").and_then(|m| m.as_array()) {
        let models = models_array
            .iter()
            .filter_map(|model| {
                // nameを優先、なければmodelを使用
                let id = model
                    .get("name")
                    .and_then(|n| n.as_str())
                    .or_else(|| model.get("model").and_then(|m| m.as_str()));
                id.filter(|s| !s.is_empty())
            })
            .map(|id| ParsedModel { id: id.to_string() })
            .collect();
        return (models, ResponseFormat::Ollama);
    }

    // どちらにも該当しない
    (Vec::new(), ResponseFormat::Unknown)
}

/// レスポンス形式を検出
pub fn detect_format(json: &serde_json::Value) -> ResponseFormat {
    if json.get("data").and_then(|d| d.as_array()).is_some() {
        ResponseFormat::OpenAi
    } else if json.get("models").and_then(|m| m.as_array()).is_some() {
        ResponseFormat::Ollama
    } else {
        ResponseFormat::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_openai_format() {
        let json = json!({
            "object": "list",
            "data": [
                {"id": "gpt-4", "object": "model", "created": 1234567890},
                {"id": "gpt-3.5-turbo", "object": "model", "created": 1234567890}
            ]
        });

        let (models, format) = parse_models_response(&json);
        assert_eq!(format, ResponseFormat::OpenAi);
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "gpt-4");
        assert_eq!(models[1].id, "gpt-3.5-turbo");
    }

    #[test]
    fn test_parse_ollama_format() {
        let json = json!({
            "models": [
                {"name": "llama3:latest", "model": "llama3:latest", "size": 123456},
                {"name": "mistral:7b", "model": "mistral:7b", "size": 789012}
            ]
        });

        let (models, format) = parse_models_response(&json);
        assert_eq!(format, ResponseFormat::Ollama);
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "llama3:latest");
        assert_eq!(models[1].id, "mistral:7b");
    }

    #[test]
    fn test_parse_ollama_model_field_fallback() {
        // nameがなくmodelのみの場合
        let json = json!({
            "models": [
                {"model": "codellama:7b"}
            ]
        });

        let (models, format) = parse_models_response(&json);
        assert_eq!(format, ResponseFormat::Ollama);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "codellama:7b");
    }

    #[test]
    fn test_parse_empty_data() {
        let json = json!({
            "data": []
        });

        let (models, format) = parse_models_response(&json);
        assert_eq!(format, ResponseFormat::OpenAi);
        assert!(models.is_empty());
    }

    #[test]
    fn test_parse_empty_models() {
        let json = json!({
            "models": []
        });

        let (models, format) = parse_models_response(&json);
        assert_eq!(format, ResponseFormat::Ollama);
        assert!(models.is_empty());
    }

    #[test]
    fn test_parse_unknown_format() {
        let json = json!({
            "results": []
        });

        let (models, format) = parse_models_response(&json);
        assert_eq!(format, ResponseFormat::Unknown);
        assert!(models.is_empty());
    }

    #[test]
    fn test_parse_skips_empty_ids() {
        let json = json!({
            "models": [
                {"name": "llama3"},
                {"name": ""},
                {"model": ""}
            ]
        });

        let (models, format) = parse_models_response(&json);
        assert_eq!(format, ResponseFormat::Ollama);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "llama3");
    }

    #[test]
    fn test_detect_format_openai() {
        let json = json!({"data": []});
        assert_eq!(detect_format(&json), ResponseFormat::OpenAi);
    }

    #[test]
    fn test_detect_format_ollama() {
        let json = json!({"models": []});
        assert_eq!(detect_format(&json), ResponseFormat::Ollama);
    }

    #[test]
    fn test_detect_format_unknown() {
        let json = json!({"other": []});
        assert_eq!(detect_format(&json), ResponseFormat::Unknown);
    }
}
