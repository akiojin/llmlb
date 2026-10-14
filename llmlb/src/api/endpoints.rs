//! エンドポイント管理API
//!
//! SPEC-e8e9326e: llmlb主導エンドポイント登録システム

use super::error::{AppError, ManagementError};
use crate::api::openai_util::{
    classify_upstream_request_error, openai_error_response_with_type, probe_ollama_model_loaded,
};
use crate::common::auth::{Claims, UserRole};
use crate::common::error::{CommonError, LbError};
use crate::db::endpoints as db;
use crate::detection::{detect_endpoint_type_with_client, DetectionError};
use crate::sync::{self, SyncError};
use crate::system_info;
use crate::types::endpoint::{
    DeviceInfo, Endpoint, EndpointCapability, EndpointModel, EndpointStatus, EndpointType,
    ModelDownloadTask,
};
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use reqwest::Url;
use serde::{Deserialize, Deserializer, Serialize};
use std::time::{Duration, Instant};
use uuid::Uuid;

mod model_management;

pub use model_management::{
    delete_endpoint_model_handler, download_model, download_progress, get_model_info,
    DeleteModelRequest, ModelInfoPath,
};

/// `Option<Option<T>>`のデシリアライズヘルパー
/// - フィールドなし → None
/// - フィールドがnull → Some(None)
/// - フィールドに値あり → Some(Some(value))
fn deserialize_optional_field<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}

/// エンドポイント登録リクエスト
#[derive(Debug, Deserialize)]
pub struct CreateEndpointRequest {
    /// 表示名
    pub name: String,
    /// ベースURL
    pub base_url: String,
    /// APIキー（任意）
    #[serde(default)]
    pub api_key: Option<String>,
    /// ヘルスチェック間隔（秒）
    #[serde(default = "default_health_check_interval")]
    pub health_check_interval_secs: u32,
    /// 推論タイムアウト（秒）
    #[serde(default)]
    pub inference_timeout_secs: Option<u32>,
    /// メモ
    #[serde(default)]
    pub notes: Option<String>,
    /// エンドポイントの機能一覧（画像生成、音声認識等）
    #[serde(default)]
    pub capabilities: Vec<EndpointCapability>,
}

fn default_health_check_interval() -> u32 {
    30
}

/// エンドポイント固有の情報取得方法でデバイス情報を取得（SPEC-f8e3a1b7, SPEC-e8e9326e）
///
/// エンドポイント登録時に呼び出し、デバイス情報を取得する。
/// エンドポイントタイプに応じて最適な取得方法を使用する：
/// - xLLM/Ollama: GET /api/system
/// - llama.cpp: GET /slots (primary) → GET /metrics (fallback)
/// - vLLM/OpenAI互換: サポートなし
///
/// 応答がない場合（タイムアウト、404等）はNoneを返す。
async fn fetch_system_info(
    client: &reqwest::Client,
    base_url: &str,
    api_key: Option<&str>,
    endpoint_type: &EndpointType,
) -> Option<DeviceInfo> {
    system_info::get_endpoint_system_info(client, base_url, api_key, endpoint_type).await
}

/// エンドポイント更新リクエスト
#[derive(Debug, Deserialize)]
pub struct UpdateEndpointRequest {
    /// 表示名
    #[serde(default)]
    pub name: Option<String>,
    /// ベースURL
    #[serde(default)]
    pub base_url: Option<String>,
    /// APIキー
    #[serde(default)]
    pub api_key: Option<String>,
    /// ヘルスチェック間隔（秒）
    #[serde(default)]
    pub health_check_interval_secs: Option<u32>,
    /// 推論タイムアウト（秒）
    #[serde(default)]
    pub inference_timeout_secs: Option<u32>,
    /// メモ（None=未指定, Some(None)=削除, Some(Some(v))=設定）
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub notes: Option<Option<String>>,
}

/// エンドポイントレスポンス
#[derive(Debug, Serialize)]
pub struct EndpointResponse {
    /// 一意識別子
    pub id: Uuid,
    /// 表示名
    pub name: String,
    /// ベースURL
    pub base_url: String,
    /// 現在の状態
    pub status: String,
    /// エンドポイントタイプ（SPEC-e8e9326e）
    pub endpoint_type: String,
    /// ヘルスチェック間隔（秒）
    pub health_check_interval_secs: u32,
    /// 推論タイムアウト（秒）
    pub inference_timeout_secs: u32,
    /// レイテンシ（ミリ秒）
    pub latency_ms: Option<u32>,
    /// 最終確認時刻
    pub last_seen: Option<String>,
    /// 最後のエラーメッセージ
    pub last_error: Option<String>,
    /// 連続エラー回数
    pub error_count: u32,
    /// 登録日時
    pub registered_at: String,
    /// メモ
    pub notes: Option<String>,
    /// デバイス情報（SPEC-f8e3a1b7: /api/systemから取得）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_info: Option<crate::types::endpoint::DeviceInfo>,
    /// モデル数（一覧取得時）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_count: Option<usize>,
    /// 関連モデル一覧（詳細取得時のみ）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<EndpointModelResponse>>,
}

