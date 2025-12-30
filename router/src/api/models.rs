//! モデル管理API
//!
//! モデル一覧取得、登録、変換、ファイル配信のエンドポイント

use crate::{
    convert::ConvertStatus,
    db::models::ModelStorage,
    registry::models::{extract_repo_id, generate_model_id, router_model_path, ModelInfo},
    registry::NodeRegistry,
    supported_models::{find_supported_model, get_supported_models, SupportedModel},
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

/// モデルPullリクエスト（SPEC-6cd7f960）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullModelRequest {
    /// 対応モデルID（例: "qwen2.5-7b-instruct"）
    pub model_id: String,
}

/// モデルPullレスポンス（SPEC-6cd7f960）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullModelResponse {
    /// モデルID
    pub model_id: String,
    /// ステータス（"queued"）
    pub status: String,
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

/// GET /v0/models/hub - 対応モデル一覧 + 状態（SPEC-6cd7f960）
///
/// ダッシュボードのModel Hub用。全ての対応モデルを状態付きで返す。
/// HF動的情報（ダウンロード数、いいね数）も含む。
pub async fn list_models_with_status(State(state): State<AppState>) -> Json<Vec<ModelWithStatus>> {
    let supported = get_supported_models();
    let registered = list_registered_models();
    let convert_tasks = state.convert_manager.list_tasks().await;

    // 登録済みモデル名のセット（ダウンロード完了判定用）
    let registered_names: std::collections::HashSet<_> =
        registered.iter().map(|m| m.name.clone()).collect();

    // ダウンロード中タスクのマップ（repo -> task）
    let downloading_tasks: HashMap<String, _> = convert_tasks
        .iter()
        .filter(|t| matches!(t.status, ConvertStatus::Queued | ConvertStatus::InProgress))
        .map(|t| (t.repo.clone(), t.clone()))
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

        // 1. ダウンロード完了判定
        if registered_names.contains(&model_name) {
            with_status.status = ModelStatus::Downloaded;
            with_status.lifecycle_status = Some(LifecycleStatus::Registered);
        }
        // 2. ダウンロード中判定
        else if let Some(task) = downloading_tasks.get(&model.repo) {
            with_status.status = ModelStatus::Downloading;
            with_status.lifecycle_status = Some(match task.status {
                ConvertStatus::Queued => LifecycleStatus::Pending,
                ConvertStatus::InProgress => LifecycleStatus::Caching,
                ConvertStatus::Failed => LifecycleStatus::Error,
                ConvertStatus::Completed => LifecycleStatus::Registered,
            });
            with_status.download_progress = Some(DownloadProgress {
                percent: task.progress as f64,
                bytes_downloaded: None,
                bytes_total: Some(model.size_bytes),
                error: task.error.clone(),
            });
        }
        // 3. それ以外は available（未ダウンロード）

        // HF情報を設定
        if let Some(Some(info)) = hf_infos.get(&model.repo) {
            with_status.hf_info = Some(info.clone());
        }

        result.push(with_status);
    }

    Json(result)
}

