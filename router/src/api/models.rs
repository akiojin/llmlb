//! モデル管理API
//!
//! モデル一覧取得、登録、変換、ファイル配信のエンドポイント

use crate::{
    db::models::ModelStorage,
    registry::models::{extract_repo_id, generate_model_id, router_model_path, ModelInfo},
    registry::NodeRegistry,
    AppState,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use llm_router_common::error::{CommonError, RouterError};
use once_cell::sync::Lazy;
use reqwest;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// モデル名の妥当性を検証
///
/// 有効なモデル名の形式:
/// - `gpt-oss-20b`, `mistral-7b-instruct-v0.2` のようなファイル名ベース形式
/// - `openai/gpt-oss-20b` のような階層形式（HuggingFace互換）
///
/// SPEC-dcaeaec4 FR-2: 階層形式を許可
fn validate_model_name(model_name: &str) -> Result<(), RouterError> {
    if model_name.is_empty() {
        return Err(RouterError::InvalidModelName(
            "Model name is empty".to_string(),
        ));
    }

    // 危険なパターンを禁止（パストラバーサル対策）
    if model_name.contains("..") || model_name.contains('\0') {
        return Err(RouterError::InvalidModelName(format!(
            "Invalid model name (contains dangerous pattern): {}",
            model_name
        )));
    }

    // 先頭・末尾のスラッシュは禁止
    if model_name.starts_with('/') || model_name.ends_with('/') {
        return Err(RouterError::InvalidModelName(format!(
            "Invalid model name (leading/trailing slash): {}",
            model_name
        )));
    }

    // 許可する文字: 小文字英数字、'-', '_', '.', '/'（ディレクトリセパレータ）
    if !model_name.chars().all(|c| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_' || c == '.' || c == '/'
    }) {
        return Err(RouterError::InvalidModelName(format!(
            "Invalid model name: {}",
            model_name
        )));
    }

    Ok(())
}

// NOTE: AvailableModelView, AvailableModelsResponse, Pagination, model_info_to_view() は
// /v0/models/available 廃止に伴い削除されました。

/// モデルのライフサイクル状態
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LifecycleStatus {
    /// 登録リクエスト受付、キャッシュ待ち
    Pending,
    /// ダウンロード・変換中（キャッシュ処理中）
    Caching,
    /// ルーターにキャッシュ完了（ノードがアクセス可能）
    Registered,
    /// エラー発生
    Error,
}

/// ダウンロード進行状況
#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    /// 進行率（0.0〜1.0）
    pub percent: f64,
    /// ダウンロード済みバイト数
    pub bytes_downloaded: Option<u64>,
    /// 総バイト数
    pub bytes_total: Option<u64>,
    /// エラーメッセージ（status=errorの場合）
    pub error: Option<String>,
}

// NOTE: RegisteredModelView と model_info_to_registered_view は /v0/models 廃止に伴い削除。
// ダッシュボードは /v1/models を使用し、TypeScript側で型を定義。

// ===== Registered model store (in-memory) =====
static REGISTERED_MODELS: Lazy<RwLock<Vec<ModelInfo>>> = Lazy::new(|| RwLock::new(Vec::new()));

/// 登録済みモデルをストレージからロード
pub async fn load_registered_models_from_storage(pool: SqlitePool) {
    let storage = ModelStorage::new(pool);
    if let Ok(models) = storage.load_models().await {
        let mut store = REGISTERED_MODELS.write().unwrap();
        *store = models;
    }
}

// NOTE: get_registered_models() ハンドラは廃止されました。
// モデル一覧は /v1/models を使用してください（openai::list_models）。
// LifecycleStatus, DownloadProgress 型は openai.rs で使用するため維持。

/// 登録済みモデル一覧を取得
pub fn list_registered_models() -> Vec<ModelInfo> {
    REGISTERED_MODELS.read().unwrap().clone()
}

/// GET /v0/models - 登録済みモデル一覧（拡張メタデータ付き）
///
/// ノード同期用途向け。配列を直接返す。
pub async fn list_models() -> Json<Vec<ModelInfo>> {
    let mut models = list_registered_models();

    for model in models.iter_mut() {
        if model.path.is_none() {
            if let Some(path) = router_model_path(&model.name) {
                if path.exists() {
                    model.path = Some(path.to_string_lossy().to_string());
                }
            }
        }
    }

    Json(models)
}

fn find_model_by_name(name: &str) -> Option<ModelInfo> {
    list_registered_models()
        .into_iter()
        .find(|m| m.name == name)
}

/// 登録モデルを追加（重複チェックあり）
pub(crate) fn add_registered_model(model: ModelInfo) -> Result<(), RouterError> {
    let mut store = REGISTERED_MODELS.write().unwrap();
    if store.iter().any(|m| m.name == model.name) {
        return Err(RouterError::InvalidModelName(
            "Model already registered".into(),
        ));
    }
    store.push(model);
    Ok(())
}

/// 既存登録モデルを更新または追加（重複エラーにせず上書き）
pub fn upsert_registered_model(model: ModelInfo) {
    let mut store = REGISTERED_MODELS.write().unwrap();
    if let Some(existing) = store.iter_mut().find(|m| m.name == model.name) {
        *existing = model;
    } else {
        store.push(model);
    }
}

/// 登録モデルを名前で削除し、削除が行われたかを返す
pub fn remove_registered_model(name: &str) -> bool {
    let mut store = REGISTERED_MODELS.write().unwrap();
    let initial_len = store.len();
    store.retain(|m| m.name != name);
    initial_len != store.len()
}

/// 登録モデルを永続化（失敗はログのみ）
pub async fn persist_registered_models(pool: &SqlitePool) {
    if let Ok(store) = std::panic::catch_unwind(|| REGISTERED_MODELS.read().unwrap().clone()) {
        let storage = ModelStorage::new(pool.clone());
        if let Err(e) = storage.save_models(&store).await {
            tracing::error!("Failed to persist registered models: {}", e);
        }
    }
}

