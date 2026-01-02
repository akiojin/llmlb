//! モデル管理API
//!
//! モデル一覧取得、登録、マニフェスト配信のエンドポイント

use crate::{
    db::models::ModelStorage,
    registry::models::{extract_repo_id, generate_model_id, ModelInfo},
    registry::NodeRegistry,
    supported_models::{get_supported_models, SupportedModel},
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
use std::collections::{HashMap, HashSet};
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

/// モデルの状態（SPEC-6cd7f960）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModelStatus {
    /// 対応モデル（未ダウンロード）
    Available,
    /// ダウンロード中
    Downloading,
    /// ダウンロード完了
    Downloaded,
}

/// HuggingFace動的情報
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HfInfo {
    /// ダウンロード数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub downloads: Option<u64>,
    /// いいね数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub likes: Option<u64>,
}

// ===== HuggingFace Info Cache (SPEC-6cd7f960) =====

/// HF情報キャッシュエントリ
#[derive(Clone)]
struct HfInfoCacheEntry {
    fetched_at: Instant,
    info: HfInfo,
}

/// HF情報キャッシュ（TTL: 10分）
static HF_INFO_CACHE: Lazy<RwLock<HashMap<String, HfInfoCacheEntry>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

const HF_INFO_CACHE_TTL: Duration = Duration::from_secs(600); // 10分

/// HuggingFace APIからモデル情報を取得（キャッシュ付き）
async fn fetch_hf_info(http_client: &reqwest::Client, repo: &str) -> Option<HfInfo> {
    // キャッシュチェック
    {
        let cache = HF_INFO_CACHE.read().unwrap();
        if let Some(entry) = cache.get(repo) {
            if entry.fetched_at.elapsed() < HF_INFO_CACHE_TTL {
                return Some(entry.info.clone());
            }
        }
    }

    // HF APIからフェッチ
    let base_url = std::env::var("HF_BASE_URL")
        .unwrap_or_else(|_| "https://huggingface.co".to_string())
        .trim_end_matches('/')
        .to_string();
    let url = format!("{}/api/models/{}", base_url, repo);

    let mut req = http_client.get(&url);
    if let Ok(token) = std::env::var("HF_TOKEN") {
        req = req.bearer_auth(token);
    }

    let resp = match req.timeout(Duration::from_secs(5)).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!(repo = %repo, error = %e, "Failed to fetch HF info");
            return None;
        }
    };

    if !resp.status().is_success() {
        tracing::debug!(repo = %repo, status = ?resp.status(), "HF API returned non-success status");
        return None;
    }

    #[derive(Deserialize)]
    struct HfModelInfo {
        downloads: Option<u64>,
        likes: Option<u64>,
    }

    let model_info: HfModelInfo = match resp.json().await {
        Ok(info) => info,
        Err(e) => {
            tracing::debug!(repo = %repo, error = %e, "Failed to parse HF info");
            return None;
        }
    };

    let info = HfInfo {
        downloads: model_info.downloads,
        likes: model_info.likes,
    };

    // キャッシュに保存
    {
        let mut cache = HF_INFO_CACHE.write().unwrap();
        cache.insert(
            repo.to_string(),
            HfInfoCacheEntry {
                fetched_at: Instant::now(),
                info: info.clone(),
            },
        );
    }

    Some(info)
}

/// 対応モデル + 状態（GET /v0/models レスポンス）
#[derive(Debug, Clone, Serialize)]
pub struct ModelWithStatus {
    /// モデルID
    pub id: String,
    /// 表示名
    pub name: String,
    /// 説明
    pub description: String,
    /// HFリポジトリ
    pub repo: String,
    /// 推奨ファイル名
    pub recommended_filename: String,
    /// ファイルサイズ（バイト）
    pub size_bytes: u64,
    /// 必要メモリ（バイト）
    pub required_memory_bytes: u64,
    /// タグ
    pub tags: Vec<String>,
    /// 能力
    pub capabilities: Vec<String>,
    /// 量子化タイプ
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantization: Option<String>,
    /// パラメータ数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_count: Option<String>,
    /// モデル状態
    pub status: ModelStatus,
    /// ライフサイクル状態（ダウンロード中/完了時のみ）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lifecycle_status: Option<LifecycleStatus>,
    /// ダウンロード進捗（ダウンロード中のみ）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_progress: Option<DownloadProgress>,
    /// HF動的情報
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hf_info: Option<HfInfo>,
}