impl From<Endpoint> for EndpointResponse {
    fn from(ep: Endpoint) -> Self {
        EndpointResponse {
            id: ep.id,
            name: ep.name,
            base_url: ep.base_url,
            status: ep.status.as_str().to_string(),
            endpoint_type: ep.endpoint_type.as_str().to_string(),
            health_check_interval_secs: ep.health_check_interval_secs,
            inference_timeout_secs: ep.inference_timeout_secs,
            latency_ms: ep.latency_ms,
            last_seen: ep.last_seen.map(|dt| dt.to_rfc3339()),
            last_error: ep.last_error,
            error_count: ep.error_count,
            registered_at: ep.registered_at.to_rfc3339(),
            notes: ep.notes,
            device_info: ep.device_info,
            model_count: None,
            models: None,
        }
    }
}

/// エンドポイント一覧レスポンス
#[derive(Debug, Serialize)]
pub struct ListEndpointsResponse {
    /// エンドポイント一覧
    pub endpoints: Vec<EndpointResponse>,
    /// 総数
    pub total: usize,
}

/// エンドポイント一覧クエリパラメータ
#[derive(Debug, Deserialize)]
pub struct ListEndpointsQuery {
    /// ステータスでフィルタ（pending, online, offline, error）
    #[serde(default)]
    pub status: Option<String>,
    /// タイプでフィルタ（xllm, ollama, vllm, openai_compatible, unknown）
    /// SPEC-e8e9326e
    #[serde(default, rename = "type")]
    pub endpoint_type: Option<String>,
}

/// モデル一覧レスポンス
#[derive(Debug, Serialize)]
pub struct EndpointModelsResponse {
    /// エンドポイントID
    pub endpoint_id: Uuid,
    /// モデル一覧
    pub models: Vec<EndpointModelResponse>,
}

/// モデル同期レスポンス
#[derive(Debug, Serialize)]
pub struct SyncModelsResponse {
    /// 同期されたモデル一覧
    pub synced_models: Vec<EndpointModelResponse>,
    /// 追加されたモデル数
    pub added: usize,
    /// 削除されたモデル数
    pub removed: usize,
    /// 更新されたモデル数
    pub updated: usize,
}

/// モデルレスポンス
#[derive(Debug, Serialize)]
pub struct EndpointModelResponse {
    /// モデルID
    pub model_id: String,
    /// 能力（chat, embeddings等）
    pub capabilities: Option<Vec<String>>,
    /// 最大トークン数（xLLM/Ollamaで取得される場合がある）
    pub max_tokens: Option<u32>,
    /// 最終確認時刻
    pub last_checked: Option<String>,
    /// 正規名（HFリポ名）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_name: Option<String>,
}

impl From<EndpointModel> for EndpointModelResponse {
    fn from(m: EndpointModel) -> Self {
        EndpointModelResponse {
            model_id: m.model_id,
            capabilities: m.capabilities,
            max_tokens: m.max_tokens,
            last_checked: m.last_checked.map(|dt| dt.to_rfc3339()),
            canonical_name: m.canonical_name,
        }
    }
}

/// 接続テストのエンドポイント情報
#[derive(Debug, Serialize)]
pub struct EndpointTestInfo {
    /// 発見されたモデル数
    pub model_count: usize,
}

/// 接続テスト結果
#[derive(Debug, Serialize)]
pub struct TestConnectionResponse {
    /// 成功フラグ
    pub success: bool,
    /// レイテンシ（ミリ秒）
    pub latency_ms: Option<u32>,
    /// エラーメッセージ
    pub error: Option<String>,
    /// 発見されたモデル一覧
    pub models_found: Option<Vec<String>>,
    /// エンドポイント情報（成功時のみ）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint_info: Option<EndpointTestInfo>,
}

// --- SPEC-e8e9326e: ダウンロード・メタデータ関連型 ---

/// ダウンロードリクエスト（SPEC-e8e9326e）
#[derive(Debug, Deserialize)]
pub struct DownloadModelRequest {
    /// ダウンロードするモデル名
    pub model: String,
    /// HuggingFaceリポジトリ（LM Studio用、任意）
    #[serde(default)]
    pub hf_repo: Option<String>,
    /// 量子化タイプ（LM Studio用、任意。デフォルト: "Q4_K_M"）
    #[serde(default)]
    pub quantization: Option<String>,
}

