//! モデル管理API
//!
//! モデル一覧取得、登録、変換、ファイル配信のエンドポイント

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use futures::StreamExt;
use once_cell::sync::Lazy;
use reqwest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::{
    convert::ConvertTask,
    registry::models::{
        generate_ollama_style_id, model_name_to_dir, router_model_path, router_models_dir,
        ModelInfo, ModelSource,
    },
    registry::NodeRegistry,
    AppState,
};
use llm_router_common::error::{CommonError, RouterError};

/// モデル名の妥当性を検証
///
/// 有効なモデル名の形式:
/// - `gpt-oss-20b`, `mistral-7b-instruct-v0.2` のようなファイル名ベース形式
fn validate_model_name(model_name: &str) -> Result<(), RouterError> {
    if model_name.is_empty() {
        return Err(RouterError::InvalidModelName(
            "Model name is empty".to_string(),
        ));
    }

    // ファイル名ベース形式（`:` や `/` は禁止）
    if !model_name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_' || c == '.')
    {
        return Err(RouterError::InvalidModelName(format!(
            "Invalid model name: {}",
            model_name
        )));
    }

    Ok(())
}

/// 利用可能なモデル一覧のレスポンスDTO
#[derive(Debug, Serialize)]
pub struct AvailableModelView {
    /// モデルID（例: gpt-oss-20b）
    pub name: String,
    /// UI表示名
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// 説明文
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// タグの一覧
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// GB単位のサイズ
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_gb: Option<f64>,
    /// 推奨GPUメモリ(GB)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_memory_gb: Option<f64>,
    /// Hugging Face repo (hfソースの場合)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// Hugging Face filename (hfソースの場合)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    /// ダウンロードURL (hfソースの場合)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    /// 量子化情報 (推定)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantization: Option<String>,
}

/// 利用可能なモデル一覧レスポンス
#[derive(Debug, Serialize)]
pub struct AvailableModelsResponse {
    /// モデル一覧（UI表示用に整形済み）
    pub models: Vec<AvailableModelView>,
    /// ソース（"builtin" / "hf" など）
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// キャッシュヒットかどうか
    pub cached: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// ページネーション情報
    pub pagination: Option<Pagination>,
}

#[derive(Debug, Serialize)]
/// ページネーション情報
pub struct Pagination {
    /// 取得件数
    pub limit: u32,
    /// オフセット
    pub offset: u32,
    /// 総件数（未取得時は0）
    pub total: u32,
}

fn model_info_to_view(model: ModelInfo) -> AvailableModelView {
    let size_gb = if model.size > 0 {
        Some((model.size as f64) / (1024.0 * 1024.0 * 1024.0))
    } else {
        None
    };
    let required_memory_gb = if model.required_memory > 0 {
        Some((model.required_memory as f64) / (1024.0 * 1024.0 * 1024.0))
    } else {
        None
    };
    let display_name = Some(model.name.clone());

    AvailableModelView {
        name: model.name,
        display_name,
        description: Some(model.description),
        tags: Some(model.tags),
        size_gb,
        required_memory_gb,
        repo: model.repo,
        filename: model.filename.clone(),
        download_url: model.download_url.clone(),
        quantization: model.filename.as_deref().and_then(extract_quantization),
    }
}

#[derive(Debug, Serialize)]
/// 登録モデル一覧をUIに返すビュー
pub struct RegisteredModelView {
    /// モデルID（例: gpt-oss-20b）
    pub name: String,
    /// 表示用説明
    pub description: Option<String>,
    /// 登録ステータス（registered/cached/failedなど）
    pub status: Option<String>,
    /// ルーターにモデルファイルが存在するか
    pub ready: bool,
    /// ルーター上のパス
    pub path: Option<String>,
    /// 元のダウンロードURL（存在する場合）
    pub download_url: Option<String>,
    /// ソース（hf/predefinedなど）
    pub source: Option<String>,
    /// HFリポジトリ
    pub repo: Option<String>,
    /// HFファイル名
    pub filename: Option<String>,
    /// サイズ(GB)
    pub size_gb: Option<f64>,
    /// 必要メモリ(GB)
    pub required_memory_gb: Option<f64>,
    /// タグ
    pub tags: Vec<String>,
}