/// 登録モデルの整合性チェックを実行
///
/// チェック内容:
/// 1. DBとメモリの整合性確認
/// 2. ファイル存在確認（存在しないモデルをログ出力）
/// 3. 不整合があれば警告ログを出力
///
/// NOTE: 自動削除は行わない（手動介入を想定）
pub async fn sync_registered_models(registry: &NodeRegistry, pool: &SqlitePool) {
    tracing::debug!("Starting model consistency check");

    let storage = ModelStorage::new(pool.clone());

    // 1. DBからモデルをロード
    let db_models = match storage.load_models().await {
        Ok(models) => models,
        Err(e) => {
            tracing::error!("Failed to load models from DB: {}", e);
            return;
        }
    };

    // 2. メモリ上のモデルを取得
    let memory_models = list_registered_models();

    // 3. DB vs メモリの整合性チェック
    let db_names: std::collections::HashSet<_> = db_models.iter().map(|m| m.name.clone()).collect();
    let memory_names: std::collections::HashSet<_> =
        memory_models.iter().map(|m| m.name.clone()).collect();

    // DBにあってメモリにないモデル
    let in_db_only: Vec<_> = db_names.difference(&memory_names).collect();
    if !in_db_only.is_empty() {
        tracing::warn!(
            models=?in_db_only,
            "Models in DB but not in memory - reloading from DB"
        );
        // DBからメモリに復元
        let mut store = REGISTERED_MODELS.write().unwrap();
        for model in &db_models {
            if in_db_only.contains(&&model.name) {
                store.push(model.clone());
            }
        }
    }

    // メモリにあってDBにないモデル
    let in_memory_only: Vec<_> = memory_names.difference(&db_names).collect();
    if !in_memory_only.is_empty() {
        tracing::warn!(
            models=?in_memory_only,
            "Models in memory but not in DB - persisting to DB"
        );
        // メモリからDBに永続化
        persist_registered_models(pool).await;
    }

    // 4. ファイル存在チェック
    let mut missing_files = Vec::new();
    for model in list_registered_models() {
        let exists = model
            .path
            .as_ref()
            .map(std::path::PathBuf::from)
            .filter(|path| crate::registry::models::is_valid_model_file(path))
            .is_some()
            || router_model_path(&model.name).is_some();

        if !exists {
            missing_files.push(model.name.clone());
        }
    }

    if !missing_files.is_empty() {
        tracing::warn!(
            models=?missing_files,
            "Registered models with missing files"
        );
    }

    // 5. ノードロード状態の確認
    let nodes = registry.list().await;
    let mut loaded_on_nodes: std::collections::HashMap<String, Vec<String>> = HashMap::new();
    for node in &nodes {
        for model_name in &node.loaded_models {
            loaded_on_nodes
                .entry(model_name.clone())
                .or_default()
                .push(node.id.to_string());
        }
    }

    // 登録済みモデルのノードロード状態をログ
    let registered_names: std::collections::HashSet<_> = list_registered_models()
        .iter()
        .map(|m| m.name.clone())
        .collect();
    let loaded_names: std::collections::HashSet<_> = loaded_on_nodes.keys().cloned().collect();

    // ノードにロード済みだが未登録のモデル
    let loaded_but_unregistered: Vec<_> = loaded_names.difference(&registered_names).collect();
    if !loaded_but_unregistered.is_empty() {
        tracing::info!(
            models=?loaded_but_unregistered,
            "Models loaded on nodes but not registered"
        );
    }

    tracing::debug!(
        db_count = db_models.len(),
        memory_count = memory_models.len(),
        missing_files = missing_files.len(),
        nodes_count = nodes.len(),
        "Model consistency check completed"
    );
}

/// 定期的な整合性チェックを開始（5分間隔）
pub fn start_periodic_sync(registry: NodeRegistry, pool: SqlitePool) {
    tokio::spawn(async move {
        let interval = Duration::from_secs(300); // 5分
        loop {
            tokio::time::sleep(interval).await;
            sync_registered_models(&registry, &pool).await;
        }
    });
}

// ===== GGUF Discovery Cache =====

/// GGUF版検索結果
#[derive(Debug, Clone, Serialize)]
pub struct GgufDiscoveryResult {
    /// リポジトリ名 (例: ggml-org/gpt-oss-20b-GGUF)
    pub repo: String,
    /// プロバイダー名 (例: ggml-org)
    pub provider: String,
    /// 信頼プロバイダーかどうか
    pub trusted: bool,
    /// 利用可能なGGUFファイル
    pub files: Vec<GgufFileInfo>,
}

/// GGUFファイル情報
#[derive(Debug, Clone, Serialize)]
pub struct GgufFileInfo {
    /// ファイル名
    pub filename: String,
    /// ファイルサイズ（バイト）
    pub size_bytes: u64,
    /// 量子化タイプ（推測）
    pub quantization: Option<String>,
}

#[derive(Clone)]
struct GgufDiscoveryCache {
    fetched_at: Instant,
    results: Vec<GgufDiscoveryResult>,
}

static GGUF_DISCOVERY_CACHE: Lazy<RwLock<HashMap<String, GgufDiscoveryCache>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

const GGUF_DISCOVERY_CACHE_TTL: Duration = Duration::from_secs(600);

/// 信頼できるGGUFプロバイダーの優先順位
const TRUSTED_PROVIDERS: &[&str] = &[
    "ggml-org",
    "bartowski",
    "lmstudio-community",
    "unsloth",
    "TheBloke",
];

/// サポートする量子化タイプ（API/UI共通）
const SUPPORTED_QUANTIZATION_LABELS: &[&str] = &[
    "BF16", "F32", "F16", "Q8_0", "Q6_K", "Q5_K_M", "Q5_K_S", "Q5_0", "Q4_K_M", "Q4_K_S", "Q4_0",
    "Q3_K_M", "Q3_K_S", "Q2_K", "IQ4_XS", "IQ3_M", "IQ2_M",
];

pub(crate) fn normalize_quantization_label(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    for label in SUPPORTED_QUANTIZATION_LABELS {
        if label.eq_ignore_ascii_case(trimmed) {
            return Some((*label).to_string());
        }
    }
    None
}

