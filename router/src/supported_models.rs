//! 対応モデル定義
//!
//! 動作確認済みモデルの静的定義

use serde::{Deserialize, Serialize};

/// 対応モデル定義
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SupportedModel {
    /// 一意識別子（例: "qwen2.5-7b-instruct"）
    pub id: String,
    /// 表示名
    pub name: String,
    /// 説明
    pub description: String,
    /// HuggingFaceリポジトリ（例: "bartowski/Qwen2.5-7B-Instruct-GGUF"）
    pub repo: String,
    /// 推奨ファイル名（例: "Qwen2.5-7B-Instruct-Q4_K_M.gguf"）
    pub recommended_filename: String,
    /// ファイルサイズ（バイト）
    pub size_bytes: u64,
    /// 必要メモリ（バイト）
    pub required_memory_bytes: u64,
    /// タグ（例: ["chat", "coding", "multilingual"]）
    pub tags: Vec<String>,
    /// 能力（例: ["TextGeneration", "Vision"]）
    pub capabilities: Vec<String>,
    /// 量子化タイプ（例: "Q4_K_M"）
    pub quantization: Option<String>,
    /// 元のパラメータ数（表示用、例: "7B"）
    pub parameter_count: Option<String>,
}

/// 対応モデル一覧を取得
///
/// 動作確認済みモデルの静的リストを返す
pub fn get_supported_models() -> Vec<SupportedModel> {
    vec![
        SupportedModel {
            id: "qwen2.5-7b-instruct".into(),
            name: "Qwen2.5 7B Instruct".into(),
            description:
                "Alibaba's multilingual instruction-tuned model with strong reasoning capabilities"
                    .into(),
            repo: "bartowski/Qwen2.5-7B-Instruct-GGUF".into(),
            recommended_filename: "Qwen2.5-7B-Instruct-Q4_K_M.gguf".into(),
            size_bytes: 4_920_000_000,
            required_memory_bytes: 7_380_000_000,
            tags: vec!["chat".into(), "multilingual".into(), "coding".into()],
            capabilities: vec!["TextGeneration".into()],
            quantization: Some("Q4_K_M".into()),
            parameter_count: Some("7B".into()),
        },
        SupportedModel {
            id: "llama3.2-3b-instruct".into(),
            name: "Llama 3.2 3B Instruct".into(),
            description: "Meta's lightweight instruction-tuned model optimized for edge deployment"
                .into(),
            repo: "bartowski/Llama-3.2-3B-Instruct-GGUF".into(),
            recommended_filename: "Llama-3.2-3B-Instruct-Q4_K_M.gguf".into(),
            size_bytes: 2_020_000_000,
            required_memory_bytes: 3_030_000_000,
            tags: vec!["chat".into(), "lightweight".into()],
            capabilities: vec!["TextGeneration".into()],
            quantization: Some("Q4_K_M".into()),
            parameter_count: Some("3B".into()),
        },
        SupportedModel {
            id: "mistral-7b-instruct".into(),
            name: "Mistral 7B Instruct".into(),
            description:
                "Mistral AI's efficient instruction-tuned model with sliding window attention"
                    .into(),
            repo: "bartowski/Mistral-7B-Instruct-v0.3-GGUF".into(),
            recommended_filename: "Mistral-7B-Instruct-v0.3-Q4_K_M.gguf".into(),
            size_bytes: 4_370_000_000,
            required_memory_bytes: 6_555_000_000,
            tags: vec!["chat".into(), "efficient".into()],
            capabilities: vec!["TextGeneration".into()],
            quantization: Some("Q4_K_M".into()),
            parameter_count: Some("7B".into()),
        },
        SupportedModel {
            id: "phi-3-mini".into(),
            name: "Phi-3 Mini".into(),
            description: "Microsoft's compact yet powerful model for reasoning and coding".into(),
            repo: "bartowski/Phi-3-mini-4k-instruct-GGUF".into(),
            recommended_filename: "Phi-3-mini-4k-instruct-Q4_K_M.gguf".into(),
            size_bytes: 2_390_000_000,
            required_memory_bytes: 3_585_000_000,
            tags: vec!["chat".into(), "coding".into(), "compact".into()],
            capabilities: vec!["TextGeneration".into()],
            quantization: Some("Q4_K_M".into()),
            parameter_count: Some("3.8B".into()),
        },
        SupportedModel {
            id: "gemma-2-9b".into(),
            name: "Gemma 2 9B".into(),
            description: "Google's open model with strong performance across diverse tasks".into(),
            repo: "bartowski/gemma-2-9b-it-GGUF".into(),
            recommended_filename: "gemma-2-9b-it-Q4_K_M.gguf".into(),
            size_bytes: 5_760_000_000,
            required_memory_bytes: 8_640_000_000,
            tags: vec!["chat".into(), "multilingual".into()],
            capabilities: vec!["TextGeneration".into()],
            quantization: Some("Q4_K_M".into()),
            parameter_count: Some("9B".into()),
        },
    ]
}

/// IDで対応モデルを検索
pub fn find_supported_model(id: &str) -> Option<SupportedModel> {
    get_supported_models().into_iter().find(|m| m.id == id)
}

/// 対応モデルかどうかを確認
pub fn is_supported_model(id: &str) -> bool {
    find_supported_model(id).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_supported_models_returns_non_empty_list() {
        let models = get_supported_models();
        assert!(!models.is_empty(), "対応モデルリストは空であってはならない");
    }

    #[test]
    fn test_supported_models_have_required_fields() {
        let models = get_supported_models();
        for model in &models {
            assert!(!model.id.is_empty(), "モデルIDは必須");
            assert!(!model.name.is_empty(), "モデル名は必須");
            assert!(!model.repo.is_empty(), "HFリポジトリは必須");
            assert!(
                !model.recommended_filename.is_empty(),
                "推奨ファイル名は必須"
            );
            assert!(model.size_bytes > 0, "ファイルサイズは0より大きい");
            assert!(model.required_memory_bytes > 0, "必要メモリは0より大きい");
        }
    }

    #[test]
    fn test_supported_models_have_unique_ids() {
        let models = get_supported_models();
        let mut ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        let original_len = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), original_len, "モデルIDは一意でなければならない");
    }

    #[test]
    fn test_find_supported_model_found() {
        let model = find_supported_model("qwen2.5-7b-instruct");
        assert!(model.is_some());
        let model = model.unwrap();
        assert_eq!(model.name, "Qwen2.5 7B Instruct");
    }

    #[test]
    fn test_find_supported_model_not_found() {
        let model = find_supported_model("non-existent-model");
        assert!(model.is_none());
    }

    #[test]
    fn test_is_supported_model() {
        assert!(is_supported_model("qwen2.5-7b-instruct"));
        assert!(is_supported_model("llama3.2-3b-instruct"));
        assert!(!is_supported_model("non-existent-model"));
    }

    #[test]
    fn test_supported_model_serialization() {
        let models = get_supported_models();
        let json = serde_json::to_string(&models).expect("シリアライズに失敗");
        let deserialized: Vec<SupportedModel> =
            serde_json::from_str(&json).expect("デシリアライズに失敗");
        assert_eq!(models, deserialized);
    }
}
