//! モデル情報管理
//!
//! LLM runtimeモデルのメタデータ管理

use chrono::{DateTime, Utc};
use llm_router_common::types::ModelCapability;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// モデルのソース種別
#[derive(Default)]
pub enum ModelSource {
    /// 事前定義モデル
    #[default]
    Predefined,
    /// HFのGGUFモデル
    HfGguf,
    /// HFのsafetensorsモデル
    HfSafetensors,
    /// HFのONNXモデル（Whisper等）
    HfOnnx,
}

/// LLM runtimeモデル情報
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelInfo {
    /// モデル名（例: "gpt-oss-20b", "llama3.2"）
    pub name: String,
    /// モデルサイズ（バイト）
    pub size: u64,
    /// モデルの説明
    pub description: String,
    /// 必要なGPUメモリ（バイト）
    pub required_memory: u64,
    /// タグ（例: ["vision", "tools", "thinking"]）
    pub tags: Vec<String>,
    /// モデルの能力（対応するAPI）
    /// 未設定の場合はModelType::Llm（テキスト生成）として扱う
    #[serde(default)]
    pub capabilities: Vec<ModelCapability>,
    /// ソース種別
    #[serde(default)]
    pub source: ModelSource,
    /// 外部から提供されるchat_template（GGUFに含まれない場合の補助）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_template: Option<String>,
    /// HFリポジトリ名
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// HFファイル名
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    /// 最終更新
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<DateTime<Utc>>,
    /// ステータス（available/registered等）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

impl ModelInfo {
    /// 新しいModelInfoを作成
    ///
    /// capabilities が空の場合は、デフォルトで TextGeneration を設定
    pub fn new(
        name: String,
        size: u64,
        description: String,
        required_memory: u64,
        tags: Vec<String>,
    ) -> Self {
        Self {
            name,
            size,
            description,
            required_memory,
            tags,
            // デフォルトは TextGeneration（LLMモデル）
            capabilities: vec![ModelCapability::TextGeneration],
            source: ModelSource::Predefined,
            chat_template: None,
            repo: None,
            filename: None,
            last_modified: None,
            status: None,
        }
    }

    /// 指定した capabilities で新しい ModelInfo を作成
    pub fn with_capabilities(
        name: String,
        size: u64,
        description: String,
        required_memory: u64,
        tags: Vec<String>,
        capabilities: Vec<ModelCapability>,
    ) -> Self {
        Self {
            name,
            size,
            description,
            required_memory,
            tags,
            capabilities,
            source: ModelSource::Predefined,
            chat_template: None,
            repo: None,
            filename: None,
            last_modified: None,
            status: None,
        }
    }

    /// 必要メモリをMB単位で取得
    pub fn required_memory_mb(&self) -> u64 {
        self.required_memory / (1024 * 1024)
    }

    /// 必要メモリをGB単位で取得
    pub fn required_memory_gb(&self) -> f64 {
        self.required_memory as f64 / (1024.0 * 1024.0 * 1024.0)
    }

    /// モデルが指定した capability をサポートしているか確認
    ///
    /// capabilities が空の場合は TextGeneration をサポートしているとみなす（後方互換性）
    pub fn has_capability(&self, capability: ModelCapability) -> bool {
        if self.capabilities.is_empty() {
            // 後方互換性: capabilities 未設定のモデルは TextGeneration のみサポート
            capability == ModelCapability::TextGeneration
        } else {
            self.capabilities.contains(&capability)
        }
    }

    /// モデルの capabilities を取得
    ///
    /// capabilities が空の場合は TextGeneration のみを返す（後方互換性）
    pub fn get_capabilities(&self) -> Vec<ModelCapability> {
        if self.capabilities.is_empty() {
            vec![ModelCapability::TextGeneration]
        } else {
            self.capabilities.clone()
        }
    }
}

/// HuggingFace URLからrepo_idを抽出
///
/// 入力例:
/// - "https://huggingface.co/openai/gpt-oss-20b" → "openai/gpt-oss-20b"
/// - "http://huggingface.co/openai/gpt-oss-20b" → "openai/gpt-oss-20b"
/// - "openai/gpt-oss-20b" → "openai/gpt-oss-20b" (そのまま)
/// - "gpt-oss-20b" → "gpt-oss-20b" (そのまま)
///
/// 備考:
/// - huggingface_hubのsnapshot_downloadはrepo_id形式（namespace/repo_name）を期待する
/// - フルURLが渡された場合はrepo_id部分のみを抽出して返す
pub fn extract_repo_id(input: &str) -> String {
    // HuggingFace URLパターンを検出
    let hf_patterns = [
        "https://huggingface.co/",
        "http://huggingface.co/",
        "https://www.huggingface.co/",
        "http://www.huggingface.co/",
    ];

    for pattern in hf_patterns {
        if let Some(rest) = input.strip_prefix(pattern) {
            // URLの残り部分からrepo_idを抽出
            // "openai/gpt-oss-20b/tree/main" → "openai/gpt-oss-20b"
            let parts: Vec<&str> = rest.split('/').collect();
            if parts.len() >= 2 {
                // namespace/repo_name を返す
                return format!("{}/{}", parts[0], parts[1]);
            } else if parts.len() == 1 && !parts[0].is_empty() {
                return parts[0].to_string();
            }
        }
    }

    // HF_BASE_URL環境変数が設定されている場合、そのURLも考慮
    if let Ok(base_url) = std::env::var("HF_BASE_URL") {
        let base_url = base_url.trim_end_matches('/');
        let patterns = [
            format!("{}/", base_url),
            format!("{}//", base_url.replace("https://", "http://")),
        ];
        for pattern in patterns {
            if let Some(rest) = input.strip_prefix(&pattern) {
                let parts: Vec<&str> = rest.split('/').collect();
                if parts.len() >= 2 {
                    return format!("{}/{}", parts[0], parts[1]);
                } else if parts.len() == 1 && !parts[0].is_empty() {
                    return parts[0].to_string();
                }
            }
        }
    }

    // URLパターンに一致しない場合はそのまま返す
    input.to_string()
}

/// HuggingFaceリポジトリ名からモデルIDを生成（階層形式）
///
/// SPEC-dcaeaec4 FR-2に準拠:
/// - `openai/gpt-oss-20b` → `openai/gpt-oss-20b`
/// - `TheBloke/Llama-2-7B-GGUF` → `thebloke/llama-2-7b-gguf`
///
/// 正規化ルール:
/// 1. 小文字に変換
/// 2. 先頭・末尾のスラッシュを除去
/// 3. 危険なパターン (`..`, `\0`) は "_latest" に変換
pub fn generate_model_id(repo: &str) -> String {
    if repo.is_empty() {
        return "_latest".into();
    }

    // 危険なパターンをチェック
    if repo.contains("..") || repo.contains('\0') {
        return "_latest".into();
    }

    // 小文字に変換し、先頭・末尾のスラッシュを除去
    let normalized = repo.to_lowercase();
    let trimmed = normalized.trim_matches('/');

    if trimmed.is_empty() {
        "_latest".into()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== モデルID生成テスト（階層形式） =====

    #[test]
    fn test_generate_model_id_hierarchical() {
        // 階層形式: org/model → org/model (小文字化)
        assert_eq!(
            generate_model_id("TheBloke/Llama-2-7B-GGUF"),
            "thebloke/llama-2-7b-gguf"
        );
    }

    #[test]
    fn test_generate_model_id_with_org() {
        // 組織名付き
        assert_eq!(
            generate_model_id("bartowski/gemma-2-9b-it-GGUF"),
            "bartowski/gemma-2-9b-it-gguf"
        );
    }

    #[test]
    fn test_generate_model_id_simple() {
        // シンプルなリポジトリ名
        assert_eq!(
            generate_model_id("openai/gpt-oss-20b"),
            "openai/gpt-oss-20b"
        );
    }

    #[test]
    fn test_generate_model_id_single_name() {
        // 単一名（組織なし）
        assert_eq!(generate_model_id("convertible-repo"), "convertible-repo");
    }

    #[test]
    fn test_generate_model_id_uppercase() {
        // 大文字を含む
        assert_eq!(
            generate_model_id("MistralAI/Mistral-7B-Instruct-v0.2-GGUF"),
            "mistralai/mistral-7b-instruct-v0.2-gguf"
        );
    }

    #[test]
    fn test_generate_model_id_empty() {
        // 空文字列
        assert_eq!(generate_model_id(""), "_latest");
    }

    #[test]
    fn test_generate_model_id_dangerous() {
        // 危険なパターン
        assert_eq!(generate_model_id("../etc/passwd"), "_latest");
        assert_eq!(generate_model_id("model/../other"), "_latest");
    }

    #[test]
    fn test_generate_model_id_trim_slashes() {
        // 先頭・末尾のスラッシュを除去
        assert_eq!(generate_model_id("/openai/gpt-oss/"), "openai/gpt-oss");
    }

    // ===== 既存テスト =====

    #[test]
    fn test_model_info_new() {
        let model = ModelInfo::new(
            "gpt-oss-20b".to_string(),
            10_000_000_000,
            "GPT-OSS 20B model".to_string(),
            16_000_000_000,
            vec!["llm".to_string(), "text".to_string()],
        );

        assert_eq!(model.name, "gpt-oss-20b");
        assert_eq!(model.size, 10_000_000_000);
        assert_eq!(model.required_memory_gb(), 14.901161193847656);
        // デフォルトは TextGeneration
        assert_eq!(model.capabilities, vec![ModelCapability::TextGeneration]);
    }

    // ===== ModelInfo capabilities テスト =====

    #[test]
    fn test_model_info_with_capabilities() {
        let caps = vec![ModelCapability::TextGeneration, ModelCapability::Vision];
        let model = ModelInfo::with_capabilities(
            "gpt-4o".to_string(),
            0,
            "GPT-4o".to_string(),
            0,
            vec![],
            caps.clone(),
        );

        assert_eq!(model.capabilities, caps);
        assert!(model.has_capability(ModelCapability::TextGeneration));
        assert!(model.has_capability(ModelCapability::Vision));
        assert!(!model.has_capability(ModelCapability::TextToSpeech));
    }

    #[test]
    fn test_model_info_has_capability() {
        let model = ModelInfo::with_capabilities(
            "tts-model".to_string(),
            0,
            "TTS Model".to_string(),
            0,
            vec![],
            vec![ModelCapability::TextToSpeech],
        );

        assert!(model.has_capability(ModelCapability::TextToSpeech));
        assert!(!model.has_capability(ModelCapability::TextGeneration));
        assert!(!model.has_capability(ModelCapability::SpeechToText));
    }

    #[test]
    fn test_model_info_has_capability_backward_compat() {
        // capabilities が空の場合は TextGeneration をサポート（後方互換性）
        let mut model = ModelInfo::new(
            "legacy-model".to_string(),
            0,
            "Legacy".to_string(),
            0,
            vec![],
        );
        // 明示的に空にする
        model.capabilities = vec![];

        assert!(model.has_capability(ModelCapability::TextGeneration));
        assert!(!model.has_capability(ModelCapability::TextToSpeech));
    }

    #[test]
    fn test_model_info_get_capabilities() {
        let model = ModelInfo::with_capabilities(
            "multi-model".to_string(),
            0,
            "Multi".to_string(),
            0,
            vec![],
            vec![
                ModelCapability::TextGeneration,
                ModelCapability::Vision,
                ModelCapability::TextToSpeech,
            ],
        );

        let caps = model.get_capabilities();
        assert_eq!(caps.len(), 3);
        assert!(caps.contains(&ModelCapability::TextGeneration));
        assert!(caps.contains(&ModelCapability::Vision));
        assert!(caps.contains(&ModelCapability::TextToSpeech));
    }

    #[test]
    fn test_model_info_get_capabilities_backward_compat() {
        // capabilities が空の場合は TextGeneration のみを返す（後方互換性）
        let mut model = ModelInfo::new(
            "legacy-model".to_string(),
            0,
            "Legacy".to_string(),
            0,
            vec![],
        );
        model.capabilities = vec![];

        let caps = model.get_capabilities();
        assert_eq!(caps, vec![ModelCapability::TextGeneration]);
    }

    // ===== extract_repo_id テスト =====

    #[test]
    fn test_extract_repo_id_https_url() {
        assert_eq!(
            extract_repo_id("https://huggingface.co/openai/gpt-oss-20b"),
            "openai/gpt-oss-20b"
        );
    }

    #[test]
    fn test_extract_repo_id_http_url() {
        assert_eq!(
            extract_repo_id("http://huggingface.co/openai/gpt-oss-20b"),
            "openai/gpt-oss-20b"
        );
    }

    #[test]
    fn test_extract_repo_id_with_tree_path() {
        // URLにtree/mainなどが含まれている場合もrepo_idのみを抽出
        assert_eq!(
            extract_repo_id("https://huggingface.co/openai/gpt-oss-20b/tree/main"),
            "openai/gpt-oss-20b"
        );
    }

    #[test]
    fn test_extract_repo_id_www_prefix() {
        assert_eq!(
            extract_repo_id("https://www.huggingface.co/openai/gpt-oss-20b"),
            "openai/gpt-oss-20b"
        );
    }

    #[test]
    fn test_extract_repo_id_already_repo_format() {
        // 既にrepo_id形式の場合はそのまま返す
        assert_eq!(extract_repo_id("openai/gpt-oss-20b"), "openai/gpt-oss-20b");
    }

    #[test]
    fn test_extract_repo_id_simple_name() {
        // シンプルな名前の場合はそのまま返す
        assert_eq!(extract_repo_id("gpt-oss-20b"), "gpt-oss-20b");
    }
}