async fn fetch_repo_siblings(
    http_client: &reqwest::Client,
    repo: &str,
) -> Result<Vec<HfSibling>, RouterError> {
    let base_url = std::env::var("HF_BASE_URL")
        .unwrap_or_else(|_| "https://huggingface.co".to_string())
        .trim_end_matches('/')
        .to_string();
    let url = format!("{}/api/models/{}?expand=siblings", base_url, repo);

    let mut req = http_client.get(&url);
    if let Ok(token) = std::env::var("HF_TOKEN") {
        req = req.bearer_auth(token);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| RouterError::Http(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(RouterError::Common(CommonError::Validation(
            "Failed to fetch specified repository".into(),
        )));
    }
    #[derive(Deserialize)]
    struct RepoDetail {
        siblings: Vec<HfSibling>,
    }
    let detail: RepoDetail = resp
        .json()
        .await
        .map_err(|e| RouterError::Http(e.to_string()))?;
    Ok(detail.siblings)
}

fn is_gguf_filename(filename: &str) -> bool {
    filename.to_ascii_lowercase().ends_with(".gguf")
}

fn is_safetensors_index_filename(filename: &str) -> bool {
    filename
        .to_ascii_lowercase()
        .ends_with(".safetensors.index.json")
}

fn is_safetensors_filename(filename: &str) -> bool {
    let lower = filename.to_ascii_lowercase();
    lower.ends_with(".safetensors") || lower.ends_with(".safetensors.index.json")
}

fn sibling_size_bytes(s: &HfSibling) -> u64 {
    s.size
        .or_else(|| s.lfs.as_ref().and_then(|l| l.size))
        .unwrap_or(0)
}

fn has_sibling(siblings: &[HfSibling], filename: &str) -> bool {
    siblings.iter().any(|s| s.rfilename == filename)
}

fn require_safetensors_metadata_files(siblings: &[HfSibling]) -> Result<(), RouterError> {
    let has_config = has_sibling(siblings, "config.json");
    let has_tokenizer = has_sibling(siblings, "tokenizer.json");
    if !has_config || !has_tokenizer {
        return Err(RouterError::Common(CommonError::Validation(
            "config.json and tokenizer.json are required for safetensors models".into(),
        )));
    }
    Ok(())
}

fn resolve_safetensors_primary(
    siblings: &[HfSibling],
    requested: Option<String>,
) -> Result<String, RouterError> {
    if let Some(filename) = requested {
        if !is_safetensors_filename(&filename) {
            return Err(RouterError::Common(CommonError::Validation(
                "filename must be a safetensors or safetensors index file".into(),
            )));
        }
        if !has_sibling(siblings, &filename) {
            return Err(RouterError::Common(CommonError::Validation(
                "Specified safetensors file not found in repository".into(),
            )));
        }
        return Ok(filename);
    }

    let index_files: Vec<_> = siblings
        .iter()
        .map(|s| s.rfilename.clone())
        .filter(|f| is_safetensors_index_filename(f))
        .collect();
    if index_files.len() == 1 {
        return Ok(index_files[0].clone());
    }
    if index_files.len() > 1 {
        return Err(RouterError::Common(CommonError::Validation(
            "Multiple safetensors index files found; specify filename".into(),
        )));
    }

    let st_files: Vec<_> = siblings
        .iter()
        .map(|s| s.rfilename.clone())
        .filter(|f| {
            // .safetensors だが index は除外
            f.to_ascii_lowercase().ends_with(".safetensors") && !is_safetensors_index_filename(f)
        })
        .collect();
    if st_files.len() == 1 {
        return Ok(st_files[0].clone());
    }
    if st_files.is_empty() {
        return Err(RouterError::Common(CommonError::Validation(
            "No safetensors file found in repository".into(),
        )));
    }
    Err(RouterError::Common(CommonError::Validation(
        "Multiple safetensors files found; specify filename".into(),
    )))
}

fn gguf_quality_rank(label: &str) -> u32 {
    match label {
        "F32" => 0,
        "BF16" => 1,
        "F16" => 2,
        "Q8_0" => 3,
        "Q6_K" => 4,
        "Q5_K_M" => 5,
        "Q5_K_S" => 6,
        "Q5_0" => 7,
        "Q4_K_M" => 8,
        "Q4_K_S" => 9,
        "Q4_0" => 10,
        "Q3_K_M" => 11,
        "Q3_K_S" => 12,
        "Q2_K" => 13,
        "IQ4_XS" => 14,
        "IQ3_M" => 15,
        "IQ2_M" => 16,
        _ => 999,
    }
}

fn gguf_speed_acceptable(label: &str) -> bool {
    matches!(
        label,
        "F32"
            | "BF16"
            | "F16"
            | "Q8_0"
            | "Q6_K"
            | "Q5_K_M"
            | "Q5_K_S"
            | "Q5_0"
            | "Q4_K_M"
            | "Q4_K_S"
            | "Q4_0"
    )
}

fn resolve_gguf_by_policy(
    siblings: &[HfSibling],
    policy: GgufSelectionPolicy,
) -> Result<String, RouterError> {
    let ggufs: Vec<_> = siblings
        .iter()
        .filter(|s| is_gguf_filename(&s.rfilename))
        .collect();
    if ggufs.is_empty() {
        return Err(RouterError::Common(CommonError::Validation(
            "No GGUF file found in repository".into(),
        )));
    }

    let selected = match policy {
        GgufSelectionPolicy::Quality => ggufs
            .iter()
            .min_by(|a, b| {
                let qa = extract_quantization(&a.rfilename)
                    .map(|q| gguf_quality_rank(&q))
                    .unwrap_or(999);
                let qb = extract_quantization(&b.rfilename)
                    .map(|q| gguf_quality_rank(&q))
                    .unwrap_or(999);
                qa.cmp(&qb)
                    // 同rankならサイズが大きい方を優先（高精度の可能性）
                    .then_with(|| sibling_size_bytes(b).cmp(&sibling_size_bytes(a)))
                    .then_with(|| a.rfilename.cmp(&b.rfilename))
            })
            .copied()
            .unwrap(),
        GgufSelectionPolicy::Memory => ggufs
            .iter()
            .min_by(|a, b| {
                sibling_size_bytes(a)
                    .cmp(&sibling_size_bytes(b))
                    .then_with(|| a.rfilename.cmp(&b.rfilename))
            })
            .copied()
            .unwrap(),
        GgufSelectionPolicy::Speed => {
            let mut candidates: Vec<_> = ggufs
                .iter()
                .filter(|s| {
                    extract_quantization(&s.rfilename)
                        .map(|q| gguf_speed_acceptable(&q))
                        .unwrap_or(false)
                })
                .copied()
                .collect();
            if candidates.is_empty() {
                candidates = ggufs.clone();
            }
            candidates
                .iter()
                .min_by(|a, b| {
                    sibling_size_bytes(a)
                        .cmp(&sibling_size_bytes(b))
                        .then_with(|| a.rfilename.cmp(&b.rfilename))
                })
                .copied()
                .unwrap()
        }
    };

    Ok(selected.rfilename.clone())
}

/// リポジトリ内のGGUFファイルを量子化指定で解決
async fn resolve_quantized_gguf_in_repo(
    http_client: &reqwest::Client,
    repo: &str,
    quantization: &str,
) -> Result<String, RouterError> {
    let siblings = fetch_repo_siblings(http_client, repo).await?;
    let filename = siblings
        .iter()
        .map(|s| s.rfilename.clone())
        .find(|f| {
            if !f.to_ascii_lowercase().ends_with(".gguf") {
                return false;
            }
            match extract_quantization(f) {
                Some(q) => q == quantization,
                None => false,
            }
        })
        .ok_or_else(|| {
            RouterError::Common(CommonError::Validation(
                "No GGUF file found for specified quantization".into(),
            ))
        })?;
    Ok(filename)
}

/// HuggingFaceのtokenizer_config.jsonからchat_templateを取得
async fn fetch_chat_template_from_hf(http_client: &reqwest::Client, repo: &str) -> Option<String> {
    let base_url = std::env::var("HF_BASE_URL")
        .unwrap_or_else(|_| "https://huggingface.co".to_string())
        .trim_end_matches('/')
        .to_string();
    let url = format!("{}/{}/resolve/main/tokenizer_config.json", base_url, repo);

    let mut req = http_client.get(&url);
    if let Ok(token) = std::env::var("HF_TOKEN") {
        req = req.bearer_auth(token);
    }

    match req.send().await {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                let template = json
                    .get("chat_template")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                if template.is_some() {
                    tracing::info!(repo = %repo, "chat_template fetched from tokenizer_config.json");
                }
                template
            } else {
                tracing::debug!(repo = %repo, "Failed to parse tokenizer_config.json");
                None
            }
        }
        Ok(resp) => {
            tracing::debug!(repo = %repo, status = ?resp.status(), "tokenizer_config.json not found");
            None
        }
        Err(e) => {
            tracing::debug!(repo = %repo, error = %e, "Failed to fetch tokenizer_config.json");
            None
        }
    }
}