impl ModelWithStatus {
    /// SupportedModelからModelWithStatusを作成（available状態）
    pub fn from_supported(model: SupportedModel) -> Self {
        Self {
            id: model.id,
            name: model.name,
            description: model.description,
            repo: model.repo,
            recommended_filename: model.recommended_filename,
            size_bytes: model.size_bytes,
            required_memory_bytes: model.required_memory_bytes,
            tags: model.tags,
            capabilities: model.capabilities,
            quantization: model.quantization,
            parameter_count: model.parameter_count,
            status: ModelStatus::Available,
            lifecycle_status: None,
            download_progress: None,
            hf_info: None,
        }
    }
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
/// NOTE: この関数は既存のノード同期用途で維持。ダッシュボードは list_models_with_status() を使用。
pub async fn list_models() -> Json<Vec<ModelInfo>> {
    Json(list_registered_models())
}

/// GET /v0/models/hub - 対応モデル一覧 + 状態（SPEC-6cd7f960）
///
/// ダッシュボードのModel Hub用。全ての対応モデルを状態付きで返す。
/// HF動的情報（ダウンロード数、いいね数）も含む。
pub async fn list_models_with_status(State(state): State<AppState>) -> Json<Vec<ModelWithStatus>> {
    let supported = get_supported_models();
    let registered = list_registered_models();
    // 登録済みモデル名のセット（登録済み判定用）
    let registered_names: std::collections::HashSet<_> =
        registered.iter().map(|m| m.name.clone()).collect();

    // ノードが報告しているreadyモデル名のセット
    let ready_names: std::collections::HashSet<String> = state
        .registry
        .list()
        .await
        .into_iter()
        .flat_map(|node| node.loaded_models)
        .collect();

    // HF情報を並列取得（タイムアウト付き）
    let hf_info_futures: Vec<_> = supported
        .iter()
        .map(|model| {
            let client = state.http_client.clone();
            let repo = model.repo.clone();
            async move { (repo.clone(), fetch_hf_info(&client, &repo).await) }
        })
        .collect();

    let hf_infos: HashMap<String, Option<HfInfo>> = futures::future::join_all(hf_info_futures)
        .await
        .into_iter()
        .collect();

    let mut result: Vec<ModelWithStatus> = Vec::with_capacity(supported.len());

    for model in supported {
        let mut with_status = ModelWithStatus::from_supported(model.clone());

        // モデルIDからmodel_nameを生成（repo形式）
        let model_name = generate_model_id(&model.repo);

        if registered_names.contains(&model_name) {
            with_status.lifecycle_status = Some(LifecycleStatus::Registered);
            if ready_names.contains(&model_name) {
                with_status.status = ModelStatus::Downloaded;
            }
        }

        // HF情報を設定
        if let Some(Some(info)) = hf_infos.get(&model.repo) {
            with_status.hf_info = Some(info.clone());
        }

        result.push(with_status);
    }

    Json(result)
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
/// 2. ノードのロード状態を確認
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

    // 4. ノードロード状態の確認
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

// ===== HuggingFace helpers =====

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArtifactFormat {
    Gguf,
    Safetensors,
}

struct ArtifactSelection {
    format: ArtifactFormat,
    filename: String,
}

fn hf_base_url() -> String {
    std::env::var("HF_BASE_URL")
        .unwrap_or_else(|_| "https://huggingface.co".to_string())
        .trim_end_matches('/')
        .to_string()
}

fn hf_resolve_url(base_url: &str, repo: &str, filename: &str) -> String {
    format!("{}/{}/resolve/main/{}", base_url, repo, filename)
}

async fn fetch_repo_siblings(
    http_client: &reqwest::Client,
    repo: &str,
) -> Result<Vec<HfSibling>, RouterError> {
    let base_url = hf_base_url();
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

async fn fetch_hf_file_bytes(
    http_client: &reqwest::Client,
    repo: &str,
    filename: &str,
) -> Result<Vec<u8>, RouterError> {
    let base_url = hf_base_url();
    let url = hf_resolve_url(&base_url, repo, filename);
    let mut req = http_client.get(&url);
    if let Ok(token) = std::env::var("HF_TOKEN") {
        req = req.bearer_auth(token);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| RouterError::Http(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(RouterError::Common(CommonError::Validation(format!(
            "Failed to fetch file: {}",
            filename
        ))));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| RouterError::Http(e.to_string()))?;
    Ok(bytes.to_vec())
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

fn infer_safetensors_index_from_shard(filename: &str) -> Option<String> {
    if is_safetensors_index_filename(filename) {
        return None;
    }
    if !filename.to_ascii_lowercase().ends_with(".safetensors") {
        return None;
    }

    let (dir, file) = match filename.rsplit_once('/') {
        Some((dir, file)) => (format!("{}/", dir), file),
        None => ("".to_string(), filename),
    };

    let stem = file.strip_suffix(".safetensors")?;
    let (left, total) = stem.rsplit_once("-of-")?;
    if left.is_empty() || total.is_empty() {
        return None;
    }
    if !total.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let (prefix, shard) = left.rsplit_once('-')?;
    if prefix.is_empty() || shard.is_empty() {
        return None;
    }
    if !shard.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    Some(format!("{}{}.safetensors.index.json", dir, prefix))
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

fn resolve_primary_artifact(
    siblings: &[HfSibling],
    filename_hint: Option<String>,
) -> Result<ArtifactSelection, RouterError> {
    if let Some(filename) = filename_hint {
        if !has_sibling(siblings, &filename) {
            return Err(RouterError::Common(CommonError::Validation(
                "Specified file not found in repository".into(),
            )));
        }
        if is_gguf_filename(&filename) {
            return Ok(ArtifactSelection {
                format: ArtifactFormat::Gguf,
                filename,
            });
        }
        if is_safetensors_filename(&filename) {
            require_safetensors_metadata_files(siblings)?;
            let resolved = resolve_safetensors_primary(siblings, Some(filename))?;
            return Ok(ArtifactSelection {
                format: ArtifactFormat::Safetensors,
                filename: resolved,
            });
        }
        return Err(RouterError::Common(CommonError::Validation(
            "filename must be a .gguf or .safetensors file".into(),
        )));
    }

    let ggufs: Vec<_> = siblings
        .iter()
        .filter(|s| is_gguf_filename(&s.rfilename))
        .map(|s| s.rfilename.clone())
        .collect();
    let safetensors: Vec<_> = siblings
        .iter()
        .filter(|s| is_safetensors_filename(&s.rfilename))
        .map(|s| s.rfilename.clone())
        .collect();

    if !ggufs.is_empty() && !safetensors.is_empty() {
        return Err(RouterError::Common(CommonError::Validation(
            "Multiple artifact types found; specify filename".into(),
        )));
    }

    if !ggufs.is_empty() {
        if ggufs.len() == 1 {
            return Ok(ArtifactSelection {
                format: ArtifactFormat::Gguf,
                filename: ggufs[0].clone(),
            });
        }
        return Err(RouterError::Common(CommonError::Validation(
            "Multiple GGUF files found; specify filename".into(),
        )));
    }

    if !safetensors.is_empty() {
        require_safetensors_metadata_files(siblings)?;
        let filename = resolve_safetensors_primary(siblings, None)?;
        return Ok(ArtifactSelection {
            format: ArtifactFormat::Safetensors,
            filename,
        });
    }

    Err(RouterError::Common(CommonError::Validation(
        "No supported model artifacts found (safetensors/gguf)".into(),
    )))
}

fn extract_runtime_from_config(value: &serde_json::Value) -> Option<String> {
    if let Some(arr) = value.get("architectures").and_then(|x| x.as_array()) {
        for a in arr {
            let Some(s) = a.as_str() else { continue };
            if s.contains("GptOss") || s.contains("GPTOSS") {
                return Some("gptoss_cpp".to_string());
            }
            if s.contains("Nemotron") {
                return Some("nemotron_cpp".to_string());
            }
        }
    }

    if let Some(mt) = value.get("model_type").and_then(|x| x.as_str()) {
        let mt = mt.to_ascii_lowercase();
        if mt.contains("gpt_oss") || mt.contains("gptoss") {
            return Some("gptoss_cpp".to_string());
        }
        if mt.contains("nemotron") {
            return Some("nemotron_cpp".to_string());
        }
    }
    None
}

async fn infer_runtime_hint(http_client: &reqwest::Client, repo: &str) -> Option<Vec<String>> {
    if !repo.is_empty() {
        if let Ok(bytes) = fetch_hf_file_bytes(http_client, repo, "config.json").await {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                if let Some(rt) = extract_runtime_from_config(&v) {
                    return Some(vec![rt]);
                }
            }
        }
    }
    None
}

async fn fetch_safetensors_index_shards(
    http_client: &reqwest::Client,
    repo: &str,
    index_filename: &str,
) -> Result<Vec<String>, RouterError> {
    let bytes = fetch_hf_file_bytes(http_client, repo, index_filename).await?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| RouterError::Http(e.to_string()))?;
    let Some(map) = value.get("weight_map").and_then(|v| v.as_object()) else {
        return Err(RouterError::Common(CommonError::Validation(
            "Invalid safetensors index format".into(),
        )));
    };
    let mut shards: std::collections::HashSet<String> = std::collections::HashSet::new();
    for v in map.values() {
        if let Some(s) = v.as_str() {
            shards.insert(s.to_string());
        }
    }
    let mut list: Vec<String> = shards.into_iter().collect();
    list.sort();
    Ok(list)
}

fn find_metal_artifact(siblings: &[HfSibling]) -> Option<String> {
    let candidates = ["model.metal.bin", "metal/model.bin"];
    for name in candidates {
        if has_sibling(siblings, name) {
            return Some(name.to_string());
        }
    }
    None
}

fn append_supported_artifacts(
    files: &mut Vec<ManifestFile>,
    supported: Option<&SupportedModel>,
    base_repo: &str,
    base_url: &str,
    runtime_hint: &Option<Vec<String>>,
) -> Result<(), RouterError> {
    let Some(model) = supported else {
        return Ok(());
    };
    if model.artifacts.is_empty() {
        return Ok(());
    }

    let mut existing: HashSet<String> = files.iter().map(|f| f.name.clone()).collect();
    for artifact in &model.artifacts {
        if artifact.name.trim().is_empty() {
            continue;
        }
        if existing.contains(&artifact.name) {
            continue;
        }

        let url = if let Some(url) = artifact.url.as_ref() {
            url.clone()
        } else if let Some(path) = artifact.path.as_ref() {
            validate_artifact_path(path)?;
            let repo = artifact.repo.as_deref().unwrap_or(base_repo);
            hf_resolve_url(base_url, repo, path)
        } else {
            continue;
        };

        let priority = artifact
            .priority
            .or_else(|| manifest_file_priority(&artifact.name));
        let runtimes = artifact.runtimes.clone().or_else(|| runtime_hint.clone());

        files.push(ManifestFile {
            name: artifact.name.clone(),
            priority,
            runtimes,
            url: Some(url),
            optional: None,
        });
        existing.insert(artifact.name.clone());
    }

    Ok(())
}

fn validate_artifact_path(path: &str) -> Result<(), RouterError> {
    if path.is_empty() {
        return Err(RouterError::Common(CommonError::Validation(
            "filename must not be empty".into(),
        )));
    }
    if path.contains("..") || path.contains('\0') {
        return Err(RouterError::Common(CommonError::Validation(
            "filename contains invalid path segment".into(),
        )));
    }
    if path.starts_with('/') || path.starts_with('\\') {
        return Err(RouterError::Common(CommonError::Validation(
            "filename must be a relative path".into(),
        )));
    }
    Ok(())
}

fn extract_filename_from_hf_url(input: &str) -> Option<String> {
    for marker in ["/resolve/", "/blob/", "/raw/"] {
        if let Some(rest) = input.split(marker).nth(1) {
            let mut parts = rest.splitn(2, '/');
            let _revision = parts.next();
            if let Some(path) = parts.next() {
                if !path.is_empty() {
                    return Some(path.to_string());
                }
            }
        }
    }
    None
}

fn parse_supported_capabilities(caps: &[String]) -> Vec<llm_router_common::types::ModelCapability> {
    use llm_router_common::types::ModelCapability;
    let mut out = Vec::new();
    for cap in caps {
        match cap.as_str() {
            "TextGeneration" => out.push(ModelCapability::TextGeneration),
            "TextToSpeech" => out.push(ModelCapability::TextToSpeech),
            "SpeechToText" => out.push(ModelCapability::SpeechToText),
            "ImageGeneration" => out.push(ModelCapability::ImageGeneration),
            "Vision" => out.push(ModelCapability::Vision),
            "Embedding" => out.push(ModelCapability::Embedding),
            _ => {}
        }
    }
    if out.is_empty() {
        out.push(ModelCapability::TextGeneration);
    }
    out
}

#[derive(Serialize)]
struct ManifestFile {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    priority: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    runtimes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    optional: Option<bool>,
}

#[derive(Serialize)]
struct Manifest {
    format: String,
    files: Vec<ManifestFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    quantization: Option<String>,
}

fn manifest_format_label(format: ArtifactFormat) -> &'static str {
    match format {
        ArtifactFormat::Gguf => "gguf",
        ArtifactFormat::Safetensors => "safetensors",
    }
}

fn manifest_file_priority(name: &str) -> Option<i32> {
    match name {
        "config.json" | "tokenizer.json" => Some(10),
        _ if is_safetensors_index_filename(name) => Some(5),
        "model.metal.bin" => Some(5),
        _ => None,
    }
}

fn infer_quantization_from_filename(filename: &str) -> Option<String> {
    let lower = filename.to_ascii_lowercase();
    if !lower.ends_with(".gguf") {
        return None;
    }
    if filename.len() <= 5 {
        return None;
    }
    let stem = &filename[..filename.len() - 5];
    let idx = stem.rfind(['.', '-'])?;
    if idx + 1 >= stem.len() {
        return None;
    }
    let token = &stem[idx + 1..];
    let mut chars = token.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    if !token.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(token.to_string())
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
        if !is_safetensors_index_filename(&filename) {
            if let Some(candidate) = infer_safetensors_index_from_shard(&filename) {
                if has_sibling(siblings, &candidate) {
                    return Ok(candidate);
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
            }
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

/// POST /v0/models/register - HFモデルを対応モデルに登録（メタデータのみ）
///
/// 方針:
/// - ルーターは変換・バイナリ保存を行わない
/// - `filename` を指定するとそのアーティファクトを主として登録
/// - 未指定の場合、リポジトリ内のアーティファクトが一意であれば自動選択
/// - safetensors では `config.json` / `tokenizer.json` が必須
pub async fn register_model(
    State(state): State<AppState>,
    Json(req): Json<RegisterModelRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    if req.repo.trim().is_empty() {
        return Err(RouterError::Common(CommonError::Validation("repo is required".into())).into());
    }

    // URLからrepo_idを抽出（フルURLが渡された場合はrepo_id形式に正規化）
    let repo = extract_repo_id(&req.repo);

    let name = generate_model_id(&repo);
    if find_model_by_name(&name).is_some() {
        return Err(RouterError::Common(CommonError::Validation(
            "Model already registered".into(),
        ))
        .into());
    }

    let mut filename_hint = req
        .filename
        .clone()
        .or_else(|| extract_filename_from_hf_url(&req.repo));

    if let Some(fname) = filename_hint.as_ref() {
        validate_artifact_path(fname)?;
    }

    if filename_hint.is_none() {
        if let Some(supported) = get_supported_models()
            .into_iter()
            .find(|m| m.repo.eq_ignore_ascii_case(&repo))
        {
            filename_hint = Some(supported.recommended_filename);
        }
    }

    let siblings = fetch_repo_siblings(&state.http_client, &repo).await?;
    let selection = resolve_primary_artifact(&siblings, filename_hint)?;

    let (content_length, required_memory, warnings) = {
        let (size, required) = match selection.format {
            ArtifactFormat::Gguf => {
                let size = siblings
                    .iter()
                    .find(|s| s.rfilename == selection.filename)
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
            ArtifactFormat::Safetensors => {
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
        let warnings = compute_gpu_warnings(&state.registry, required).await;
        (size, required, warnings)
    };

    let chat_template = if req.chat_template.is_some() {
        req.chat_template.clone()
    } else {
        fetch_chat_template_from_hf(&state.http_client, &repo).await
    };

    let supported = get_supported_models()
        .into_iter()
        .find(|m| m.repo.eq_ignore_ascii_case(&repo));

    let (description, tags, capabilities, size_bytes, required_memory_bytes) =
        if let Some(m) = supported.clone() {
            (
                m.description,
                m.tags,
                parse_supported_capabilities(&m.capabilities),
                m.size_bytes.max(content_length),
                m.required_memory_bytes.max(required_memory),
            )
        } else {
            let mut tags = Vec::new();
            match selection.format {
                ArtifactFormat::Gguf => tags.push("gguf".to_string()),
                ArtifactFormat::Safetensors => tags.push("safetensors".to_string()),
            }
            (
                req.display_name.clone().unwrap_or_else(|| repo.clone()),
                tags,
                vec![llm_router_common::types::ModelCapability::TextGeneration],
                content_length,
                required_memory,
            )
        };

    let source = match selection.format {
        ArtifactFormat::Gguf => crate::registry::models::ModelSource::HfGguf,
        ArtifactFormat::Safetensors => crate::registry::models::ModelSource::HfSafetensors,
    };

    let model = ModelInfo {
        name: name.clone(),
        size: size_bytes,
        description,
        required_memory: required_memory_bytes,
        tags,
        capabilities,
        source,
        chat_template,
        repo: Some(repo.clone()),
        filename: Some(selection.filename.clone()),
        last_modified: None,
        status: Some("registered".to_string()),
    };

    add_registered_model(model)?;
    persist_registered_models(&state.db_pool).await;

    tracing::info!(
        repo = %repo,
        filename = %selection.filename,
        size_bytes = content_length,
        required_memory_bytes = required_memory,
        warnings = warnings.len(),
        "hf_model_registered"
    );

    let response = serde_json::json!({
        "name": name,
        "status": "registered",
        "filename": selection.filename,
        "size_bytes": content_length,
        "required_memory_bytes": required_memory,
        "warnings": warnings,
    });

    Ok((StatusCode::CREATED, Json(response)))
}

/// DELETE /v0/models/:model_name - 登録モデル削除
///
/// 登録情報のみ削除し、Nodeは次回同期でキャッシュを削除する。
pub async fn delete_model(
    State(state): State<AppState>,
    Path(model_name): Path<String>,
) -> Result<StatusCode, AppError> {
    let removed = remove_registered_model(&model_name);
    if removed {
        persist_registered_models(&state.db_pool).await;
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(RouterError::Common(CommonError::Validation("model not found".into())).into())
    }
}

/// GET /v0/models/registry/:model_name/manifest.json - モデル配布マニフェスト
///
/// Node がモデルを複数ファイル（safetensors + metadata）として取得するためのマニフェスト。
pub async fn get_model_registry_manifest(
    State(state): State<AppState>,
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

    let model = match list_registered_models()
        .into_iter()
        .find(|m| m.name == model_name)
    {
        Some(m) => m,
        None => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from(format!(
                    "{{\"error\": \"Model not found: {}\"}}",
                    model_name
                )))
                .unwrap();
        }
    };

    let Some(repo) = model.repo.clone() else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("{\"error\": \"repo not set for model\"}"))
            .unwrap();
    };

    let siblings = match fetch_repo_siblings(&state.http_client, &repo).await {
        Ok(list) => list,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from(format!("{{\"error\": \"{}\"}}", e)))
                .unwrap();
        }
    };

    let supported = get_supported_models()
        .into_iter()
        .find(|m| m.repo.eq_ignore_ascii_case(&repo));

    let selection = match resolve_primary_artifact(&siblings, model.filename.clone()) {
        Ok(sel) => sel,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(format!("{{\"error\": \"{}\"}}", e)))
                .unwrap();
        }
    };

    let runtime_hint = match selection.format {
        ArtifactFormat::Gguf => Some(vec!["llama_cpp".to_string()]),
        ArtifactFormat::Safetensors => infer_runtime_hint(&state.http_client, &repo).await,
    };
    let manifest_quantization = match selection.format {
        ArtifactFormat::Gguf => supported
            .as_ref()
            .and_then(|m| m.quantization.clone())
            .or_else(|| infer_quantization_from_filename(&selection.filename)),
        ArtifactFormat::Safetensors => None,
    };

    let base_url = hf_base_url();
    let mut files: Vec<ManifestFile> = Vec::new();

    match selection.format {
        ArtifactFormat::Gguf => {
            files.push(ManifestFile {
                name: "model.gguf".to_string(),
                priority: None,
                runtimes: runtime_hint.clone(),
                url: Some(hf_resolve_url(&base_url, &repo, &selection.filename)),
                optional: None,
            });
        }
        ArtifactFormat::Safetensors => {
            if let Err(e) = require_safetensors_metadata_files(&siblings) {
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(format!("{{\"error\": \"{}\"}}", e)))
                    .unwrap();
            }

            let mut names: Vec<String> =
                vec!["config.json".to_string(), "tokenizer.json".to_string()];
            names.push(selection.filename.clone());

            if is_safetensors_index_filename(&selection.filename) {
                match fetch_safetensors_index_shards(&state.http_client, &repo, &selection.filename)
                    .await
                {
                    Ok(shards) => {
                        for shard in shards {
                            if !names.contains(&shard) {
                                names.push(shard);
                            }
                        }
                    }
                    Err(e) => {
                        return Response::builder()
                            .status(StatusCode::BAD_REQUEST)
                            .body(Body::from(format!("{{\"error\": \"{}\"}}", e)))
                            .unwrap();
                    }
                }
            }

            for name in names {
                files.push(ManifestFile {
                    name: name.clone(),
                    priority: manifest_file_priority(&name),
                    runtimes: runtime_hint.clone(),
                    url: Some(hf_resolve_url(&base_url, &repo, &name)),
                    optional: None,
                });
            }
        }
    }