fn model_info_to_registered_view(model: ModelInfo) -> RegisteredModelView {
    let path = model
        .path
        .as_ref()
        .map(std::path::PathBuf::from)
        .filter(|p| p.exists())
        .or_else(|| router_model_path(&model.name));
    let ready = path.is_some();
    let size_gb = if model.size > 0 {
        Some((model.size as f64) / (1024.0 * 1024.0 * 1024.0))
    } else {
        None
    };
    let required_memory_gb = if model.required_memory > 0 {
        Some((model.required_memory as f64) / (1024.0 * 1024.0 * 1024.0))
    } else {
        None
    };

    RegisteredModelView {
        name: model.name,
        description: Some(model.description),
        status: model.status,
        ready,
        path: path.map(|p| p.to_string_lossy().to_string()),
        download_url: model.download_url,
        source: Some(format!("{:?}", model.source)).map(|s| s.to_lowercase()),
        repo: model.repo,
        filename: model.filename,
        size_gb,
        required_memory_gb,
        tags: model.tags,
    }
}

// ===== Registered model store (in-memory) =====
static REGISTERED_MODELS: Lazy<RwLock<Vec<ModelInfo>>> = Lazy::new(|| RwLock::new(Vec::new()));

/// 登録済みモデルをストレージからロード
pub async fn load_registered_models_from_storage() {
    if let Ok(models) = crate::db::models::load_models().await {
        let mut store = REGISTERED_MODELS.write().unwrap();
        *store = models;
    }
}

/// 登録モデルの状態を返す（ダウンロード完了判定含む）
pub async fn get_registered_models() -> Result<Json<Vec<RegisteredModelView>>, AppError> {
    let list: Vec<RegisteredModelView> = list_registered_models()
        .into_iter()
        .map(model_info_to_registered_view)
        .collect();
    Ok(Json(list))
}