/// モデル名からGGUF版を検索
pub async fn discover_gguf_versions(
    http_client: &reqwest::Client,
    base_model_name: &str,
) -> Result<Vec<GgufDiscoveryResult>, RouterError> {
    // キャッシュチェック
    {
        let cache = GGUF_DISCOVERY_CACHE.read().unwrap();
        if let Some(entry) = cache.get(base_model_name) {
            if entry.fetched_at.elapsed() < GGUF_DISCOVERY_CACHE_TTL {
                return Ok(entry.results.clone());
            }
        }
    }

    // モデル名を正規化 (org/model -> model)
    let search_name = base_model_name
        .split('/')
        .next_back()
        .unwrap_or(base_model_name);

    // HuggingFace APIでGGUF版を検索
    let token = std::env::var("HF_TOKEN").ok();
    let base_url = std::env::var("HF_BASE_URL")
        .unwrap_or_else(|_| "https://huggingface.co".to_string())
        .trim_end_matches('/')
        .to_string();

    // library=gguf で検索
    let url = format!(
        "{}/api/models?library=gguf&search={}&limit=50&expand=siblings",
        base_url, search_name
    );

    let mut req = http_client.get(&url);
    if let Some(t) = &token {
        req = req.bearer_auth(t);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| RouterError::Http(e.to_string()))?;

    if !resp.status().is_success() {
        // キャッシュがあればそれを返す
        let cache = GGUF_DISCOVERY_CACHE.read().unwrap();
        if let Some(entry) = cache.get(base_model_name) {
            return Ok(entry.results.clone());
        }
        return Err(RouterError::Http(format!(
            "HF API returned {}",
            resp.status()
        )));
    }

    let models: Vec<HfModel> = resp
        .json()
        .await
        .map_err(|e| RouterError::Http(e.to_string()))?;

    let mut results: Vec<GgufDiscoveryResult> = Vec::new();

    for model in models {
        let provider = model.model_id.split('/').next().unwrap_or("").to_string();
        let trusted = TRUSTED_PROVIDERS.contains(&provider.as_str());

        // GGUFファイルを抽出
        let gguf_files: Vec<GgufFileInfo> = model
            .siblings
            .iter()
            .filter(|s| s.rfilename.to_lowercase().ends_with(".gguf"))
            .map(|s| {
                let size = s
                    .lfs
                    .as_ref()
                    .and_then(|l| l.size)
                    .unwrap_or(s.size.unwrap_or(0));
                let quantization = extract_quantization(&s.rfilename);
                GgufFileInfo {
                    filename: s.rfilename.clone(),
                    size_bytes: size,
                    quantization,
                }
            })
            .collect();

        if !gguf_files.is_empty() {
            results.push(GgufDiscoveryResult {
                repo: model.model_id.clone(),
                provider,
                trusted,
                files: gguf_files,
            });
        }
    }

    // 信頼プロバイダー順にソート
    results.sort_by(|a, b| {
        let a_priority = TRUSTED_PROVIDERS
            .iter()
            .position(|&p| p == a.provider)
            .unwrap_or(usize::MAX);
        let b_priority = TRUSTED_PROVIDERS
            .iter()
            .position(|&p| p == b.provider)
            .unwrap_or(usize::MAX);
        a_priority.cmp(&b_priority)
    });

    // キャッシュに保存
    {
        let mut cache = GGUF_DISCOVERY_CACHE.write().unwrap();
        cache.insert(
            base_model_name.to_string(),
            GgufDiscoveryCache {
                fetched_at: Instant::now(),
                results: results.clone(),
            },
        );
    }

    Ok(results)
}

/// ファイル名から量子化タイプを推測
fn extract_quantization(filename: &str) -> Option<String> {
    let upper = filename.to_uppercase();
    for label in SUPPORTED_QUANTIZATION_LABELS {
        if upper.contains(label) {
            return Some((*label).to_string());
        }
    }
    None
}

/// 登録モデルのインメモリキャッシュをクリア（テスト用）
pub fn clear_registered_models() {
    *REGISTERED_MODELS.write().unwrap() = Vec::new();
}

