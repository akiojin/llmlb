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

/// モデル名をディレクトリ名に変換（ファイル名ベース形式）
pub fn model_name_to_dir(name: &str) -> String {
    if name.is_empty() {
        return "_latest".into();
    }

    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_' || c == '.' {
            out.push(c);
        } else if c.is_ascii_uppercase() {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }

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

/// GGUFファイル名からモデルIDを生成（ファイル名ベース形式）
///
/// パターン解析:
/// - "llama-2-7b.Q4_K_M.gguf" → "llama-2-7b"
/// - "gemma-2-9b-it-Q4_K_M.gguf" → "gemma-2-9b-it"
/// - "model.bin" (サイズ情報なし) → リポジトリ名から推測 → "gpt-oss-20b"
///
/// 抽出ルール:
/// 1. 拡張子 (.gguf, .bin) を除去
/// 2. 量子化サフィックス (Q4_K_M, Q5_0, etc.) を除去
/// 3. 小文字に正規化
///
/// 注: コロン区切りの name:tag 形式は廃止し、ファイル名/リポジトリ名をそのまま使用
pub fn generate_ollama_style_id(filename: &str, fallback_repo: &str) -> String {
    // HFのrfilename等はディレクトリを含むことがあるため、常にbasenameで扱う
    let basename = filename.rsplit(&['/', '\\'][..]).next().unwrap_or(filename);

    // 汎用ファイル名（model.bin, model.gguf等）の場合はリポジトリ名から生成
    let base_name = basename
        .trim_end_matches(".gguf")
        .trim_end_matches(".bin")
        .trim_end_matches(".safetensors");

    let parsed = normalize_model_id_candidate(base_name);
    if parsed == "model" || parsed.is_empty() {
        // リポジトリ名の最後の部分を使用 (e.g., "openai/gpt-oss-20b" → "gpt-oss-20b")
        let repo_name = fallback_repo
            .split('/')
            .next_back()
            .unwrap_or(fallback_repo);
        return normalize_model_id_candidate(repo_name);
    }

    parsed
}

fn normalize_model_id_candidate(candidate: &str) -> String {
    // 量子化サフィックスを除去 (Q4_K_M, Q5_0, Q8_0, IQ2_M, etc.)
    let without_quant = remove_quantization_suffix(candidate);

    // -GGUF サフィックスを除去
    let without_gguf = without_quant
        .trim_end_matches("-GGUF")
        .trim_end_matches("-gguf");

    // モデル名とタグを抽出
    extract_name_and_tag(without_gguf)
}

/// 量子化サフィックスを除去
fn remove_quantization_suffix(name: &str) -> &str {
    // パターン: .Q4_K_M, -Q5_0, _Q8_0, .IQ2_M, Q4_K_M (区切りなし) など
    // 再帰的に量子化タグを除去（複数回あり得る場合に備える）

    // まず区切り文字付きパターンを検索
    if let Some(pos) = name.rfind(['.', '-', '_']) {
        let suffix = &name[pos + 1..];
        if is_quantization_tag(suffix) {
            // 再帰的に残りも処理
            return remove_quantization_suffix(&name[..pos]);
        }
    }

    // 区切り文字なしでファイル名末尾に直接量子化タグがある場合
    // 例: "llama-2-7b.Q4_K_M" (拡張子除去後)
    // Q/q または IQ/iq で始まり、数字が続くパターンを末尾から検索
    let lower = name.to_lowercase();
    for pattern_start in ["q", "iq"] {
        if let Some(idx) = lower.rfind(pattern_start) {
            if idx > 0 {
                let before = name.chars().nth(idx - 1);
                // 量子化タグの前は区切り文字か数字以外
                if before.is_some_and(|c| c == '.' || c == '-' || c == '_' || c == 'b' || c == 'B')
                {
                    let after_q = &name[idx + pattern_start.len()..];
                    if after_q.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                        return &name[..idx - 1];
                    }
                }
            }
        }
    }

    name
}

/// 量子化タグかどうかを判定
fn is_quantization_tag(s: &str) -> bool {
    let lower = s.to_lowercase();
    // Q4_K_M, Q5_0, Q8_0, IQ2_M, IQ4_XS など
    (lower.starts_with('q') || lower.starts_with("iq"))
        && lower.len() > 1
        && lower
            .chars()
            .nth(if lower.starts_with("iq") { 2 } else { 1 })
            .is_some_and(|c| c.is_ascii_digit())
}

/// モデル名を正規化して返す（ファイル名ベース形式）
///
/// 注: コロン区切りの name:tag 形式は廃止し、ファイル名/リポジトリ名をそのまま使用
fn extract_name_and_tag(name: &str) -> String {
    // 小文字に正規化してそのまま返す（コロン形式は廃止）
    name.to_lowercase().trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== モデルID生成テスト（ファイル名ベース形式） =====

    #[test]
    fn test_generate_model_id_standard_gguf() {
        // 標準的なGGUFファイル名: llama-2-7b.Q4_K_M.gguf → llama-2-7b
        assert_eq!(
            generate_ollama_style_id("llama-2-7b.Q4_K_M.gguf", "TheBloke/Llama-2-7B-GGUF"),
            "llama-2-7b"
        );
    }

    #[test]
    fn test_generate_model_id_with_variant() {
        // バリアント付き: gemma-2-9b-it → gemma-2-9b-it
        assert_eq!(
            generate_ollama_style_id("gemma-2-9b-it-Q4_K_M.gguf", "bartowski/gemma-2-9b-it-GGUF"),
            "gemma-2-9b-it"
        );
    }

    #[test]
    fn test_generate_model_id_generic_filename() {
        // 汎用ファイル名(model.bin)の場合、リポジトリ名からフォールバック
        assert_eq!(
            generate_ollama_style_id("model.bin", "openai/gpt-oss-20b"),
            "gpt-oss-20b"
        );
    }

    #[test]
    fn test_generate_model_id_generic_quantized_filename_falls_back_to_repo() {
        // `model.Q4_K_M.gguf` のような汎用ファイル名は、量子化タグ除去後に `model` になるため repo にフォールバックする
        assert_eq!(
            generate_ollama_style_id("model.Q4_K_M.gguf", "convertible-repo"),
            "convertible-repo"
        );
    }

    #[test]
    fn test_generate_model_id_with_path_segments() {
        // HFのrfilename等はディレクトリを含むことがある
        assert_eq!(
            generate_ollama_style_id("metal/model.bin", "openai/gpt-oss-20b"),
            "gpt-oss-20b"
        );
    }

    #[test]
    fn test_generate_model_id_no_size() {
        // サイズ情報がない場合もファイル名ベース形式を維持
        assert_eq!(
            generate_ollama_style_id("mistral-small.gguf", "mistral/mistral-small"),
            "mistral-small"
        );
    }

    #[test]
    fn test_generate_model_id_instruct_variant() {
        // Instructバリアント
        assert_eq!(
            generate_ollama_style_id(
                "Mistral-7B-Instruct-v0.2.Q5_K_M.gguf",
                "mistralai/Mistral-7B-Instruct-v0.2-GGUF"
            ),
            "mistral-7b-instruct-v0.2"
        );
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
}