/// 登録済みモデル一覧を取得
pub fn list_registered_models() -> Vec<ModelInfo> {
    REGISTERED_MODELS.read().unwrap().clone()
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
pub async fn persist_registered_models() {
    if let Ok(store) = std::panic::catch_unwind(|| REGISTERED_MODELS.read().unwrap().clone()) {
        if let Err(e) = crate::db::models::save_models(&store).await {
            tracing::error!("Failed to persist registered models: {}", e);
        }
    }
}

// ===== HF available cache =====
#[derive(Clone)]
struct HfCache {
    fetched_at: Instant,
    models: Vec<ModelInfo>,
}

static HF_CACHE: Lazy<RwLock<Option<HfCache>>> = Lazy::new(|| RwLock::new(None));

const HF_CACHE_TTL: Duration = Duration::from_secs(300);

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

/// リポジトリ内のGGUFファイルを解決
async fn resolve_first_gguf_in_repo(
    http_client: &reqwest::Client,
    repo: &str,
) -> Result<String, RouterError> {
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
    let filename = detail
        .siblings
        .iter()
        .map(|s| s.rfilename.clone())
        .find(|f| f.to_ascii_lowercase().ends_with(".gguf"))
        .ok_or_else(|| {
            RouterError::Common(CommonError::Validation(
                "No GGUF file found in repository".into(),
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
    let patterns = [
        "Q8_0", "Q6_K", "Q5_K_M", "Q5_K_S", "Q5_0", "Q4_K_M", "Q4_K_S", "Q4_0", "Q3_K_M", "Q3_K_S",
        "Q2_K", "IQ4_XS", "IQ3_M", "IQ2_M", "F16", "F32", "BF16",
    ];
    let upper = filename.to_uppercase();
    for pat in patterns {
        if upper.contains(pat) {
            return Some(pat.to_string());
        }
    }
    None
}

/// テストやリカバリ用途でHFカタログキャッシュを強制クリアするユーティリティ。
pub fn clear_hf_cache() {
    *HF_CACHE.write().unwrap() = None;
}

/// 登録モデルのインメモリキャッシュをクリア（テスト用）
pub fn clear_registered_models() {
    *REGISTERED_MODELS.write().unwrap() = Vec::new();
}

#[derive(Deserialize)]
/// HFカタログクエリ
pub struct AvailableQuery {
    /// 部分一致検索
    pub search: Option<String>,
    /// 取得件数
    pub limit: Option<u32>,
    /// オフセット
    pub offset: Option<u32>,
    /// ソース指定（hf/builtin）
    pub source: Option<String>,
}

#[derive(Deserialize)]
struct HfModel {
    #[serde(rename = "modelId", alias = "id")]
    model_id: String,
    tags: Option<Vec<String>>,
    #[serde(default)]
    siblings: Vec<HfSibling>,
    #[serde(rename = "lastModified")]
    last_modified: Option<String>,
}

#[derive(Deserialize)]
struct HfSibling {
    #[serde(rename = "rfilename")]
    rfilename: String,
    size: Option<u64>,
    #[serde(default)]
    lfs: Option<HfLfs>,
}

#[derive(Deserialize)]
struct HfLfs {
    size: Option<u64>,
}

async fn fetch_hf_models(
    http_client: &reqwest::Client,
    query: &AvailableQuery,
) -> Result<(Vec<ModelInfo>, bool), RouterError> {
    // cache hit
    if query.search.is_none() {
        if let Some(cache) = HF_CACHE.read().unwrap().as_ref() {
            if cache.fetched_at.elapsed() < HF_CACHE_TTL {
                return Ok((cache.models.clone(), true));
            }
        }
    }

    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);
    // HFの一覧APIはlibraryフィルタではGGUFを返さない場合があるため、
    // デフォルト検索語に"gguf"を入れてGGUFリポジトリを優先的に取得する
    let search = query.search.clone().unwrap_or_else(|| "gguf".to_string());

    let token = std::env::var("HF_TOKEN").ok();
    let base_url = std::env::var("HF_BASE_URL")
        .unwrap_or_else(|_| "https://huggingface.co".to_string())
        .trim_end_matches('/')
        .to_string();
    let url = format!(
        "{}/api/models?limit={limit}&offset={offset}&search={search}&fields=id,modelId,tags,lastModified,siblings&expand=siblings",
        base_url
    );

    let mut req = http_client.get(url);
    if let Some(t) = token {
        req = req.bearer_auth(t);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| RouterError::Http(e.to_string()))?;

    // HF API がレートリミット/障害を返した場合はキャッシュをフォールバックとして返す
    if !resp.status().is_success() {
        if let Some(cache) = HF_CACHE.read().unwrap().as_ref() {
            tracing::warn!(
                "HF API returned {}, serving cached available models",
                resp.status()
            );
            return Ok((cache.models.clone(), true));
        }
        return Err(RouterError::Http(resp.status().to_string()));
    }
    let models_raw: Vec<HfModel> = resp
        .json()
        .await
        .map_err(|e| RouterError::Http(e.to_string()))?;

    #[derive(Clone)]
    struct Candidate {
        model: ModelInfo,
        rank: usize,
    }

    fn quantization_rank(q: Option<&str>) -> usize {
        // Prefer common llama.cpp defaults. Lower is better.
        match q {
            Some("Q4_K_M") => 0,
            Some("Q5_K_M") => 1,
            Some("Q4_K_S") => 2,
            Some("Q5_K_S") => 3,
            Some("Q4_0") => 4,
            Some("Q5_0") => 5,
            Some("Q6_K") => 6,
            Some("Q8_0") => 7,
            Some("F16") => 8,
            Some("BF16") => 9,
            Some("F32") => 10,
            _ => 100,
        }
    }

    let mut best_by_id: HashMap<String, Candidate> = HashMap::new();
    for m in models_raw {
        // pick gguf siblings
        for sib in m.siblings {
            if !sib.rfilename.ends_with(".gguf") {
                continue;
            }

            let model_id = generate_ollama_style_id(&sib.rfilename, &m.model_id);
            let size = sib
                .size
                .or_else(|| sib.lfs.as_ref().and_then(|l| l.size))
                .unwrap_or(0);
            let download_url =
                format!("{}/{}/resolve/main/{}", base_url, m.model_id, sib.rfilename);
            let tags = m.tags.clone().unwrap_or_default();
            let last_modified = m
                .last_modified
                .as_ref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc));

            let quantization = extract_quantization(&sib.rfilename);
            let rank = quantization_rank(quantization.as_deref());

            let candidate = Candidate {
                model: ModelInfo {
                    name: model_id.clone(),
                    size,
                    description: m.model_id.clone(),
                    required_memory: size,
                    tags,
                    source: ModelSource::HfGguf,
                    download_url: Some(download_url),
                    path: None,
                    chat_template: None,
                    repo: Some(m.model_id.clone()),
                    filename: Some(sib.rfilename.clone()),
                    last_modified,
                    status: Some("available".into()),
                },
                rank,
            };

            best_by_id
                .entry(model_id)
                .and_modify(|existing| {
                    let replace = candidate.rank < existing.rank
                        || (candidate.rank == existing.rank
                            && candidate.model.size < existing.model.size);
                    if replace {
                        *existing = candidate.clone();
                    }
                })
                .or_insert(candidate);
        }
    }

    let mut out: Vec<ModelInfo> = best_by_id.into_values().map(|c| c.model).collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));

    if query.search.is_none() {
        *HF_CACHE.write().unwrap() = Some(HfCache {
            fetched_at: Instant::now(),
            models: out.clone(),
        });
    }

    Ok((out, false))
}