/// HFリポジトリのsiblings情報
#[derive(Deserialize)]
struct HfSibling {
    #[serde(rename = "rfilename")]
    rfilename: String,
    /// ファイルサイズ（オプション）
    #[serde(default)]
    size: Option<u64>,
    /// LFS情報（オプション）
    lfs: Option<HfLfs>,
}

/// HF LFS情報
#[derive(Deserialize)]
struct HfLfs {
    /// ファイルサイズ
    size: Option<u64>,
}

/// HFモデル情報（discover_gguf_versions用）
#[derive(Deserialize)]
struct HfModel {
    /// モデルID (例: "bartowski/Qwen2.5-7B-Instruct-GGUF")
    #[serde(rename = "modelId")]
    model_id: String,
    /// ファイル一覧
    #[serde(default)]
    siblings: Vec<HfSibling>,
}

/// Axum用のエラーレスポンス型
#[derive(Debug)]
pub struct AppError(RouterError);

impl From<RouterError> for AppError {
    fn from(err: RouterError) -> Self {
        AppError(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self.0 {
            RouterError::NodeNotFound(_) => (StatusCode::NOT_FOUND, self.0.to_string()),
            RouterError::NoNodesAvailable => (StatusCode::SERVICE_UNAVAILABLE, self.0.to_string()),
            RouterError::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg.clone()),
            RouterError::NodeOffline(_) => (StatusCode::SERVICE_UNAVAILABLE, self.0.to_string()),
            RouterError::InvalidModelName(_) => (StatusCode::BAD_REQUEST, self.0.to_string()),
            RouterError::InsufficientStorage(_) => {
                (StatusCode::INSUFFICIENT_STORAGE, self.0.to_string())
            }
            RouterError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string()),
            RouterError::Http(_) => (StatusCode::BAD_GATEWAY, self.0.to_string()),
            RouterError::Timeout(_) => (StatusCode::GATEWAY_TIMEOUT, self.0.to_string()),
            RouterError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string()),
            RouterError::PasswordHash(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string()),
            RouterError::Jwt(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string()),
            RouterError::Authentication(_) => (StatusCode::UNAUTHORIZED, self.0.to_string()),
            RouterError::Authorization(_) => (StatusCode::FORBIDDEN, self.0.to_string()),
            RouterError::Common(err) => (StatusCode::BAD_REQUEST, err.to_string()),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

// NOTE: GET /v0/models/available は廃止されました。
// HFカタログは直接 https://huggingface.co を参照してください。

#[derive(Deserialize)]
/// HFモデル登録リクエスト
pub struct RegisterModelRequest {
    /// HFリポジトリ名 (e.g., TheBloke/Llama-2-7B-GGUF)
    pub repo: String,
    /// 登録するモデル形式（HF上に safetensors/GGUF の両方がある場合は必須）
    #[serde(default)]
    pub format: Option<ModelArtifactFormat>,
    /// ファイル名 (e.g., llama-2-7b.Q4_K_M.gguf)
    pub filename: Option<String>,
    /// 量子化指定（例: Q4_K_M）
    ///
    /// NOTE: `format=gguf` で `filename` 未指定の場合は `gguf_policy` を推奨。
    #[serde(default)]
    pub quantization: Option<String>,
    /// GGUFの選択ポリシー（`format=gguf` かつ `filename` 未指定の場合に使用）
    #[serde(default)]
    pub gguf_policy: Option<GgufSelectionPolicy>,
    /// 表示名（任意）
    #[serde(default)]
    pub display_name: Option<String>,
    /// オプションのchat_template（GGUFに含まれない場合の補助）
    #[serde(default)]
    pub chat_template: Option<String>,
}

/// モデルアーティファクト形式（登録時に選択）
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelArtifactFormat {
    /// Hugging Face の safetensors（config/tokenizer を含むスナップショット）
    Safetensors,
    /// GGUF（llama.cpp 用）
    Gguf,
}

/// GGUF siblings からの選択ポリシー
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GgufSelectionPolicy {
    /// 高品質を優先（F32/BF16/F16/Q8/Q6... の順）
    Quality,
    /// 省メモリを優先（ファイルサイズ最小）
    Memory,
    /// 速度優先（実用的な量子化から小さめを選択）
    Speed,
}

async fn compute_gpu_warnings(registry: &NodeRegistry, required_memory: u64) -> Vec<String> {
    let mut warnings = Vec::new();
    if required_memory == 0 {
        return warnings;
    }

    let nodes = registry.list().await;
    let mut memories: Vec<u64> = Vec::new();
    for node in nodes {
        for device in node.gpu_devices {
            if let Some(mem) = device.memory {
                memories.push(mem);
            }
        }
    }

    if memories.is_empty() {
        warnings.push("No GPU memory info available from registered nodes".into());
        return warnings;
    }

    let max_mem = *memories.iter().max().unwrap();
    if required_memory > max_mem {
        warnings.push(format!(
            "Model requires {:.1}GB but max node GPU memory is {:.1}GB",
            required_memory as f64 / (1024.0 * 1024.0 * 1024.0),
            max_mem as f64 / (1024.0 * 1024.0 * 1024.0),
        ));
    }

    warnings
}

/// POST /v0/models/register - HF GGUFを対応モデルに登録
///
/// 方針:
/// - HFリポジトリ内に safetensors/GGUF の両方がある場合は、登録時に `format` が必須
/// - safetensors を選んだ場合は、`config.json` / `tokenizer.json` を必須とし、weights(safetensors)をキャッシュする
/// - GGUF を選んだ場合は、GGUFファイルをキャッシュする（`filename` 未指定なら `gguf_policy` で選択）
pub async fn register_model(
    State(state): State<AppState>,
    Json(req): Json<RegisterModelRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    // URLからrepo_idを抽出（フルURLが渡された場合はrepo_id形式に正規化）
    let repo = extract_repo_id(&req.repo);

    if req.gguf_policy.is_some() && req.quantization.is_some() {
        return Err(RouterError::Common(CommonError::Validation(
            "Specify either gguf_policy or quantization, not both".into(),
        ))
        .into());
    }

    let quantization = match req.quantization.as_deref() {
        Some(q) => Some(normalize_quantization_label(q).ok_or_else(|| {
            RouterError::Common(CommonError::Validation("Invalid quantization label".into()))
        })?),
        None => None,
    };

    let siblings = fetch_repo_siblings(&state.http_client, &repo).await?;
    let has_gguf = siblings.iter().any(|s| is_gguf_filename(&s.rfilename));
    let has_safetensors = siblings
        .iter()
        .any(|s| is_safetensors_filename(&s.rfilename));

    if !has_gguf && !has_safetensors {
        return Err(RouterError::Common(CommonError::Validation(
            "No supported model artifacts found (safetensors/gguf)".into(),
        ))
        .into());
    }

    if has_gguf && has_safetensors && req.format.is_none() {
        return Err(RouterError::Common(CommonError::Validation(
            "format is required when both safetensors and gguf exist in the repository".into(),
        ))
        .into());
    }

    let format = req.format.unwrap_or(if has_safetensors {
        ModelArtifactFormat::Safetensors
    } else {
        ModelArtifactFormat::Gguf
    });

    let filename = match format {
        ModelArtifactFormat::Gguf => {
            if !has_gguf {
                return Err(RouterError::Common(CommonError::Validation(
                    "GGUF not found in repository".into(),
                ))
                .into());
            }

            match req.filename.clone() {
                Some(f) => {
                    if !is_gguf_filename(&f) {
                        return Err(RouterError::Common(CommonError::Validation(
                            "filename must be a gguf file when format=gguf".into(),
                        ))
                        .into());
                    }
                    if !has_sibling(&siblings, &f) {
                        return Err(RouterError::Common(CommonError::Validation(
                            "Specified GGUF file not found in repository".into(),
                        ))
                        .into());
                    }
                    if let Some(q) = quantization.as_deref() {
                        let detected = extract_quantization(&f);
                        if detected.as_deref() != Some(q) {
                            return Err(RouterError::Common(CommonError::Validation(
                                "Quantization does not match filename".into(),
                            ))
                            .into());
                        }
                    }
                    f
                }
                None => {
                    if let Some(q) = quantization.as_deref() {
                        // legacy: quantization指定あり → 一致するGGUFを選択
                        resolve_quantized_gguf_in_repo(&state.http_client, &repo, q).await?
                    } else {
                        let policy = req.gguf_policy.ok_or_else(|| {
                            RouterError::Common(CommonError::Validation(
                                "gguf_policy is required when filename is not specified (format=gguf)".into(),
                            ))
                        })?;
                        resolve_gguf_by_policy(&siblings, policy)?
                    }
                }
            }
        }
        ModelArtifactFormat::Safetensors => {
            if !has_safetensors {
                return Err(RouterError::Common(CommonError::Validation(
                    "safetensors not found in repository".into(),
                ))
                .into());
            }
            require_safetensors_metadata_files(&siblings)?;
            resolve_safetensors_primary(&siblings, req.filename.clone())?
        }
    };

    register_model_internal(
        &state,
        &repo,
        format,
        &filename,
        quantization.clone(),
        req.display_name.clone(),
        req.chat_template.clone(),
    )
    .await
}

/// モデル登録の内部実装
async fn register_model_internal(
    state: &AppState,
    repo: &str,
    format: ModelArtifactFormat,
    filename: &str,
    quantization: Option<String>,
    _display_name: Option<String>,
    chat_template: Option<String>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    // モデルIDは階層形式（リポジトリ名）を使用 (SPEC-dcaeaec4 FR-2)
    let name = generate_model_id(repo);

    // 重複チェック：登録済みモデルまたは処理中タスクに同じリポジトリがあればエラー
    if find_model_by_name(&name).is_some() {
        return Err(RouterError::Common(CommonError::Validation(
            "Model already registered".into(),
        ))
        .into());
    }
    if state.convert_manager.has_task_for_repo(repo).await {
        return Err(RouterError::Common(CommonError::Validation(
            "Model already registered".into(),
        ))
        .into());
    }

    // chat_template: ユーザー指定がなければHFのtokenizer_config.jsonから自動取得
    let chat_template = if chat_template.is_some() {
        chat_template
    } else {
        fetch_chat_template_from_hf(&state.http_client, repo).await
    };

    // 事前に概算サイズ（HF siblingsのsize情報がある場合）を推定して警告を出す
    let (content_length, required_memory, warnings) = {
        let siblings = fetch_repo_siblings(&state.http_client, repo).await?;

        let (size_bytes, required_memory) = match format {
            ModelArtifactFormat::Gguf => {
                let size = siblings
                    .iter()
                    .find(|s| s.rfilename == filename)
                    .map(sibling_size_bytes)
                    .unwrap_or(0);
                const REQUIRED_MEMORY_RATIO: f64 = 1.5;
                let required = if size > 0 {
                    ((size as f64) * REQUIRED_MEMORY_RATIO).ceil() as u64
                } else {
                    0
                };
                (size, required)
            }
            ModelArtifactFormat::Safetensors => {
                // index + shards を前提に、safetensorsファイル群の合計サイズを推定
                let total = siblings
                    .iter()
                    .filter(|s| s.rfilename.to_ascii_lowercase().ends_with(".safetensors"))
                    .map(sibling_size_bytes)
                    .sum::<u64>();
                const REQUIRED_MEMORY_RATIO: f64 = 1.5;
                let required = if total > 0 {
                    ((total as f64) * REQUIRED_MEMORY_RATIO).ceil() as u64
                } else {
                    0
                };
                (total, required)
            }
        };

        let warnings = compute_gpu_warnings(&state.registry, required_memory).await;
        (size_bytes, required_memory, warnings)
    };

    // NOTE: モデル登録は ConvertTask 完了時に finalize_model_registration() で行う
    // ここでは REGISTERED_MODELS に追加しない（UI上の重複を防ぐため）

    // コンバートキューへ投入（GGUFは即完了、非GGUFはconvert）
    // 重複チェックのためenqueueはawaitして、タスクがキューに追加されてからレスポンスを返す
    state
        .convert_manager
        .enqueue(
            repo.to_string(),
            format,
            filename.to_string(),
            None,
            quantization.clone(),
            chat_template.clone(),
        )
        .await;

    tracing::info!(
        repo = %repo,
        format = ?format,
        filename = %filename,
        size_bytes = content_length,
        required_memory_bytes = required_memory,
        warnings = warnings.len(),
        "hf_model_registered"
    );

    let response = serde_json::json!({
        "name": name,
        "status": "registered",
        "format": format,
        "filename": filename,
        "size_bytes": content_length,
        "required_memory_bytes": required_memory,
        "warnings": warnings,
    });

    Ok((StatusCode::CREATED, Json(response)))
}

/// DELETE /v0/models/:model_name - 登録モデル削除
///
/// 以下のいずれかの処理を行う:
/// 1. ダウンロード中/待機中の場合: ConvertTaskをキャンセル
/// 2. 完了済みの場合: ファイルを削除し、登録を解除
pub async fn delete_model(
    State(state): State<AppState>,
    Path(model_name): Path<String>,
) -> Result<StatusCode, AppError> {
    // 1. ConvertTaskがあれば削除
    let tasks = state.convert_manager.list_tasks().await;
    let mut task_deleted = false;
    for task in tasks {
        let task_model_name = generate_model_id(&task.repo);
        if task_model_name == model_name {
            // 任意の状態のタスクを削除（ダウンロード中、待機中、失敗、完了を含む）
            if state.convert_manager.delete(task.id).await {
                task_deleted = true;
                tracing::info!(model_name=%model_name, task_id=?task.id, status=?task.status, "ConvertTask deleted");
            }
        }
    }

    // 2. 登録モデルを削除
    let removed = remove_registered_model(&model_name);

    // 3. ルーター側キャッシュ（モデルディレクトリ）を削除
    if let Some(base) = crate::registry::models::router_models_dir() {
        let dir = base.join(crate::registry::models::model_name_to_dir(&model_name));
        if dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&dir) {
                tracing::warn!("Failed to remove model directory {}: {}", dir.display(), e);
            }
        }
    }

    if removed || task_deleted {
        if removed {
            persist_registered_models(&state.db_pool).await;
        }
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(RouterError::Common(CommonError::Validation("model not found".into())).into())
    }
}