/// POST /v0/models/pull - 対応モデルをダウンロード（SPEC-6cd7f960）
///
/// 対応モデルのダウンロードをキューに登録する。
/// 対応モデル以外のIDが指定された場合はエラーを返す。
pub async fn pull_model(
    State(state): State<AppState>,
    Json(req): Json<PullModelRequest>,
) -> Result<(StatusCode, Json<PullModelResponse>), AppError> {
    // 対応モデルかどうかを確認
    let supported_model = find_supported_model(&req.model_id).ok_or_else(|| {
        RouterError::Common(llm_router_common::error::CommonError::Validation(format!(
            "Model '{}' is not a supported model",
            req.model_id
        )))
    })?;

    // モデル名を生成（repo形式）
    let model_name = generate_model_id(&supported_model.repo);

    // 既に登録済みかチェック
    if find_model_by_name(&model_name).is_some() {
        return Err(
            RouterError::Common(llm_router_common::error::CommonError::Validation(
                "Model already downloaded".into(),
            ))
            .into(),
        );
    }

    // 既にダウンロード中かチェック
    if state
        .convert_manager
        .has_task_for_repo(&supported_model.repo)
        .await
    {
        return Err(
            RouterError::Common(llm_router_common::error::CommonError::Validation(
                "Model is already being downloaded".into(),
            ))
            .into(),
        );
    }

    // ConvertTaskManagerにキュー登録
    state
        .convert_manager
        .enqueue(
            supported_model.repo.clone(),
            ModelArtifactFormat::Gguf,
            supported_model.recommended_filename.clone(),
            None, // revision
            None, // quantization
            None, // chat_template (HFから自動取得)
        )
        .await;

    tracing::info!(
        model_id = %req.model_id,
        repo = %supported_model.repo,
        filename = %supported_model.recommended_filename,
        "Model pull queued"
    );

    Ok((
        StatusCode::ACCEPTED,
        Json(PullModelResponse {
            model_id: req.model_id,
            status: "queued".to_string(),
        }),
    ))
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
    "MXFP4", "Q3_K_M", "Q3_K_S", "Q2_K", "IQ4_XS", "IQ3_M", "IQ2_M",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManifestFormat {
    Gguf,
    Safetensors,
    Unknown,
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
    files: Vec<ManifestFile>,
}

fn resolve_manifest_format(model_name: &str, dir: &std::path::Path) -> ManifestFormat {
    if let Some(model) = list_registered_models()
        .into_iter()
        .find(|m| m.name == model_name)
    {
        if model.tags.iter().any(|t| t == "gguf") {
            return ManifestFormat::Gguf;
        }
        if model.tags.iter().any(|t| t == "safetensors") {
            return ManifestFormat::Safetensors;
        }
    }

    if dir.join("model.gguf").exists() {
        return ManifestFormat::Gguf;
    }

    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if is_safetensors_filename(&name) {
                return ManifestFormat::Safetensors;
            }
        }
    }

    ManifestFormat::Unknown
}

fn should_include_manifest_file(format: ManifestFormat, name: &str) -> bool {
    match format {
        ManifestFormat::Gguf => name == "model.gguf",
        ManifestFormat::Safetensors => {
            name == "config.json"
                || name == "tokenizer.json"
                || name == "model.metal.bin"
                || is_safetensors_filename(name)
        }
        ManifestFormat::Unknown => true,
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

fn build_registry_manifest_files(
    dir: &std::path::Path,
    format: ManifestFormat,
    runtime_hint: Option<&Vec<String>>,
) -> Vec<ManifestFile> {
    let mut files: Vec<ManifestFile> = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return files;
    };

    for entry in rd.flatten() {
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !should_include_manifest_file(format, &name) {
            continue;
        }

        let priority = manifest_file_priority(&name);
        files.push(ManifestFile {
            name,
            priority,
            runtimes: runtime_hint.cloned(),
            url: None,
            optional: None,
        });
    }

    files.sort_by(|a, b| a.name.cmp(&b.name));
    files
}

fn is_repo_allowed_for_optimized_artifacts(repo: &str) -> bool {
    let allowlist = std::env::var("LLM_ROUTER_OPTIMIZED_ARTIFACT_ALLOWLIST")
        .unwrap_or_else(|_| "openai/*,nvidia/*".to_string());
    for pat in allowlist
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        if let Some(prefix) = pat.strip_suffix("/*") {
            let prefix = prefix.trim_end_matches('/');
            if repo.starts_with(&format!("{}/", prefix)) {
                return true;
            }
            continue;
        }
        if pat.eq_ignore_ascii_case(repo) {
            return true;
        }
    }
    false
}

fn parse_hf_revision(download_url: &str, repo: &str) -> Option<String> {
    let repo = repo.trim_start_matches('/');
    let needle = format!("/{}/resolve/", repo);
    let idx = download_url.find(&needle)?;
    let rest = &download_url[idx + needle.len()..];
    let rev = rest.split('/').next()?;
    if rev.is_empty() {
        None
    } else {
        Some(rev.to_string())
    }
}