/// HFからモデルをpullするリクエスト
#[derive(Debug, Deserialize)]
pub struct PullFromHfRequest {
    /// HFリポジトリ名 (例: TheBloke/gpt-oss-GGUF)
    pub repo: String,
    /// ファイル名 (例: gpt-oss-20b.Q4_K_M.gguf)
    pub filename: String,
    /// オプションのchat_template
    #[serde(default)]
    pub chat_template: Option<String>,
}

/// HF pull APIのレスポンス
#[derive(Debug, Serialize)]
pub struct PullFromHfResponse {
    /// 登録名（モデルID）
    pub name: String,
    /// ルーター側にキャッシュされたローカルパス
    pub path: String,
}

/// モデル変換リクエスト
#[derive(Debug, Deserialize)]
pub struct ConvertModelRequest {
    /// HFリポジトリ (e.g., TheBloke/Llama-2-7B-GGUF)
    pub repo: String,
    /// ファイル名 (e.g., llama-2-7b.Q4_K_M.gguf)
    pub filename: String,
    /// リビジョン（任意, default main）
    #[serde(default)]
    pub revision: Option<String>,
    /// 量子化指定（現状未使用。将来拡張用）
    #[serde(default)]
    pub quantization: Option<String>,
    /// オプションのchat_template
    #[serde(default)]
    pub chat_template: Option<String>,
}

/// モデル変換レスポンス
#[derive(Debug, Serialize)]
pub struct ConvertModelResponse {
    /// ジョブID
    pub task_id: Uuid,
    /// ステータス文字列
    pub status: String,
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
            RouterError::AgentNotFound(_) => (StatusCode::NOT_FOUND, self.0.to_string()),
            RouterError::NoAgentsAvailable => (StatusCode::SERVICE_UNAVAILABLE, self.0.to_string()),
            RouterError::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg.clone()),
            RouterError::AgentOffline(_) => (StatusCode::SERVICE_UNAVAILABLE, self.0.to_string()),
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

/// T027: GET /api/models/available - 利用可能なモデル一覧を取得
pub async fn get_available_models(
    State(state): State<AppState>,
    Query(query): Query<AvailableQuery>,
) -> Result<Json<AvailableModelsResponse>, AppError> {
    // Default to built-in model list for backward compatibility.
    // Hugging Face catalog is available via `source=hf` query parameter.
    let source = query.source.clone().unwrap_or_else(|| "builtin".into());

    if source == "hf" {
        tracing::debug!("Fetching available models from Hugging Face");
        let (models, cached) = fetch_hf_models(&state.http_client, &query).await?;
        let models_view = models.into_iter().map(model_info_to_view).collect();
        return Ok(Json(AvailableModelsResponse {
            models: models_view,
            source: "hf".to_string(),
            cached: Some(cached),
            pagination: Some(Pagination {
                limit: query.limit.unwrap_or(20),
                offset: query.offset.unwrap_or(0),
                total: 0,
            }),
        }));
    }

    tracing::debug!("Builtin models are disabled; returning empty list");
    Ok(Json(AvailableModelsResponse {
        models: Vec::new(),
        source: "builtin".to_string(),
        cached: None,
        pagination: None,
    }))
}