/// GGUF版検索リクエスト
#[derive(Debug, Deserialize)]
pub struct DiscoverGgufRequest {
    /// 検索対象のモデル名（例: openai/gpt-oss-20b または gpt-oss-20b）
    pub model: String,
}

/// GGUF版検索レスポンス
#[derive(Debug, Serialize)]
pub struct DiscoverGgufResponse {
    /// 検索対象のモデル名
    pub base_model: String,
    /// 見つかったGGUF版の一覧（信頼プロバイダー順）
    pub gguf_alternatives: Vec<GgufDiscoveryResult>,
    /// キャッシュから取得したかどうか
    pub cached: bool,
}

/// POST /v0/models/discover-gguf - GGUF版を検索
pub async fn discover_gguf_endpoint(
    State(state): State<AppState>,
    Json(req): Json<DiscoverGgufRequest>,
) -> Result<Json<DiscoverGgufResponse>, AppError> {
    let results = discover_gguf_versions(&state.http_client, &req.model).await?;

    Ok(Json(DiscoverGgufResponse {
        base_model: req.model,
        gguf_alternatives: results,
        cached: false, // TODO: キャッシュヒット判定
    }))
}

// NOTE: /v0/models/convert エンドポイントは廃止されました。
// ダウンロード状態は /v0/models の lifecycle_status で確認できます。