/// ダウンロードタスクレスポンス（SPEC-e8e9326e）
#[derive(Debug, Serialize)]
pub struct DownloadTaskResponse {
    /// タスクID
    pub task_id: String,
    /// モデル名
    pub model: String,
    /// ステータス
    pub status: String,
    /// 進捗（0.0-100.0）
    pub progress: f64,
    /// ダウンロード速度（MB/s）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_mbps: Option<f64>,
    /// 残り時間（秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta_seconds: Option<u32>,
    /// エラーメッセージ
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

impl From<ModelDownloadTask> for DownloadTaskResponse {
    fn from(task: ModelDownloadTask) -> Self {
        DownloadTaskResponse {
            task_id: task.id,
            model: task.model,
            status: task.status.as_str().to_string(),
            progress: task.progress,
            speed_mbps: task.speed_mbps,
            eta_seconds: task.eta_seconds,
            error_message: task.error_message,
        }
    }
}

/// ダウンロード進捗一覧レスポンス（SPEC-e8e9326e）
#[derive(Debug, Serialize)]
pub struct DownloadProgressResponse {
    /// エンドポイントID
    pub endpoint_id: Uuid,
    /// ダウンロードタスク一覧
    pub tasks: Vec<DownloadTaskResponse>,
}

/// モデル情報レスポンス（SPEC-e8e9326e）
#[derive(Debug, Serialize)]
pub struct ModelInfoResponse {
    /// モデルID
    pub model_id: String,
    /// エンドポイントID
    pub endpoint_id: Uuid,
    /// 最大トークン数（コンテキスト長）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// 最終確認時刻
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_checked: Option<String>,
}

/// エラーレスポンス
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// エラーメッセージ
    pub error: String,
    /// エラーコード
    pub code: String,
}

/// Admin権限を確認
fn ensure_admin(claims: &Claims) -> Result<(), ManagementError> {
    if claims.role != UserRole::Admin {
        return Err(ManagementError(LbError::Authorization(
            "Admin permission required".to_string(),
        )));
    }
    Ok(())
}

/// 接続テスト実行（DB/キャッシュの更新を含む）
async fn run_connection_test(state: &AppState, endpoint: &Endpoint) -> TestConnectionResponse {
    // GET /v1/models でヘルスチェック
    let url = format!("{}/v1/models", endpoint.base_url.trim_end_matches('/'));
    let start = std::time::Instant::now();

    let mut request = state.http_client.get(&url);
    if let Some(ref api_key) = endpoint.api_key {
        request = request.header("Authorization", format!("Bearer {}", api_key));
    }

    let result = request
        .timeout(std::time::Duration::from_secs(
            endpoint.inference_timeout_secs as u64,
        ))
        .send()
        .await;

    let latency_ms = start.elapsed().as_millis() as u32;

    match result {
        Ok(response) => {
            if response.status().is_success() {
                // モデル一覧を取得
                let models_found: Option<Vec<String>> = match response
                    .json::<serde_json::Value>()
                    .await
                {
                    Ok(json) => json["data"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|m| m["id"].as_str().map(String::from))
                                .collect()
                        })
                        .or_else(|| {
                            json["models"].as_array().map(|arr| {
                                arr.iter()
                                    .filter_map(|m| {
                                        m["name"].as_str().or(m["model"].as_str()).map(String::from)
                                    })
                                    .collect()
                            })
                        }),
                    Err(_) => None,
                };

                // ステータスを更新（DB + キャッシュ）
                let _ = state
                    .balancer
                    .endpoint_registry
                    .update_status(endpoint.id, EndpointStatus::Online, Some(latency_ms), None)
                    .await;

                // モデル数を計算
                let model_count = models_found.as_ref().map(|m| m.len()).unwrap_or(0);

                TestConnectionResponse {
                    success: true,
                    latency_ms: Some(latency_ms),
                    error: None,
                    models_found,
                    endpoint_info: Some(EndpointTestInfo { model_count }),
                }
            } else {
                let error_msg = format!("HTTP {}", response.status());
                let _ = state
                    .balancer
                    .endpoint_registry
                    .update_status(endpoint.id, EndpointStatus::Error, None, Some(&error_msg))
                    .await;

                TestConnectionResponse {
                    success: false,
                    latency_ms: Some(latency_ms),
                    error: Some(error_msg),
                    models_found: None,
                    endpoint_info: None,
                }
            }
        }
        Err(e) => {
            let error_msg = e.to_string();
            let _ = state
                .balancer
                .endpoint_registry
                .update_status(endpoint.id, EndpointStatus::Error, None, Some(&error_msg))
                .await;

            TestConnectionResponse {
                success: false,
                latency_ms: None,
                error: Some(error_msg),
                models_found: None,
                endpoint_info: None,
            }
        }
    }
}