    if let Some(metal_path) = find_metal_artifact(&siblings) {
        files.push(ManifestFile {
            name: "model.metal.bin".to_string(),
            priority: manifest_file_priority("model.metal.bin"),
            runtimes: runtime_hint.clone(),
            url: Some(hf_resolve_url(&base_url, &repo, &metal_path)),
            optional: None,
        });
    }

    if let Err(e) = append_supported_artifacts(
        &mut files,
        supported.as_ref(),
        &repo,
        &base_url,
        &runtime_hint,
    ) {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from(format!("{{\"error\": \"{}\"}}", e)))
            .unwrap();
    }

    let body = serde_json::to_string(&Manifest {
        format: manifest_format_label(selection.format).to_string(),
        files,
        quantization: manifest_quantization,
    })
    .unwrap_or_else(|_| "{\"format\":\"unknown\",\"files\":[]}".into());
    Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

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

    // NOTE: compute_gpu_warnings は SPEC-6cd7f960 で廃止されました

    // ===== SPEC-6cd7f960: 対応モデルリスト型管理 =====

    #[test]
    fn test_model_status_serialization() {
        // ModelStatusが正しくシリアライズされることを確認
        assert_eq!(
            serde_json::to_string(&ModelStatus::Available).unwrap(),
            "\"available\""
        );
        assert_eq!(
            serde_json::to_string(&ModelStatus::Downloading).unwrap(),
            "\"downloading\""
        );
        assert_eq!(
            serde_json::to_string(&ModelStatus::Downloaded).unwrap(),
            "\"downloaded\""
        );
    }

    #[test]
    fn test_model_with_status_from_supported() {
        use crate::supported_models::SupportedModel;

        let supported = SupportedModel {
            id: "test-model".into(),
            name: "Test Model".into(),
            description: "A test model".into(),
            repo: "test/repo".into(),
            recommended_filename: "model.gguf".into(),
            size_bytes: 1000,
            required_memory_bytes: 1500,
            tags: vec!["test".into()],
            capabilities: vec!["TextGeneration".into()],
            quantization: Some("Q4_K_M".into()),
            parameter_count: Some("7B".into()),
            format: "gguf".into(),
            engine: "llama_cpp".into(),
            platforms: vec!["macos-metal".into()],
            artifacts: vec![],
        };

        let with_status = ModelWithStatus::from_supported(supported.clone());

        assert_eq!(with_status.id, "test-model");
        assert_eq!(with_status.name, "Test Model");
        assert_eq!(with_status.status, ModelStatus::Available);
        assert!(with_status.lifecycle_status.is_none());
        assert!(with_status.download_progress.is_none());
        assert!(with_status.hf_info.is_none());
    }
    #[test]
    fn test_model_with_status_serialization() {
        use crate::supported_models::SupportedModel;

        let supported = SupportedModel {
            id: "qwen2.5-7b-instruct".into(),
            name: "Qwen2.5 7B Instruct".into(),
            description: "Test".into(),
            repo: "bartowski/Qwen2.5-7B-Instruct-GGUF".into(),
            recommended_filename: "Qwen2.5-7B-Instruct-Q4_K_M.gguf".into(),
            size_bytes: 4_920_000_000,
            required_memory_bytes: 7_380_000_000,
            tags: vec!["chat".into()],
            capabilities: vec!["TextGeneration".into()],
            quantization: Some("Q4_K_M".into()),
            parameter_count: Some("7B".into()),
            format: "gguf".into(),
            engine: "llama_cpp".into(),
            platforms: vec![
                "macos-metal".into(),
                "windows-directml".into(),
                "linux-cuda".into(),
            ],
            artifacts: vec![],
        };

        let with_status = ModelWithStatus::from_supported(supported);
        let json = serde_json::to_string(&with_status).expect("シリアライズに失敗");

        // JSONに必要なフィールドが含まれることを確認
        assert!(json.contains("\"id\":\"qwen2.5-7b-instruct\""));
        assert!(json.contains("\"status\":\"available\""));
        // skip_serializing_if により None フィールドは含まれない
        assert!(!json.contains("\"lifecycle_status\""));
        assert!(!json.contains("\"download_progress\""));
    }

    #[test]
    fn test_append_supported_artifacts_adds_entries() {
        use crate::supported_models::{SupportedArtifact, SupportedModel};

        let supported = SupportedModel {
            id: "gpt-oss-20b".into(),
            name: "GPT-OSS 20B".into(),
            description: "Test".into(),
            repo: "openai/gpt-oss-20b".into(),
            recommended_filename: "model.safetensors.index.json".into(),
            size_bytes: 1,
            required_memory_bytes: 1,
            tags: vec!["test".into()],
            capabilities: vec!["TextGeneration".into()],
            quantization: None,
            parameter_count: Some("20B".into()),
            format: "safetensors".into(),
            engine: "gptoss_cpp".into(),
            platforms: vec!["macos-metal".into()],
            artifacts: vec![SupportedArtifact {
                name: "model.metal.bin".into(),
                path: Some("metal/model.bin".into()),
                url: None,
                repo: Some("openai/gpt-oss-20b".into()),
                priority: Some(5),
                runtimes: Some(vec!["gptoss_cpp".into()]),
            }],
        };

        let mut files = vec![ManifestFile {
            name: "model.safetensors.index.json".into(),
            priority: None,
            runtimes: None,
            url: Some(
                "https://hf.example.com/openai/gpt-oss-20b/resolve/main/model.safetensors.index.json"
                    .into(),
            ),
            optional: None,
        }];

        let runtime_hint = Some(vec!["gptoss_cpp".into()]);
        append_supported_artifacts(
            &mut files,
            Some(&supported),
            "openai/gpt-oss-20b",
            "https://hf.example.com",
            &runtime_hint,
        )
        .expect("append_supported_artifacts failed");

        assert!(files.iter().any(|f| {
            f.name == "model.metal.bin"
                && f.url.as_deref()
                    == Some(
                        "https://hf.example.com/openai/gpt-oss-20b/resolve/main/metal/model.bin",
                    )
        }));
    }

    #[test]
    fn test_list_models_with_status_returns_all_supported_models() {
        // list_models_with_status()が全ての対応モデルを返すことを確認
        // NOTE: このテストはlist_models_with_status()実装後に有効化
        let supported = get_supported_models();
        assert!(!supported.is_empty(), "対応モデルが存在すること");

        // 各モデルがModelWithStatusに変換できることを確認
        for model in supported {
            let with_status = ModelWithStatus::from_supported(model);
            assert_eq!(with_status.status, ModelStatus::Available);
        }
    }

    #[test]
    fn test_hf_info_serialization() {
        let hf_info = HfInfo {
            downloads: Some(125000),
            likes: Some(450),
        };
        let json = serde_json::to_string(&hf_info).expect("シリアライズに失敗");
        assert!(json.contains("\"downloads\":125000"));
        assert!(json.contains("\"likes\":450"));

        // Noneの場合はフィールドが省略される
        let empty_info = HfInfo::default();
        let empty_json = serde_json::to_string(&empty_info).expect("シリアライズに失敗");
        assert_eq!(empty_json, "{}");
    }

    #[test]
    fn test_pull_model_validates_supported_model() {
        use crate::supported_models::{find_supported_model, is_supported_model};

        // 対応モデルは有効
        assert!(is_supported_model("qwen2.5-7b-instruct"));
        assert!(find_supported_model("qwen2.5-7b-instruct").is_some());

        // 非対応モデルは無効
        assert!(!is_supported_model("unsupported-model"));
        assert!(find_supported_model("unsupported-model").is_none());
    }
}
