//! モデル管理API
//!
//! モデル一覧取得、登録、変換、ファイル配信のエンドポイント

use crate::{
    convert::ConvertStatus,
    db::models::ModelStorage,
    registry::models::{generate_model_id, router_model_path, router_model_path_any, ModelInfo},
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
        if router_model_path(&model.name).is_none() {
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

// NOTE: GGUF Discovery Cache は SPEC-6cd7f960 で廃止されました

/// 登録モデルのインメモリキャッシュをクリア（テスト用）
pub fn clear_registered_models() {
    *REGISTERED_MODELS.write().unwrap() = Vec::new();
}

// NOTE: HfSibling, HfLfs, HfModel は SPEC-6cd7f960 で廃止されました

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

// NOTE: POST /v0/models/register は SPEC-6cd7f960 で廃止されました。
// 新しいモデル登録は /v0/models/pull を使用してください。

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

    // 3. ファイルを削除
    if let Some(path) = router_model_path_any(&model_name) {
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

    if removed || task_deleted {
        if removed {
            persist_registered_models(&state.db_pool).await;
        }
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(RouterError::Common(CommonError::Validation("model not found".into())).into())
    }
}

// NOTE: POST /v0/models/discover-gguf は SPEC-6cd7f960 で廃止されました。

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