/// 接続先 URL のホスト部を返す
///
/// `NodeRegistered` の `ip_address` に使う。URL に含まれうる認証情報は購読者へ渡さない。
fn endpoint_host(base_url: &str) -> String {
    Url::parse(base_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Classify a failed write without inferring an error code from SQL text.
async fn endpoint_write_error(
    pool: &sqlx::SqlitePool,
    endpoint: &Endpoint,
    error: sqlx::Error,
    operation: &str,
) -> ManagementError {
    if error
        .as_database_error()
        .is_some_and(|err| err.is_unique_violation())
    {
        // SQLite does not expose the violated constraint's name. Check the
        // persisted URL and exclude this endpoint (including unchanged updates).
        let duplicate_url = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM endpoints WHERE base_url = ? AND id <> ?)",
        )
        .bind(&endpoint.base_url)
        .bind(endpoint.id.to_string())
        .fetch_one(pool)
        .await;
        let message = "Endpoint with this name or URL already exists".to_string();
        match duplicate_url {
            Ok(true) => ManagementError(LbError::DuplicateUrl(message)),
            Ok(false) => ManagementError(LbError::Conflict(message)),
            Err(err) => {
                tracing::error!("Failed to classify endpoint uniqueness: {}", err);
                ManagementError(LbError::Database(operation.to_string()))
            }
        }
    } else {
        tracing::error!("{}: {}", operation, error);
        ManagementError(LbError::Database(operation.to_string()))
    }
}

// --- Handlers ---

/// POST /api/endpoints - エンドポイント登録
pub async fn create_endpoint(
    Extension(claims): Extension<Claims>,
    State(state): State<AppState>,
    Json(req): Json<CreateEndpointRequest>,
) -> impl IntoResponse {
    // Admin権限チェック
    if let Err(e) = ensure_admin(&claims) {
        return e.into_response();
    }

    // バリデーション
    if req.name.trim().is_empty() {
        return ManagementError(LbError::Common(CommonError::Validation(
            "Name is required".to_string(),
        )))
        .into_response();
    }

    if req.base_url.trim().is_empty() {
        return ManagementError(LbError::Common(CommonError::Validation(
            "Base URL is required".to_string(),
        )))
        .into_response();
    }

    // URL形式チェック
    if Url::parse(&req.base_url).is_err() {
        return ManagementError(LbError::Common(CommonError::Validation(
            "Invalid URL format".to_string(),
        )))
        .into_response();
    }

    // ヘルスチェック間隔のバリデーション（10-300秒）
    if req.health_check_interval_secs < 10 || req.health_check_interval_secs > 300 {
        return ManagementError(LbError::Common(CommonError::Validation(
            "Health check interval must be between 10 and 300 seconds".to_string(),
        )))
        .into_response();
    }

    // 名前の重複チェック
    match db::find_by_name(&state.db_pool, &req.name).await {
        Ok(Some(_)) => {
            return ManagementError(LbError::Common(CommonError::Validation(format!(
                "Endpoint with name '{}' already exists",
                req.name
            ))))
            .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to check name uniqueness: {}", e);
            return ManagementError(LbError::Database(
                "Failed to check name uniqueness".to_string(),
            ))
            .into_response();
        }
        Ok(None) => {} // OK - 名前は一意
    }

    // SPEC-e8e9326e: 自動検出（手動指定は廃止、対応タイプのみ許可）
    let detection_result =
        detect_endpoint_type_with_client(&state.http_client, &req.base_url, req.api_key.as_deref())
            .await;

    let detected_type = match detection_result {
        Ok(result) => result.endpoint_type,
        Err(DetectionError::Unreachable(msg)) => {
            return ManagementError(LbError::Http(format!("Endpoint unreachable: {}", msg)))
                .into_response();
        }
        Err(DetectionError::UnsupportedType(msg)) => {
            return ManagementError(LbError::Common(CommonError::Validation(format!(
                "Unsupported endpoint type: {}",
                msg
            ))))
            .into_response();
        }
    };

    let mut endpoint = Endpoint::new(req.name, req.base_url.clone(), detected_type);
    endpoint.api_key = req.api_key.clone();
    endpoint.health_check_interval_secs = req.health_check_interval_secs;
    if let Some(timeout) = req.inference_timeout_secs {
        endpoint.inference_timeout_secs = timeout;
    }
    endpoint.notes = req.notes;
    if !req.capabilities.is_empty() {
        endpoint.capabilities = req.capabilities;
    }

    match db::create_endpoint(&state.db_pool, &endpoint).await {
        Ok(()) => {
            // EndpointRegistryキャッシュも更新（DBは既に保存済みなのでキャッシュのみ）
            state
                .balancer
                .endpoint_registry
                .add_to_cache(endpoint.clone())
                .await;

            // SPEC #582 FR-048a: 登録の確定（DB 保存とキャッシュ反映）を購読者へ通知する
            state
                .event_bus
                .publish(crate::events::DashboardEvent::NodeRegistered {
                    runtime_id: endpoint.id,
                    machine_name: endpoint.name.clone(),
                    ip_address: endpoint_host(&endpoint.base_url),
                    status: endpoint.status,
                });

            // SPEC-f8e3a1b7, SPEC-e8e9326e: エンドポイント固有の方法でデバイス情報を取得
            let endpoint_id = endpoint.id;
            let base_url = endpoint.base_url.clone();
            let api_key = endpoint.api_key.clone();
            let endpoint_type = endpoint.endpoint_type;
            let registry = state.balancer.endpoint_registry.clone();
            let http_client = state.http_client.clone();

            // Fire-and-forget: デバイス情報取得は非同期で行う（レスポンスをブロックしない）
            tokio::spawn(async move {
                if let Some(device_info) =
                    fetch_system_info(&http_client, &base_url, api_key.as_deref(), &endpoint_type)
                        .await
                {
                    tracing::info!(
                        endpoint_id = %endpoint_id,
                        device_type = ?device_info.device_type,
                        gpu_count = device_info.gpu_devices.len(),
                        endpoint_type = ?endpoint_type,
                        "Retrieved device info via endpoint-specific method"
                    );
                    if let Err(e) = registry
                        .update_device_info(endpoint_id, Some(device_info))
                        .await
                    {
                        tracing::warn!(
                            endpoint_id = %endpoint_id,
                            error = %e,
                            "Failed to save device info"
                        );
                    }
                }
            });

            // 登録直後に接続チェック＆モデル同期（バックグラウンド実行）
            let state_clone = state.clone();
            let endpoint_clone = endpoint.clone();
            tokio::spawn(async move {
                let test_result = run_connection_test(&state_clone, &endpoint_clone).await;
                if !test_result.success {
                    tracing::warn!(
                        endpoint_id = %endpoint_clone.id,
                        endpoint_name = %endpoint_clone.name,
                        error = ?test_result.error,
                        "Auto connection test failed"
                    );
                    return;
                }

                match sync::sync_models_with_type(
                    &state_clone.db_pool,
                    &state_clone.http_client,
                    endpoint_clone.id,
                    &endpoint_clone.base_url,
                    endpoint_clone.api_key.as_deref(),
                    endpoint_clone.inference_timeout_secs as u64,
                    Some(endpoint_clone.endpoint_type),
                )
                .await
                {
                    Ok(result) => {
                        if let Err(e) = state_clone
                            .balancer
                            .endpoint_registry
                            .refresh_model_mappings(endpoint_clone.id)
                            .await
                        {
                            tracing::warn!(
                                endpoint_id = %endpoint_clone.id,
                                error = %e,
                                "Failed to refresh model mappings"
                            );
                        }
                        tracing::info!(
                            endpoint_id = %endpoint_clone.id,
                            added = result.added,
                            removed = result.removed,
                            updated = result.updated,
                            "Auto model sync completed"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            endpoint_id = %endpoint_clone.id,
                            error = %e,
                            "Auto model sync failed"
                        );
                    }
                }
            });

            (StatusCode::CREATED, Json(EndpointResponse::from(endpoint))).into_response()
        }
        Err(e) => endpoint_write_error(&state.db_pool, &endpoint, e, "Failed to create endpoint")
            .await
            .into_response(),
    }
}

