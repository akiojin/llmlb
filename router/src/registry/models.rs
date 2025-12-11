//! モデル情報管理
//!
//! LLM runtimeモデルのメタデータとダウンロードタスク管理

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

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
    /// モデル名（例: "gpt-oss:20b", "llama3.2"）
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

/// モデル名をディレクトリ名に変換（gpt-oss:20b -> gpt-oss_20b）
pub fn model_name_to_dir(name: &str) -> String {
    if name.is_empty() {
        return "_latest".into();
    }
    let mut dir = name.replace(':', "_");
    if !name.contains(':') {
        dir.push_str("_latest");
    }
    dir
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

        let dir = home.join(".llm-router").join("models").join("gpt-oss_20b");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("model.gguf");
        std::fs::write(&file, b"dummy").unwrap();

        let info = ModelInfo::new("gpt-oss:20b".to_string(), 0, "test".to_string(), 0, vec![]);

        let path = ensure_router_model_cached(&info).await;
        assert!(path.is_some());
        assert_eq!(path.unwrap(), file);

        // restore HOME
        if let Some(h) = old_home {
            std::env::set_var("HOME", h);
        }
    }
}

/// ノードにインストール済みのモデル
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstalledModel {
    /// モデル名
    pub name: String,
    /// モデルサイズ（バイト）
    pub size: u64,
    /// インストール日時
    pub installed_at: DateTime<Utc>,
    /// digest（LLM runtimeのモデル識別子）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

impl InstalledModel {
    /// 新しいInstalledModelを作成
    pub fn new(name: String, size: u64) -> Self {
        Self {
            name,
            size,
            installed_at: Utc::now(),
            digest: None,
        }
    }

    /// digestを指定してInstalledModelを作成
    pub fn with_digest(name: String, size: u64, digest: String) -> Self {
        Self {
            name,
            size,
            installed_at: Utc::now(),
            digest: Some(digest),
        }
    }
}

/// ダウンロードタスク
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DownloadTask {
    /// タスクID
    pub id: Uuid,
    /// ノードID
    pub node_id: Uuid,
    /// モデル名
    pub model_name: String,
    /// ステータス
    pub status: DownloadStatus,
    /// 進捗（0.0-1.0）
    pub progress: f32,
    /// ダウンロード速度（バイト/秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<u64>,
    /// 開始日時
    pub started_at: DateTime<Utc>,
    /// 完了日時
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    /// エラーメッセージ
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl DownloadTask {
    /// 新しいダウンロードタスクを作成
    pub fn new(node_id: Uuid, model_name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_id,
            model_name,
            status: DownloadStatus::Pending,
            progress: 0.0,
            speed: None,
            started_at: Utc::now(),
            completed_at: None,
            error: None,
        }
    }

    /// 進捗を更新
    pub fn update_progress(&mut self, progress: f32, speed: Option<u64>) {
        self.progress = progress.clamp(0.0, 1.0);
        self.speed = speed;

        if self.status == DownloadStatus::Pending && progress > 0.0 {
            self.status = DownloadStatus::InProgress;
        }
    }

    /// 完了として  マーク
    pub fn mark_completed(&mut self) {
        self.status = DownloadStatus::Completed;
        self.progress = 1.0;
        self.completed_at = Some(Utc::now());
    }

    /// 失敗としてマーク
    pub fn mark_failed(&mut self, error: String) {
        self.status = DownloadStatus::Failed;
        self.completed_at = Some(Utc::now());
        self.error = Some(error);
    }

    /// タスクが完了しているか（成功または失敗）
    pub fn is_finished(&self) -> bool {
        matches!(
            self.status,
            DownloadStatus::Completed | DownloadStatus::Failed
        )
    }
}

/// ダウンロードステータス
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    /// 待機中
    Pending,
    /// ダウンロード中
    InProgress,
    /// 完了
    Completed,
    /// 失敗
    Failed,
}