#[derive(Deserialize)]
/// HFモデル登録リクエスト
pub struct RegisterModelRequest {
    /// HFリポジトリ名 (e.g., TheBloke/Llama-2-7B-GGUF)
    pub repo: String,
    /// ファイル名 (e.g., llama-2-7b.Q4_K_M.gguf)
    pub filename: Option<String>,
    /// 表示名（任意）
    #[serde(default)]
    pub display_name: Option<String>,
    /// オプションのchat_template（GGUFに含まれない場合の補助）
    #[serde(default)]
    pub chat_template: Option<String>,
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

/// POST /api/models/register - HF GGUFを対応モデルに登録
///
/// 新しい方針:
/// - ユーザー指定リポジトリにGGUFがあれば使用
/// - なければそのリポジトリのモデルをGGUFに変換
/// - 他リポジトリからのGGUF自動取得は行わない
pub async fn register_model(
    State(state): State<AppState>,
    Json(req): Json<RegisterModelRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let repo = req.repo.clone();

    // ファイル名を解決
    let filename = match req.filename.clone() {
        Some(f) => {
            // ファイル名が指定されている場合はそのまま使用
            // （GGUFでない場合は後で変換される）
            f
        }
        None => {
            // ファイル名指定なし - リポジトリ内のGGUFを探す
            match resolve_first_gguf_in_repo(&state.http_client, &repo).await {
                Ok(f) => f,
                Err(_) => {
                    // リポジトリ内にGGUFがない → 変換対象として空文字列
                    // （convert_managerが適切に処理する）
                    tracing::info!(repo = %repo, "No GGUF in repo, will attempt conversion");
                    String::new()
                }
            }
        }
    };

    register_model_internal(
        &state,
        &repo,
        &filename,
        req.display_name.clone(),
        req.chat_template.clone(),
    )
    .await
}

/// モデル登録の内部実装
async fn register_model_internal(
    state: &AppState,
    repo: &str,
    filename: &str,
    _display_name: Option<String>,
    chat_template: Option<String>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    // モデルIDはファイル名ベース形式に正規化する (SPEC-dcaeaec4 / SPEC-8ae67d67)
    let name = generate_ollama_style_id(filename, repo);

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

    // GGUFファイル名が空の場合は変換パスに進む（HEADチェックをスキップ）
    let (content_length, required_memory, warnings) = if filename.is_empty() {
        tracing::info!(repo = %repo, "No GGUF file specified, proceeding with conversion path");
        (0_u64, 0_u64, vec![])
    } else {
        let base_url = std::env::var("HF_BASE_URL")
            .unwrap_or_else(|_| "https://huggingface.co".to_string())
            .trim_end_matches('/')
            .to_string();
        let download_url = format!("{}/{}/resolve/main/{}", base_url, repo, filename);

        // HEADで存在確認（404時は明示的に返す）
        let mut head = state.http_client.head(&download_url);
        if let Ok(token) = std::env::var("HF_TOKEN") {
            head = head.bearer_auth(token);
        }
        let head_res = head
            .send()
            .await
            .map_err(|e| RouterError::Http(e.to_string()))?;
        if head_res.status() == reqwest::StatusCode::NOT_FOUND {
            tracing::warn!(
                repo = %repo,
                filename = %filename,
                status = ?head_res.status(),
                "hf_model_register_not_found"
            );
            return Err(RouterError::Common(CommonError::Validation(
                "Specified GGUF file not found".into(),
            ))
            .into());
        }
        if !head_res.status().is_success() {
            tracing::error!(
                repo = %repo,
                filename = %filename,
                status = ?head_res.status(),
                "hf_model_register_head_failed"
            );
            return Err(RouterError::Http(head_res.status().to_string()).into());
        }

        let content_length = head_res
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        // llama.cpp runtimeは概ねサイズの1.5倍のメモリを使用するため同倍率で推定
        const REQUIRED_MEMORY_RATIO: f64 = 1.5;
        let required_memory = if content_length > 0 {
            ((content_length as f64) * REQUIRED_MEMORY_RATIO).ceil() as u64
        } else {
            0
        };

        let warnings = compute_gpu_warnings(&state.registry, required_memory).await;
        (content_length, required_memory, warnings)
    };

    // NOTE: モデル登録は ConvertTask 完了時に finalize_model_registration() で行う
    // ここでは REGISTERED_MODELS に追加しない（UI上の重複を防ぐため）

    // コンバートキューへ投入（GGUFは即完了、非GGUFはconvert）
    // 重複チェックのためenqueueはawaitして、タスクがキューに追加されてからレスポンスを返す
    state
        .convert_manager
        .enqueue(
            repo.to_string(),
            filename.to_string(),
            None,
            None,
            chat_template.clone(),
        )
        .await;

    tracing::info!(
        repo = %repo,
        filename = %filename,
        size_bytes = content_length,
        required_memory_bytes = required_memory,
        warnings = warnings.len(),
        "hf_model_registered"
    );

    let response = serde_json::json!({
        "name": name,
        "status": "registered",
        "size_bytes": content_length,
        "required_memory_bytes": required_memory,
        "warnings": warnings,
    });

    Ok((StatusCode::CREATED, Json(response)))
}

/// POST /api/models/pull - HFからダウンロードしローカルキャッシュに保存して登録
pub async fn pull_model_from_hf(
    State(state): State<AppState>,
    Json(req): Json<PullFromHfRequest>,
) -> Result<(StatusCode, Json<PullFromHfResponse>), AppError> {
    let name = generate_ollama_style_id(&req.filename, &req.repo);
    validate_model_name(&name)?;

    let base_url = std::env::var("HF_BASE_URL")
        .unwrap_or_else(|_| "https://huggingface.co".to_string())
        .trim_end_matches('/')
        .to_string();
    let download_url = format!("{}/{}/resolve/main/{}", base_url, req.repo, req.filename);

    let base = router_models_dir().ok_or_else(|| RouterError::Internal("HOME not set".into()))?;
    let dir = base.join(model_name_to_dir(&name));
    let target = dir.join("model.gguf");

    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| RouterError::Internal(e.to_string()))?;