/// GET /v0/models/blob/{model_name} - モデルファイル（GGUF）をストリーミング配信
///
/// ノードがルーターからモデルファイルをHTTP経由でダウンロードするためのエンドポイント。
/// 共有パスにアクセスできない環境でのフォールバック用。
pub async fn get_model_blob(Path(model_name): Path<String>) -> axum::response::Response {
    use axum::body::Body;
    use axum::response::Response;
    use tokio_util::io::ReaderStream;

    // モデル名のバリデーション
    if let Err(e) = validate_model_name(&model_name) {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from(format!("{{\"error\": \"{}\"}}", e)))
            .unwrap();
    }

    // モデルファイルのパスを取得
    let model_path = match router_model_path(&model_name) {
        Some(path) => path,
        None => {
            tracing::warn!("Model not found: {}", model_name);
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from(format!(
                    "{{\"error\": \"Model not found: {}\"}}",
                    model_name
                )))
                .unwrap();
        }
    };

    // ファイルを開く
    let file = match tokio::fs::File::open(&model_path).await {
        Ok(f) => f,
        Err(e) => {
            tracing::error!("Failed to open model file {:?}: {}", model_path, e);
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!(
                    "{{\"error\": \"Failed to open model file: {}\"}}",
                    e
                )))
                .unwrap();
        }
    };

    // ファイルサイズを取得
    let file_size = match file.metadata().await {
        Ok(m) => m.len(),
        Err(e) => {
            tracing::error!("Failed to get file metadata: {}", e);
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!(
                    "{{\"error\": \"Failed to get file metadata: {}\"}}",
                    e
                )))
                .unwrap();
        }
    };

    // ストリーミングレスポンスを構築
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    tracing::info!(
        "Streaming model blob: model={}, size={}",
        model_name,
        file_size
    );

    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/octet-stream")
        .header("content-length", file_size.to_string())
        .header("content-disposition", "attachment; filename=\"model.gguf\"")
        .body(body)
        .unwrap()
}

/// GET /v0/models/registry/:model_name/manifest.json - モデル配布マニフェスト
///
/// Node がモデルを複数ファイル（safetensors + metadata）として取得するためのマニフェスト。
pub async fn get_model_registry_manifest(
    Path(model_name): Path<String>,
) -> axum::response::Response {
    use axum::body::Body;
    use axum::response::Response;

    if let Err(e) = validate_model_name(&model_name) {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from(format!("{{\"error\": \"{}\"}}", e)))
            .unwrap();
    }

    let Some(base) = crate::registry::models::router_models_dir() else {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("{\"error\": \"HOME not set\"}"))
            .unwrap();
    };
    let dir = base.join(crate::registry::models::model_name_to_dir(&model_name));
    if !dir.exists() {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(format!(
                "{{\"error\": \"Model not found: {}\"}}",
                model_name
            )))
            .unwrap();
    }

    #[derive(Serialize)]
    struct ManifestFile {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        priority: Option<i32>,
    }
    #[derive(Serialize)]
    struct Manifest {
        files: Vec<ManifestFile>,
    }

    let mut files: Vec<ManifestFile> = Vec::new();
    if let Ok(mut rd) = tokio::fs::read_dir(&dir).await {
        while let Ok(Some(entry)) = rd.next_entry().await {
            let Ok(meta) = entry.metadata().await else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(|s| s.to_string()) else {
                continue;
            };

            // 優先: config/tokenizer は先に取得したい
            let priority = match name.as_str() {
                "config.json" | "tokenizer.json" => Some(10),
                _ => None,
            };

            files.push(ManifestFile { name, priority });
        }
    }

    // 安定順序（テストしやすいように）
    files.sort_by(|a, b| a.name.cmp(&b.name));

    let body =
        serde_json::to_string(&Manifest { files }).unwrap_or_else(|_| "{\"files\":[]}".into());
    Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap()
}