/// GGUFファイル名からOllama風モデルIDを生成
///
/// パターン解析:
/// - "llama-2-7b.Q4_K_M.gguf" → "llama-2:7b"
/// - "gemma-2-9b-it-Q4_K_M.gguf" → "gemma-2:9b-it"
/// - "model.bin" (サイズ情報なし) → リポジトリ名から推測
///
/// 抽出ルール:
/// 1. 拡張子 (.gguf, .bin) を除去
/// 2. 量子化サフィックス (Q4_K_M, Q5_0, etc.) を除去
/// 3. モデル名とサイズ/バリアントを分離
/// 4. 小文字に正規化
pub fn generate_ollama_style_id(filename: &str, fallback_repo: &str) -> String {
    // 汎用ファイル名（model.bin, model.gguf等）の場合はリポジトリ名から生成
    let base_name = filename
        .trim_end_matches(".gguf")
        .trim_end_matches(".bin")
        .trim_end_matches(".safetensors");

    let is_generic = matches!(base_name.to_lowercase().as_str(), "model" | "");

    let name_to_parse = if is_generic {
        // リポジトリ名の最後の部分を使用 (e.g., "openai/gpt-oss-20b" → "gpt-oss-20b")
        fallback_repo
            .split('/')
            .next_back()
            .unwrap_or(fallback_repo)
    } else {
        base_name
    };

    // 量子化サフィックスを除去 (Q4_K_M, Q5_0, Q8_0, IQ2_M, etc.)
    let without_quant = remove_quantization_suffix(name_to_parse);

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

/// モデル名とタグを抽出して "name:tag" 形式で返す
fn extract_name_and_tag(name: &str) -> String {
    let lower = name.to_lowercase();

    // サイズパターンを検索: 7b, 13b, 70b, 8x7b, 9b-it, 7b-instruct-v0.2 など
    // パターン: -{数字}b または -{数字}x{数字}b
    let size_pattern = find_size_pattern(&lower);

    match size_pattern {
        Some((model_name, tag)) => {
            format!("{}:{}", model_name.trim_matches('-'), tag)
        }
        None => {
            // サイズが見つからない場合は :latest を付与
            format!("{}:latest", lower.trim_matches('-'))
        }
    }
}

/// サイズパターンを検索して (モデル名, タグ) を返す
fn find_size_pattern(name: &str) -> Option<(String, String)> {
    // パターン: -7b, -13b, -70b, -8x7b, -7b-it, -7b-instruct-v0.2
    // または: llama-2-7b, gemma-2-9b-it
    let chars: Vec<char> = name.chars().collect();
    let len = chars.len();

    for i in 0..len {
        // '-' または数字の開始を探す
        if chars[i] == '-' || (i > 0 && chars[i].is_ascii_digit()) {
            let start = if chars[i] == '-' { i + 1 } else { i };
            if start >= len {
                continue;
            }

            // 数字を収集
            let mut j = start;
            while j < len && (chars[j].is_ascii_digit() || chars[j] == 'x') {
                j += 1;
            }

            // 'b' または 'B' で終わるかチェック
            if j < len && (chars[j] == 'b' || chars[j] == 'B') {
                let size_end = j + 1;
                let model_name = &name[..i];

                // タグ部分: サイズ + 残りのバリアント
                let tag = if size_end < len {
                    // バリアント部分を含める (例: -it, -instruct-v0.2)
                    let variant = &name[size_end..];
                    let variant = variant.trim_start_matches('-');
                    if variant.is_empty() {
                        name[start..size_end].to_string()
                    } else {
                        format!("{}-{}", &name[start..size_end], variant)
                    }
                } else {
                    name[start..size_end].to_string()
                };

                if !model_name.is_empty() {
                    return Some((model_name.to_string(), tag));
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Ollama風ID生成テスト =====

    #[test]
    fn test_generate_ollama_id_standard_gguf() {
        // 標準的なGGUFファイル名: llama-2-7b.Q4_K_M.gguf → llama-2:7b
        assert_eq!(
            generate_ollama_style_id("llama-2-7b.Q4_K_M.gguf", "TheBloke/Llama-2-7B-GGUF"),
            "llama-2:7b"
        );
    }

    #[test]
    fn test_generate_ollama_id_with_variant() {
        // バリアント付き: gemma-2-9b-it → gemma-2:9b-it
        assert_eq!(
            generate_ollama_style_id("gemma-2-9b-it-Q4_K_M.gguf", "bartowski/gemma-2-9b-it-GGUF"),
            "gemma-2:9b-it"
        );
    }

    #[test]
    fn test_generate_ollama_id_generic_filename() {
        // 汎用ファイル名(model.bin)の場合、リポジトリ名からフォールバック
        assert_eq!(
            generate_ollama_style_id("model.bin", "openai/gpt-oss-20b"),
            "gpt-oss:20b"
        );
    }

    #[test]
    fn test_generate_ollama_id_no_size() {
        // サイズ情報がない場合は :latest を付与
        assert_eq!(
            generate_ollama_style_id("mistral-small.gguf", "mistral/mistral-small"),
            "mistral-small:latest"
        );
    }

    #[test]
    fn test_generate_ollama_id_instruct_variant() {
        // Instructバリアント
        assert_eq!(
            generate_ollama_style_id(
                "Mistral-7B-Instruct-v0.2.Q5_K_M.gguf",
                "mistralai/Mistral-7B-Instruct-v0.2-GGUF"
            ),
            "mistral:7b-instruct-v0.2"
        );
    }

    // ===== 既存テスト =====

    #[test]
    fn test_model_info_new() {
        let model = ModelInfo::new(
            "gpt-oss:20b".to_string(),
            10_000_000_000,
            "GPT-OSS 20B model".to_string(),
            16_000_000_000,
            vec!["llm".to_string(), "text".to_string()],
        );

        assert_eq!(model.name, "gpt-oss:20b");
        assert_eq!(model.size, 10_000_000_000);
        assert_eq!(model.required_memory_gb(), 14.901161193847656);
    }

    #[test]
    fn test_installed_model_new() {
        let model = InstalledModel::new("llama3.2".to_string(), 5_000_000_000);

        assert_eq!(model.name, "llama3.2");
        assert_eq!(model.size, 5_000_000_000);
        assert!(model.digest.is_none());
    }

    #[test]
    fn test_download_task_lifecycle() {
        let mut task = DownloadTask::new(Uuid::new_v4(), "gpt-oss:7b".to_string());

        assert_eq!(task.status, DownloadStatus::Pending);
        assert_eq!(task.progress, 0.0);
        assert!(!task.is_finished());

        // 進捗更新
        task.update_progress(0.5, Some(1_000_000));
        assert_eq!(task.status, DownloadStatus::InProgress);
        assert_eq!(task.progress, 0.5);
        assert_eq!(task.speed, Some(1_000_000));

        // 完了
        task.mark_completed();
        assert_eq!(task.status, DownloadStatus::Completed);
        assert_eq!(task.progress, 1.0);
        assert!(task.is_finished());
        assert!(task.completed_at.is_some());
    }

    #[test]
    fn test_download_task_failure() {
        let mut task = DownloadTask::new(Uuid::new_v4(), "invalid-model".to_string());

        task.mark_failed("Model not found".to_string());
        assert_eq!(task.status, DownloadStatus::Failed);
        assert!(task.is_finished());
        assert_eq!(task.error, Some("Model not found".to_string()));
        assert!(task.completed_at.is_some());
    }

    #[test]
    fn test_progress_clamping() {
        let mut task = DownloadTask::new(Uuid::new_v4(), "test-model".to_string());

        // 範囲外の値はクランプされる
        task.update_progress(1.5, None);
        assert_eq!(task.progress, 1.0);

        task.update_progress(-0.5, None);
        assert_eq!(task.progress, 0.0);
    }
}