    // ダウンロード
    let resp = state
        .http_client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| RouterError::Http(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(RouterError::Http(resp.status().to_string()).into());
    }

    let mut file = tokio::fs::File::create(&target)
        .await
        .map_err(|e| RouterError::Internal(e.to_string()))?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| RouterError::Http(e.to_string()))?;
        file.write_all(&bytes)
            .await
            .map_err(|e| RouterError::Internal(e.to_string()))?;
    }

    let mut model = ModelInfo::new(name.clone(), 0, req.repo.clone(), 0, vec!["gguf".into()]);
    model.download_url = Some(download_url.clone());
    model.path = Some(target.to_string_lossy().to_string());
    model.chat_template = req.chat_template.clone();
    model.repo = Some(req.repo.clone());
    model.filename = Some(req.filename.clone());
    model.status = Some("cached".into());

    if let Ok(meta) = tokio::fs::metadata(&target).await {
        model.size = meta.len();
    }

    add_registered_model(model)?;
    persist_registered_models().await;

    Ok((
        StatusCode::CREATED,
        Json(PullFromHfResponse {
            name,
            path: target.to_string_lossy().to_string(),
        }),
    ))
}

/// DELETE /api/models/:model_name - 登録モデル削除
pub async fn delete_model(Path(model_name): Path<String>) -> Result<StatusCode, AppError> {
    let removed = remove_registered_model(&model_name);

    if let Some(path) = router_model_path(&model_name) {
        if path.exists() {
            if let Err(e) = std::fs::remove_file(&path) {
                tracing::warn!("Failed to remove model file {}: {}", path.display(), e);
            }
            if let Some(parent) = path.parent() {
                if let Err(e) = std::fs::remove_dir_all(parent) {
                    tracing::warn!(
                        "Failed to remove model directory {}: {}",
                        parent.display(),
                        e
                    );
                }
            }
        }
    }

    if removed {
        persist_registered_models().await;
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

/// POST /api/models/discover-gguf - GGUF版を検索
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

/// POST /api/models/convert - HFモデルをダウンロード＆（必要なら）変換するジョブを作成
pub async fn convert_model(
    State(state): State<AppState>,
    Json(req): Json<ConvertModelRequest>,
) -> Result<(StatusCode, Json<ConvertModelResponse>), AppError> {
    let name = generate_ollama_style_id(&req.filename, &req.repo);
    validate_model_name(&name)?;

    let task = state
        .convert_manager
        .enqueue(
            req.repo.clone(),
            req.filename.clone(),
            req.revision.clone(),
            req.quantization.clone(),
            req.chat_template.clone(),
        )
        .await;

    Ok((
        StatusCode::ACCEPTED,
        Json(ConvertModelResponse {
            task_id: task.id,
            status: format!("{:?}", task.status).to_lowercase(),
        }),
    ))
}

/// GET /api/models/convert - 変換ジョブ一覧
pub async fn list_convert_tasks(
    State(state): State<AppState>,
) -> Result<Json<Vec<ConvertTask>>, AppError> {
    let tasks = state.convert_manager.list().await;
    Ok(Json(tasks))
}

/// GET /api/models/convert/{task_id} - 単一ジョブ取得
pub async fn get_convert_task(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> Result<Json<ConvertTask>, AppError> {
    let task = state
        .convert_manager
        .get(task_id)
        .await
        .ok_or_else(|| RouterError::Internal("Task not found".into()))?;
    Ok(Json(task))
}

/// DELETE /api/models/convert/{task_id} - ジョブ削除（×ボタン用）
pub async fn delete_convert_task(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    if state.convert_manager.delete(task_id).await {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(RouterError::Internal("Task not found".into()).into())
    }
}

/// GET /api/models/blob/{model_name} - モデルファイル（GGUF）をストリーミング配信
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
    fn test_available_model_view_serialize() {
        let view = AvailableModelView {
            name: "gpt-oss-7b".to_string(),
            display_name: Some("GPT-OSS 7B".to_string()),
            description: Some("Test model".to_string()),
            tags: Some(vec!["7b".to_string()]),
            size_gb: Some(4.0),
            required_memory_gb: Some(6.0),
            repo: None,
            filename: None,
            download_url: None,
            quantization: None,
        };
        let json = serde_json::to_string(&view).unwrap();
        assert!(json.contains("gpt-oss-7b"));
        assert!(json.contains("GPT-OSS 7B"));
    }

    #[test]
    fn test_available_model_view_optional_fields_skipped() {
        let view = AvailableModelView {
            name: "test".to_string(),
            display_name: None,
            description: None,
            tags: None,
            size_gb: None,
            required_memory_gb: None,
            repo: None,
            filename: None,
            download_url: None,
            quantization: None,
        };
        let json = serde_json::to_string(&view).unwrap();
        assert!(!json.contains("display_name"));
        assert!(!json.contains("description"));
    }

    #[test]
    fn test_available_models_response_serialize() {
        let response = AvailableModelsResponse {
            models: vec![],
            source: "builtin".to_string(),
            cached: None,
            pagination: None,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("builtin"));
    }

    #[test]
    fn test_model_info_to_view_conversion() {
        let mut model = crate::registry::models::ModelInfo::new(
            "gpt-oss-7b".to_string(),
            4 * 1024 * 1024 * 1024,
            "Test model".to_string(),
            6 * 1024 * 1024 * 1024,
            vec!["7b".to_string()],
        );
        model.download_url = None;
        model.repo = None;
        model.filename = None;
        model.last_modified = None;
        model.status = None;
        let view = model_info_to_view(model);
        assert_eq!(view.name, "gpt-oss-7b");
        // 新形式（コロンなし）の場合、display_name はモデル名そのまま
        assert_eq!(view.display_name, Some("gpt-oss-7b".to_string()));
        assert!((view.size_gb.unwrap() - 4.0).abs() < 0.001);
        assert!((view.required_memory_gb.unwrap() - 6.0).abs() < 0.001);
    }

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
