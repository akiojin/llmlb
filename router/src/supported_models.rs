//! 対応モデル定義
//!
//! 動作確認済みモデルの静的定義
//! モデル定義はsupported_models.jsonから読み込まれる

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
    /// モデル形式（例: "gguf", "safetensors"）
    #[serde(default = "default_format")]
    pub format: String,
    /// 使用エンジン（例: "llama_cpp", "gptoss_cpp", "nemotron_cpp"）
    #[serde(default = "default_engine")]
    pub engine: String,
    /// 対応プラットフォーム（例: ["macos-metal", "windows-directml"]）
    #[serde(default)]
    pub platforms: Vec<String>,
}

fn default_format() -> String {
    "gguf".to_string()
}

fn default_engine() -> String {
    "llama_cpp".to_string()
}

/// JSONファイルからモデル定義を読み込む
const SUPPORTED_MODELS_JSON: &str = include_str!("supported_models.json");

/// 対応モデル一覧を取得
///
/// 動作確認済みモデルの静的リストを返す
/// モデル定義はsupported_models.jsonから読み込まれる
pub fn get_supported_models() -> Vec<SupportedModel> {
    serde_json::from_str(SUPPORTED_MODELS_JSON).expect("Failed to parse supported_models.json")
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
