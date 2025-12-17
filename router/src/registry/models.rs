//! モデル情報管理
//!
//! LLM runtimeモデルのメタデータ管理

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

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
    /// HF非GGUFで変換待ち
    HfPendingConversion,
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
    /// ソース種別
    #[serde(default)]
    pub source: ModelSource,
    /// ダウンロードURL（HFなど外部ソース用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    /// 共有ストレージ上のモデルパス（存在する場合のみ）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
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
            source: ModelSource::Predefined,
            download_url: None,
            path: None,
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
}

/// モデル名をディレクトリパスに変換
///
/// SPEC-dcaeaec4 FR-2: 階層形式を許可
/// - `gpt-oss-20b` → `gpt-oss-20b`
/// - `openai/gpt-oss-20b` → `openai/gpt-oss-20b`（ネストディレクトリ）
///
/// `/` はディレクトリセパレータとして保持し、危険なパターンは除去。
pub fn model_name_to_dir(name: &str) -> String {
    if name.is_empty() {
        return "_latest".into();
    }

    // 危険なパターンを除去
    if name.contains("..") || name.contains('\0') {
        return "_latest".into();
    }

    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_lowercase()
            || c.is_ascii_digit()
            || c == '-'
            || c == '_'
            || c == '.'
            || c == '/'
        {
            out.push(c);
        } else if c.is_ascii_uppercase() {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }

    // 先頭・末尾のスラッシュを除去
    let out = out.trim_matches('/').to_string();

    if out.is_empty() || out == "." || out == ".." {
        "_latest".into()
    } else {
        out
    }
}

/// ルーター側のデフォルトモデルディレクトリ（~/.llm-router/models）
pub fn router_models_dir() -> Option<PathBuf> {
    let home = env::var("HOME").or_else(|_| env::var("USERPROFILE")).ok()?;
    Some(PathBuf::from(home).join(".llm-router").join("models"))
}

/// モデルのggufパスを返す（存在しない場合はNone）
pub fn router_model_path(name: &str) -> Option<PathBuf> {
    let base = router_models_dir()?;
    let path = base.join(model_name_to_dir(name)).join("model.gguf");
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

/// ルーター側にモデルをキャッシュする（ベストエフォート）。
/// - 既に存在すればそのパスを返す。
/// - download_url がある場合のみダウンロードを試行。
/// - 失敗しても None を返し、呼び出し側で download_url を利用できるようにする。
pub async fn ensure_router_model_cached(model: &ModelInfo) -> Option<PathBuf> {
    if let Some(existing) = router_model_path(&model.name) {
        return Some(existing);
    }

    let url = match &model.download_url {
        Some(u) if !u.is_empty() => u.clone(),
        _ => return None,
    };

    let base = match router_models_dir() {
        Some(p) => p,
        None => return None,
    };

    let dir = base.join(model_name_to_dir(&model.name));
    let target = dir.join("model.gguf");

    if let Err(e) = tokio::fs::create_dir_all(&dir).await {
        tracing::warn!(dir=?dir, err=?e, "cache_model:create_dir_failed");
        return None;
    }

    // 簡易ダウンロード（大容量でもストリーミングで書き込み）
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(err=?e, "cache_model:client_build_failed");
            return None;
        }
    };

    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(url=&url, err=?e, "cache_model:request_failed");
            return None;
        }
    };

    if !resp.status().is_success() {
        tracing::warn!(url=&url, status=?resp.status(), "cache_model:bad_status");
        return None;
    }

    let mut file = match tokio::fs::File::create(&target).await {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!(path=?target, err=?e, "cache_model:file_create_failed");
            return None;
        }
    };

    let mut stream = resp.bytes_stream();
    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(bytes) => {
                if let Err(e) = file.write_all(&bytes).await {
                    tracing::warn!(path=?target, err=?e, "cache_model:write_failed");
                    let _ = tokio::fs::remove_file(&target).await;
                    return None;
                }
            }
            Err(e) => {
                tracing::warn!(url=&url, err=?e, "cache_model:stream_err");
                let _ = tokio::fs::remove_file(&target).await;
                return None;
            }
        }
    }

    Some(target)
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_router_model_cache_existing_file() {
        let tmp = tempdir().unwrap();
        let home = tmp.path();
        // Save old HOME
        let old_home = std::env::var("HOME").ok();
        std::env::set_var("HOME", home);

        let dir = home.join(".llm-router").join("models").join("gpt-oss-20b");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("model.gguf");
        std::fs::write(&file, b"dummy").unwrap();

        let info = ModelInfo::new("gpt-oss-20b".to_string(), 0, "test".to_string(), 0, vec![]);

        let path = ensure_router_model_cached(&info).await;
        assert!(path.is_some());
        assert_eq!(path.unwrap(), file);

        // restore HOME
        if let Some(h) = old_home {
            std::env::set_var("HOME", h);
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

    // ===== model_name_to_dir テスト =====

    #[test]
    fn test_model_name_to_dir_flat() {
        assert_eq!(model_name_to_dir("gpt-oss-20b"), "gpt-oss-20b");
        assert_eq!(model_name_to_dir("llama3.2"), "llama3.2");
    }

    #[test]
    fn test_model_name_to_dir_hierarchical() {
        // SPEC-dcaeaec4 FR-2: 階層形式を許可
        assert_eq!(
            model_name_to_dir("openai/gpt-oss-20b"),
            "openai/gpt-oss-20b"
        );
        assert_eq!(model_name_to_dir("meta/llama-3-8b"), "meta/llama-3-8b");
    }

    #[test]
    fn test_model_name_to_dir_case_insensitive() {
        assert_eq!(
            model_name_to_dir("OpenAI/GPT-OSS-20B"),
            "openai/gpt-oss-20b"
        );
    }

    #[test]
    fn test_model_name_to_dir_dangerous_patterns() {
        // 危険なパターンは "_latest" に変換
        assert_eq!(model_name_to_dir("../etc/passwd"), "_latest");
        assert_eq!(model_name_to_dir("model/../other"), "_latest");
    }

    #[test]
    fn test_model_name_to_dir_leading_trailing_slash() {
        // 先頭・末尾のスラッシュは除去
        assert_eq!(model_name_to_dir("/openai/gpt-oss/"), "openai/gpt-oss");
        assert_eq!(model_name_to_dir("/model"), "model");
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