fn append_official_gpu_artifacts(
    files: &mut Vec<ManifestFile>,
    model: Option<&ModelInfo>,
    runtime_hint: Option<&Vec<String>>,
) {
    let Some(model) = model else { return };
    let Some(repo) = model.repo.as_ref() else {
        return;
    };
    if !is_repo_allowed_for_optimized_artifacts(repo) {
        return;
    }

    let base_url = std::env::var("HF_BASE_URL")
        .unwrap_or_else(|_| "https://huggingface.co".to_string())
        .trim_end_matches('/')
        .to_string();
    let rev = model
        .download_url
        .as_deref()
        .and_then(|u| parse_hf_revision(u, repo))
        .unwrap_or_else(|| "main".to_string());

    let name = "model.metal.bin";
    if files.iter().any(|f| f.name == name) {
        return;
    }

    let url = format!("{}/{}/resolve/{}/metal/model.bin", base_url, repo, rev);
    files.push(ManifestFile {
        name: name.to_string(),
        priority: Some(5),
        runtimes: runtime_hint.cloned(),
        url: Some(url),
        optional: Some(true),
    });
    files.sort_by(|a, b| a.name.cmp(&b.name));
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
        "MXFP4" => 11,
        "Q3_K_M" => 12,
        "Q3_K_S" => 13,
        "Q2_K" => 14,
        "IQ4_XS" => 15,
        "IQ3_M" => 16,
        "IQ2_M" => 17,
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
            | "MXFP4"
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
async fn discover_gguf_versions_impl(
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
    #[serde(rename = "id", alias = "modelId")]
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

#[derive(Deserialize)]
/// /v0/models/discover-gguf リクエスト
pub struct DiscoverGgufRequest {
    /// 検索対象のモデル名（HF repo または URL）
    pub model: String,
}

#[derive(Serialize)]
/// /v0/models/discover-gguf レスポンス
pub struct DiscoverGgufResponse {
    /// 正規化後のベースモデル名
    pub base_model: String,
    /// 見つかった GGUF 代替候補
    pub gguf_alternatives: Vec<GgufDiscoveryResult>,
    /// キャッシュヒットかどうか
    pub cached: bool,
}

/// POST /v0/models/discover-gguf - HF repo から GGUF 版を探索
pub async fn discover_gguf_versions(
    State(state): State<AppState>,
    Json(req): Json<DiscoverGgufRequest>,
) -> Result<(StatusCode, Json<DiscoverGgufResponse>), AppError> {
    if req.model.trim().is_empty() {
        return Err(
            RouterError::Common(CommonError::Validation("model is required".into())).into(),
        );
    }

    let base_model = extract_repo_id(&req.model);
    let cached = {
        let cache = GGUF_DISCOVERY_CACHE.read().unwrap();
        cache
            .get(&base_model)
            .map(|entry| entry.fetched_at.elapsed() < GGUF_DISCOVERY_CACHE_TTL)
            .unwrap_or(false)
    };

    let results = discover_gguf_versions_impl(&state.http_client, &base_model).await?;
    let response = DiscoverGgufResponse {
        base_model,
        gguf_alternatives: results,
        cached,
    };

    Ok((StatusCode::OK, Json(response)))
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

    // Optional: runtime hint for nodes so they can skip downloading unsupported models.
    // This keeps the manifest backward-compatible: nodes that don't understand `runtimes` will ignore it.
    let runtime_hint: Option<Vec<String>> = if dir.join("model.gguf").exists() {
        Some(vec!["llama_cpp".to_string()])
    } else {
        let cfg_path = dir.join("config.json");
        if cfg_path.exists() {
            match tokio::fs::read(&cfg_path).await {
                Ok(bytes) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
                    Ok(v) => {
                        let mut rt: Option<String> = None;

                        if let Some(arr) = v.get("architectures").and_then(|x| x.as_array()) {
                            for a in arr {
                                let Some(s) = a.as_str() else { continue };
                                if s.contains("GptOss") || s.contains("GPTOSS") {
                                    rt = Some("gptoss_cpp".to_string());
                                    break;
                                }
                                if s.contains("Nemotron") {
                                    rt = Some("nemotron_cpp".to_string());
                                    break;
                                }
                            }
                        }

                        if rt.is_none() {
                            if let Some(mt) = v.get("model_type").and_then(|x| x.as_str()) {
                                let mt = mt.to_ascii_lowercase();
                                if mt.contains("gpt_oss") || mt.contains("gptoss") {
                                    rt = Some("gptoss_cpp".to_string());
                                } else if mt.contains("nemotron") {
                                    rt = Some("nemotron_cpp".to_string());
                                }
                            }
                        }

                        rt.map(|s| vec![s])
                    }
                    Err(_) => None,
                },
                Err(_) => None,
            }
        } else {
            None
        }
    };

    let model_info = list_registered_models()
        .into_iter()
        .find(|m| m.name == model_name);

    let format = resolve_manifest_format(&model_name, &dir);
    let mut files = build_registry_manifest_files(&dir, format, runtime_hint.as_ref());
    if format == ManifestFormat::Safetensors {
        append_official_gpu_artifacts(&mut files, model_info.as_ref(), runtime_hint.as_ref());
    }

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

    fn write_text(path: &std::path::Path, contents: &str) {
        std::fs::write(path, contents).expect("write file");
    }

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

    #[test]
    fn test_extract_quantization_detects_mxfp4() {
        assert_eq!(
            extract_quantization("gpt-oss-20b-mxfp4.gguf"),
            Some("MXFP4".to_string())
        );
        assert_eq!(
            normalize_quantization_label("mxfp4"),
            Some("MXFP4".to_string())
        );
    }

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
    fn test_hf_model_deserialize_accepts_id_field() {
        let input = r#"{"id":"ggml-org/gpt-oss-20b-GGUF","siblings":[{"rfilename":"a.gguf"}]}"#;
        let parsed: HfModel = serde_json::from_str(input).expect("deserialize");
        assert_eq!(parsed.model_id, "ggml-org/gpt-oss-20b-GGUF");
    }

    #[test]
    fn test_hf_model_deserialize_accepts_model_id_field_alias() {
        let input =
            r#"{"modelId":"ggml-org/gpt-oss-20b-GGUF","siblings":[{"rfilename":"a.gguf"}]}"#;
        let parsed: HfModel = serde_json::from_str(input).expect("deserialize");
        assert_eq!(parsed.model_id, "ggml-org/gpt-oss-20b-GGUF");
    }

    #[test]
    fn test_build_registry_manifest_files_filters_safetensors() {
        use tempfile::tempdir;

        let temp_dir = tempdir().expect("temp dir");
        let model_dir = temp_dir.path().join("openai").join("gpt-oss-20b");
        std::fs::create_dir_all(&model_dir).expect("create model dir");

        write_text(&model_dir.join("config.json"), "{}");
        write_text(&model_dir.join("tokenizer.json"), "{}");
        write_text(
            &model_dir.join("model.safetensors.index.json"),
            "{\"weight_map\":{}}",
        );
        write_text(&model_dir.join("model-00001-of-00002.safetensors"), "a");
        write_text(&model_dir.join("model-00002-of-00002.safetensors"), "b");
        write_text(&model_dir.join("model.metal.bin"), "cache");
        write_text(&model_dir.join("README.md"), "ignore");

        let files = build_registry_manifest_files(&model_dir, ManifestFormat::Safetensors, None);
        let names: std::collections::HashSet<_> = files.iter().map(|f| f.name.as_str()).collect();

        assert!(names.contains("config.json"));
        assert!(names.contains("tokenizer.json"));
        assert!(names.contains("model.safetensors.index.json"));
        assert!(names.contains("model-00001-of-00002.safetensors"));
        assert!(names.contains("model-00002-of-00002.safetensors"));
        assert!(names.contains("model.metal.bin"));
        assert!(!names.contains("README.md"));
    }

    #[test]
    fn test_build_registry_manifest_files_filters_gguf() {
        use tempfile::tempdir;

        let temp_dir = tempdir().expect("temp dir");
        let model_dir = temp_dir.path().join("llama-3-8b");
        std::fs::create_dir_all(&model_dir).expect("create model dir");

        write_text(&model_dir.join("model.gguf"), "gguf");
        write_text(&model_dir.join("config.json"), "{}");
        write_text(&model_dir.join("README.md"), "ignore");

        let files = build_registry_manifest_files(&model_dir, ManifestFormat::Gguf, None);
        let names: std::collections::HashSet<_> = files.iter().map(|f| f.name.as_str()).collect();

        assert!(names.contains("model.gguf"));
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn test_manifest_appends_optional_metal_artifact_when_allowed() {
        use tempfile::tempdir;

        let temp_dir = tempdir().expect("temp dir");
        let model_dir = temp_dir.path().join("openai").join("gpt-oss-20b");
        std::fs::create_dir_all(&model_dir).expect("create model dir");

        write_text(&model_dir.join("config.json"), "{}");
        write_text(&model_dir.join("tokenizer.json"), "{}");
        write_text(
            &model_dir.join("model.safetensors.index.json"),
            "{\"weight_map\":{}}",
        );

        let prev_allowlist = std::env::var("LLM_ROUTER_OPTIMIZED_ARTIFACT_ALLOWLIST").ok();
        let prev_base_url = std::env::var("HF_BASE_URL").ok();
        std::env::set_var("LLM_ROUTER_OPTIMIZED_ARTIFACT_ALLOWLIST", "openai/*");
        std::env::set_var("HF_BASE_URL", "https://hf.test");

        let mut model = ModelInfo::new(
            "openai/gpt-oss-20b".into(),
            0,
            "test".into(),
            0,
            vec!["safetensors".into()],
        );
        model.repo = Some("openai/gpt-oss-20b".into());
        model.download_url = Some(
            "https://hf.test/openai/gpt-oss-20b/resolve/dev/model.safetensors.index.json".into(),
        );

        let runtime_hint = vec!["gptoss_cpp".to_string()];
        let mut files = build_registry_manifest_files(
            &model_dir,
            ManifestFormat::Safetensors,
            Some(&runtime_hint),
        );
        append_official_gpu_artifacts(&mut files, Some(&model), Some(&runtime_hint));

        let entry = files
            .iter()
            .find(|f| f.name == "model.metal.bin")
            .expect("optional metal artifact");

        assert_eq!(
            entry.url.as_deref(),
            Some("https://hf.test/openai/gpt-oss-20b/resolve/dev/metal/model.bin")
        );
        assert_eq!(entry.optional, Some(true));
        assert_eq!(entry.runtimes.as_ref().unwrap(), &runtime_hint);

        if let Some(v) = prev_allowlist {
            std::env::set_var("LLM_ROUTER_OPTIMIZED_ARTIFACT_ALLOWLIST", v);
        } else {
            std::env::remove_var("LLM_ROUTER_OPTIMIZED_ARTIFACT_ALLOWLIST");
        }
        if let Some(v) = prev_base_url {
            std::env::set_var("HF_BASE_URL", v);
        } else {
            std::env::remove_var("HF_BASE_URL");
        }
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
    fn test_pull_model_request_serialization() {
        let req = PullModelRequest {
            model_id: "qwen2.5-7b-instruct".to_string(),
        };
        let json = serde_json::to_string(&req).expect("シリアライズに失敗");
        assert!(json.contains("\"model_id\":\"qwen2.5-7b-instruct\""));

        // デシリアライズも確認
        let deserialized: PullModelRequest =
            serde_json::from_str(&json).expect("デシリアライズに失敗");
        assert_eq!(deserialized.model_id, "qwen2.5-7b-instruct");
    }

    #[test]
    fn test_pull_model_response_serialization() {
        let resp = PullModelResponse {
            model_id: "qwen2.5-7b-instruct".to_string(),
            status: "queued".to_string(),
        };
        let json = serde_json::to_string(&resp).expect("シリアライズに失敗");
        assert!(json.contains("\"model_id\":\"qwen2.5-7b-instruct\""));
        assert!(json.contains("\"status\":\"queued\""));
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