/// GET /v0/models/registry/:model_name/files/:file_name - モデル配布ファイル
pub async fn get_model_registry_file(
    Path((model_name, file_name)): Path<(String, String)>,
) -> axum::response::Response {
    use axum::body::Body;
    use axum::response::Response;
    use tokio_util::io::ReaderStream;

    if let Err(e) = validate_model_name(&model_name) {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from(format!("{{\"error\": \"{}\"}}", e)))
            .unwrap();
    }

    // file_name は単一セグメント前提（念のためパストラバーサルを禁止）
    if file_name.is_empty()
        || file_name.contains("..")
        || file_name.contains('/')
        || file_name.contains('\\')
        || file_name.contains('\0')
    {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("{\"error\": \"Invalid file name\"}"))
            .unwrap();
    }

    let Some(base) = crate::registry::models::router_models_dir() else {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("{\"error\": \"HOME not set\"}"))
            .unwrap();
    };
    let dir = base.join(crate::registry::models::model_name_to_dir(&model_name));
    let path = dir.join(&file_name);
    if !path.exists() {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("{\"error\": \"File not found\"}"))
            .unwrap();
    }

    let file = match tokio::fs::File::open(&path).await {
        Ok(f) => f,
        Err(e) => {
            tracing::error!("Failed to open model registry file {:?}: {}", path, e);
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("{\"error\": \"Failed to open file\"}"))
                .unwrap();
        }
    };

    let stream = ReaderStream::new(file);
    Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::from_stream(stream))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_router_common::{protocol::RegisterRequest, types::GpuDeviceInfo};

    #[test]
    fn test_validate_model_name_valid() {
        assert!(validate_model_name("gpt-oss").is_ok());
        assert!(validate_model_name("gpt-oss-7b").is_ok());
        assert!(validate_model_name("llama3.2").is_ok());
        assert!(validate_model_name("model_name-v1.0").is_ok());
    }

    #[test]
    fn test_validate_model_name_hierarchical_valid() {
        // SPEC-dcaeaec4 FR-2: 階層形式を許可
        assert!(validate_model_name("openai/gpt-oss-20b").is_ok());
        assert!(validate_model_name("meta/llama-3-8b").is_ok());
        assert!(validate_model_name("org/sub/model").is_ok());
    }

    #[test]
    fn test_validate_model_name_empty() {
        assert!(validate_model_name("").is_err());
    }

    #[test]
    fn test_validate_model_name_colon_rejected() {
        assert!(validate_model_name("llama3.2:latest").is_err());
    }

    #[test]
    fn test_validate_model_name_invalid_characters() {
        assert!(validate_model_name("Model Name").is_err());
        assert!(validate_model_name("model@name").is_err());
    }

    #[test]
    fn test_validate_model_name_dangerous_patterns_rejected() {
        // パストラバーサル対策
        assert!(validate_model_name("../etc/passwd").is_err());
        assert!(validate_model_name("model/../other").is_err());
        assert!(validate_model_name("/absolute/path").is_err());
        assert!(validate_model_name("trailing/").is_err());
    }

    // NOTE: AvailableModelView, AvailableModelsResponse, model_info_to_view は廃止
    // HFカタログは直接 https://huggingface.co を参照 (Phase 1で削除)

    #[tokio::test]
    async fn test_compute_gpu_warnings_detects_insufficient_memory() {
        let registry = NodeRegistry::new();
        let req = RegisterRequest {
            machine_name: "node-1".into(),
            ip_address: "127.0.0.1".parse().unwrap(),
            runtime_version: "0.1.0".into(),
            runtime_port: 11434,
            gpu_available: true,
            gpu_devices: vec![GpuDeviceInfo {
                model: "Test GPU".into(),
                count: 1,
                memory: Some(4 * 1024 * 1024 * 1024),
            }],
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".into()),
            supported_runtimes: Vec::new(),
        };
        registry.register(req).await.unwrap();

        let warnings = compute_gpu_warnings(&registry, 6 * 1024 * 1024 * 1024).await;
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("max node GPU memory"));
    }

    #[tokio::test]
    async fn test_get_model_blob_returns_file_when_exists() {
        use tempfile::tempdir;

        // テンポラリディレクトリにモデルファイルを作成
        // router_models_dir() は ~/.llm-router/models を返すため、
        // temp_dir/.llm-router/models/gpt-oss-7b/model.gguf の構造が必要
        // （新形式ではディレクトリ名 = モデル名、コロン無しの場合はそのまま）
        let temp_dir = tempdir().expect("temp dir");
        let models_dir = temp_dir.path().join(".llm-router").join("models");
        let model_dir = models_dir.join("gpt-oss-7b");
        std::fs::create_dir_all(&model_dir).expect("create model dir");

        // テスト用のGGUFファイルを作成（GGUFマジックナンバー付き）
        let model_path = model_dir.join("model.gguf");
        let gguf_header = b"GGUF\x03\x00\x00\x00"; // GGUF magic + version
        std::fs::write(&model_path, gguf_header).expect("write test file");

        // 環境変数を設定してモデルディレクトリを指定
        std::env::set_var("HOME", temp_dir.path());

        // router_model_path が正しいパスを返すことを確認
        let result = router_model_path("gpt-oss-7b");
        assert!(result.is_some(), "router_model_path should return Some");
        assert!(result.unwrap().exists(), "model file should exist");
    }

    #[tokio::test]
    async fn test_get_model_blob_returns_none_when_not_found() {
        use tempfile::tempdir;

        let temp_dir = tempdir().expect("temp dir");
        std::env::set_var("HOME", temp_dir.path());

        // 存在しないモデルの場合はNoneを返す
        let result = router_model_path("nonexistent-model-7b");
        assert!(
            result.is_none(),
            "router_model_path should return None for nonexistent model"
        );
    }
}