/// GET /api/endpoints - エンドポイント一覧
pub async fn list_endpoints(
    State(state): State<AppState>,
    Query(query): Query<ListEndpointsQuery>,
) -> impl IntoResponse {
    match db::list_endpoints(&state.db_pool).await {
        Ok(endpoints) => {
            // ステータスでフィルタ
            let mut filtered_endpoints: Vec<Endpoint> = if let Some(ref status) = query.status {
                endpoints
                    .into_iter()
                    .filter(|ep| ep.status.as_str() == status)
                    .collect()
            } else {
                endpoints
            };

            // SPEC-e8e9326e: タイプでフィルタ
            if let Some(ref endpoint_type) = query.endpoint_type {
                filtered_endpoints.retain(|ep| ep.endpoint_type.as_str() == endpoint_type);
            }

            let total = filtered_endpoints.len();
            let mut response_endpoints = Vec::with_capacity(total);

            for ep in filtered_endpoints {
                let ep_id = ep.id;
                let mut response = EndpointResponse::from(ep);

                // モデル数を取得
                if let Ok(models) = db::list_endpoint_models(&state.db_pool, ep_id).await {
                    response.model_count = Some(models.len());
                } else {
                    response.model_count = Some(0);
                }

                response_endpoints.push(response);
            }

            let response = ListEndpointsResponse {
                endpoints: response_endpoints,
                total,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list endpoints: {}", e);
            ManagementError(LbError::Database("Failed to list endpoints".to_string()))
                .into_response()
        }
    }
}

/// GET /api/endpoints/:id - エンドポイント詳細
pub async fn get_endpoint(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match db::get_endpoint(&state.db_pool, id).await {
        Ok(Some(endpoint)) => {
            // モデル一覧も取得して詳細レスポンスに含める
            let models = match db::list_endpoint_models(&state.db_pool, id).await {
                Ok(m) => Some(m.into_iter().map(EndpointModelResponse::from).collect()),
                Err(_) => None,
            };
            let mut response = EndpointResponse::from(endpoint);
            response.models = models;
            (StatusCode::OK, Json(response)).into_response()
        }
        Ok(None) => ManagementError(LbError::EndpointNotFound(id)).into_response(),
        Err(e) => {
            tracing::error!("Failed to get endpoint: {}", e);
            ManagementError(LbError::Database("Failed to get endpoint".to_string())).into_response()
        }
    }
}

/// PUT /api/endpoints/:id - エンドポイント更新
pub async fn update_endpoint(
    Extension(claims): Extension<Claims>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateEndpointRequest>,
) -> impl IntoResponse {
    // Admin権限チェック
    if let Err(e) = ensure_admin(&claims) {
        return e.into_response();
    }

    // 既存のエンドポイントを取得
    let existing = match db::get_endpoint(&state.db_pool, id).await {
        Ok(Some(ep)) => ep,
        Ok(None) => return ManagementError(LbError::EndpointNotFound(id)).into_response(),
        Err(e) => {
            tracing::error!("Failed to get endpoint for update: {}", e);
            return ManagementError(LbError::Database("Failed to get endpoint".to_string()))
                .into_response();
        }
    };

    // 名前のバリデーション（空文字列は不許可）
    if let Some(ref name) = req.name {
        if name.trim().is_empty() {
            return ManagementError(LbError::Common(CommonError::Validation(
                "Name cannot be empty".to_string(),
            )))
            .into_response();
        }
    }

    // URL形式チェック
    if let Some(ref url) = req.base_url {
        if Url::parse(url).is_err() {
            return ManagementError(LbError::Common(CommonError::Validation(
                "Invalid URL format".to_string(),
            )))
            .into_response();
        }
    }

    // 名前変更時の重複チェック（他のエンドポイントと重複していないか）
    if let Some(ref new_name) = req.name {
        if new_name != &existing.name {
            match db::find_by_name(&state.db_pool, new_name).await {
                Ok(Some(_)) => {
                    return ManagementError(LbError::Common(CommonError::Validation(format!(
                        "Endpoint with name '{}' already exists",
                        new_name
                    ))))
                    .into_response()
                }
                Err(e) => {
                    tracing::error!("Failed to check name uniqueness: {}", e);
                    return ManagementError(LbError::Database(
                        "Failed to check name uniqueness".to_string(),
                    ))
                    .into_response();
                }
                Ok(None) => {} // OK - 名前は一意
            }
        }
    }

    // 更新内容を適用
    let original_base_url = existing.base_url.clone();
    let mut updated = existing;
    if let Some(name) = req.name {
        updated.name = name;
    }
    if let Some(base_url) = req.base_url {
        updated.base_url = base_url;
    }
    if let Some(api_key) = req.api_key {
        updated.api_key = Some(api_key);
    }
    if let Some(interval) = req.health_check_interval_secs {
        updated.health_check_interval_secs = interval;
    }
    if let Some(timeout) = req.inference_timeout_secs {
        updated.inference_timeout_secs = timeout;
    }
    // notes: None=未指定(そのまま), Some(None)=削除, Some(Some(v))=設定
    if let Some(notes_value) = req.notes {
        updated.notes = notes_value;
    }

    // SPEC-e8e9326e: base_url変更時はタイプを再検出
    if updated.base_url != original_base_url {
        let detection_result = detect_endpoint_type_with_client(
            &state.http_client,
            &updated.base_url,
            updated.api_key.as_deref(),
        )
        .await;

        match detection_result {
            Ok(result) => {
                updated.endpoint_type = result.endpoint_type;
            }
            Err(DetectionError::Unreachable(msg)) => {
                return ManagementError(LbError::Http(format!("Endpoint unreachable: {}", msg)))
                    .into_response();
            }
            Err(DetectionError::UnsupportedType(msg)) => {
                return ManagementError(LbError::Common(CommonError::Validation(format!(
                    "Unsupported endpoint type: {}",
                    msg
                ))))
                .into_response();
            }
        }
    }

    match state
        .balancer
        .endpoint_registry
        .update(updated.clone())
        .await
    {
        Ok(true) => (StatusCode::OK, Json(EndpointResponse::from(updated))).into_response(),
        Ok(false) => ManagementError(LbError::EndpointNotFound(id)).into_response(),
        Err(e) => endpoint_write_error(&state.db_pool, &updated, e, "Failed to update endpoint")
            .await
            .into_response(),
    }
}

/// DELETE /api/endpoints/:id - エンドポイント削除
pub async fn delete_endpoint(
    Extension(claims): Extension<Claims>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    // Admin権限チェック
    if let Err(e) = ensure_admin(&claims) {
        return e.into_response();
    }

    // EndpointRegistry::remove を使用してDBとキャッシュ両方から削除
    match state.balancer.endpoint_registry.remove(id).await {
        Ok(true) => {
            // EndpointRegistry::remove は LoadManager の状態までは掃除しないため、
            // 負荷状態・TPS状態がリークしないよう明示的に破棄する。
            state.balancer.load_manager.forget_endpoint(id).await;
            // SPEC #582 FR-048b: 削除の確定を購読者へ通知する
            state
                .event_bus
                .publish(crate::events::DashboardEvent::NodeRemoved { runtime_id: id });
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => ManagementError(LbError::EndpointNotFound(id)).into_response(),
        Err(e) => {
            tracing::error!("Failed to delete endpoint: {}", e);
            ManagementError(LbError::Database("Failed to delete endpoint".to_string()))
                .into_response()
        }
    }
}

/// POST /api/endpoints/:id/test - 接続テスト
pub async fn test_endpoint(
    Extension(claims): Extension<Claims>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    // Admin権限チェック
    if let Err(e) = ensure_admin(&claims) {
        return e.into_response();
    }

    // エンドポイントを取得
    let endpoint = match db::get_endpoint(&state.db_pool, id).await {
        Ok(Some(ep)) => ep,
        Ok(None) => return ManagementError(LbError::EndpointNotFound(id)).into_response(),
        Err(e) => {
            tracing::error!("Failed to get endpoint for test: {}", e);
            return ManagementError(LbError::Database("Failed to get endpoint".to_string()))
                .into_response();
        }
    };

    let response = run_connection_test(&state, &endpoint).await;
    (StatusCode::OK, Json(response)).into_response()
}

/// POST /api/endpoints/:id/sync - モデル一覧同期
pub async fn sync_endpoint_models(
    Extension(claims): Extension<Claims>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    // Admin権限チェック
    if let Err(e) = ensure_admin(&claims) {
        return e.into_response();
    }

    // エンドポイントを取得
    let endpoint = match db::get_endpoint(&state.db_pool, id).await {
        Ok(Some(ep)) => ep,
        Ok(None) => return ManagementError(LbError::EndpointNotFound(id)).into_response(),
        Err(e) => {
            tracing::error!("Failed to get endpoint for sync: {}", e);
            return ManagementError(LbError::Database("Failed to get endpoint".to_string()))
                .into_response();
        }
    };

    let result = sync::sync_models_with_type(
        &state.db_pool,
        &state.http_client,
        id,
        &endpoint.base_url,
        endpoint.api_key.as_deref(),
        endpoint.inference_timeout_secs as u64,
        Some(endpoint.endpoint_type),
    )
    .await;

    match result {
        Ok(result) => {
            // EndpointRegistryキャッシュをリロードしてモデルマッピングを更新
            let _ = state.balancer.endpoint_registry.reload().await;

            let synced_models = result
                .models
                .into_iter()
                .map(EndpointModelResponse::from)
                .collect();

            (
                StatusCode::OK,
                Json(SyncModelsResponse {
                    synced_models,
                    added: result.added,
                    removed: result.removed,
                    updated: result.updated,
                }),
            )
                .into_response()
        }
        Err(err) => ManagementError(sync_error_to_lb_error(err)).into_response(),
    }
}

fn sync_error_to_lb_error(err: SyncError) -> LbError {
    match err {
        SyncError::ConnectionError(msg) => {
            LbError::ServiceUnavailable(format!("Failed to connect: {}", msg))
        }
        SyncError::HttpError(status, _) => {
            LbError::Http(format!("Endpoint returned HTTP {}", status))
        }
        SyncError::ParseError(msg) => LbError::Http(format!("Failed to parse response: {}", msg)),
        SyncError::DbError(msg) => LbError::Database(format!("Failed to update models: {}", msg)),
    }
}

/// GET /api/endpoints/:id/models - エンドポイントのモデル一覧
pub async fn list_endpoint_models(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    // エンドポイント存在確認
    match db::get_endpoint(&state.db_pool, id).await {
        Ok(None) => return ManagementError(LbError::EndpointNotFound(id)).into_response(),
        Err(e) => {
            tracing::error!("Failed to get endpoint: {}", e);
            return ManagementError(LbError::Database("Failed to get endpoint".to_string()))
                .into_response();
        }
        Ok(Some(_)) => {}
    }

    match db::list_endpoint_models(&state.db_pool, id).await {
        Ok(models) => (
            StatusCode::OK,
            Json(EndpointModelsResponse {
                endpoint_id: id,
                models: models
                    .into_iter()
                    .map(EndpointModelResponse::from)
                    .collect(),
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to list endpoint models: {}", e);
            ManagementError(LbError::Database("Failed to list models".to_string())).into_response()
        }
    }
}

/// POST /api/endpoints/:id/chat/completions - エンドポイントへのチャットプロキシ
///
/// ダッシュボードのPlayground用。JWT認証済みユーザーが直接エンドポイントと通信できる。
/// リクエストをそのままエンドポイントの`/v1/chat/completions`に転送する。
pub async fn proxy_chat_completions(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    // エンドポイントを取得
    let endpoint = match db::get_endpoint(&state.db_pool, id).await {
        Ok(Some(ep)) => ep,
        Ok(None) => return AppError(LbError::EndpointNotFound(id)).into_response(),
        Err(e) => {
            tracing::error!("Failed to get endpoint for proxy: {}", e);
            return AppError(LbError::Database("Failed to get endpoint".to_string()))
                .into_response();
        }
    };

    // エンドポイントがオンラインか確認
    if endpoint.status != EndpointStatus::Online {
        return AppError(LbError::ServiceUnavailable(format!(
            "Endpoint is not online (status: {:?})",
            endpoint.status
        )))
        .into_response();
    }

    // リクエストを転送
    let url = format!(
        "{}/v1/chat/completions",
        endpoint.base_url.trim_end_matches('/')
    );
    let request_model = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|payload| {
            payload
                .get("model")
                .and_then(|model| model.as_str())
                .map(str::to_string)
        });

    let mut request = state
        .http_client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(body.to_vec());
    let started_at = Instant::now();

    if let Some(ref api_key) = endpoint.api_key {
        request = request.header("Authorization", format!("Bearer {}", api_key));
    }

    let result = request
        .timeout(Duration::from_secs(endpoint.inference_timeout_secs as u64))
        .send()
        .await;

    match result {
        Ok(response) => {
            // reqwest::StatusCode -> axum::http::StatusCode
            let status_code = StatusCode::from_u16(response.status().as_u16())
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            let latency_ms = u32::try_from(started_at.elapsed().as_millis()).unwrap_or(u32::MAX);
            if status_code.is_success() {
                let _ = state
                    .balancer
                    .endpoint_registry
                    .update_status(endpoint.id, EndpointStatus::Online, Some(latency_ms), None)
                    .await;
            } else if status_code.is_server_error() {
                let error_msg = format!("HTTP {}", status_code);
                let _ = state
                    .balancer
                    .endpoint_registry
                    .update_status(endpoint.id, EndpointStatus::Error, None, Some(&error_msg))
                    .await;
            } else {
                let _ = state
                    .balancer
                    .endpoint_registry
                    .update_status(endpoint.id, EndpointStatus::Online, Some(latency_ms), None)
                    .await;
            }
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/json")
                .to_string();

            // ストリーミングレスポンスの場合
            if content_type.contains("text/event-stream") {
                let stream = response.bytes_stream();
                let body = axum::body::Body::from_stream(stream);
                return axum::response::Response::builder()
                    .status(status_code)
                    .header("Content-Type", "text/event-stream")
                    .header("Cache-Control", "no-cache")
                    .header("Connection", "keep-alive")
                    .body(body)
                    .unwrap()
                    .into_response();
            }

            // 通常のJSONレスポンス
            match response.bytes().await {
                Ok(bytes) => axum::response::Response::builder()
                    .status(status_code)
                    .header("Content-Type", content_type)
                    .body(axum::body::Body::from(bytes))
                    .unwrap()
                    .into_response(),
                Err(e) => AppError(LbError::Http(format!("Failed to read response: {}", e)))
                    .into_response(),
            }
        }
        Err(e) => {
            let ollama_loading_model = if e.is_timeout()
                && endpoint.endpoint_type == crate::types::endpoint::EndpointType::Ollama
            {
                match request_model.as_deref() {
                    Some(model) => {
                        match probe_ollama_model_loaded(
                            &state.http_client,
                            &endpoint.base_url,
                            endpoint.api_key.as_deref(),
                            model,
                        )
                        .await
                        {
                            Some(false) => Some(model.to_string()),
                            _ => None,
                        }
                    }
                    None => None,
                }
            } else {
                None
            };
            let classified_error = classify_upstream_request_error(
                &e,
                &endpoint.base_url,
                endpoint.inference_timeout_secs,
                ollama_loading_model.as_deref(),
            );
            let _ = state
                .balancer
                .endpoint_registry
                .update_status(
                    endpoint.id,
                    EndpointStatus::Error,
                    None,
                    Some(&classified_error.record_message),
                )
                .await;
            openai_error_response_with_type(
                classified_error.client_message,
                classified_error.error_type,
                classified_error.status_code,
            )
            .into_response()
        }
    }
}

#[cfg(test)]
mod tests;
