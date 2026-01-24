//! エンドポイント管理API
//!
//! SPEC-66555000: ルーター主導エンドポイント登録システム

use crate::db::endpoints as db;
use crate::types::endpoint::{
    Endpoint, EndpointCapability, EndpointModel, EndpointStatus, SupportedAPI,
};
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use llmlb_common::auth::{Claims, UserRole};
use reqwest::Url;
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

/// Option<Option<T>>のデシリアライズヘルパー
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
    #[serde(default = "default_inference_timeout")]
    pub inference_timeout_secs: u32,
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

fn default_inference_timeout() -> u32 {
    120
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
    /// Responses API対応フラグ（SPEC-24157000）
    pub supports_responses_api: bool,
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
            health_check_interval_secs: ep.health_check_interval_secs,
            inference_timeout_secs: ep.inference_timeout_secs,
            latency_ms: ep.latency_ms,
            last_seen: ep.last_seen.map(|dt| dt.to_rfc3339()),
            last_error: ep.last_error,
            error_count: ep.error_count,
            registered_at: ep.registered_at.to_rfc3339(),
            notes: ep.notes,
            supports_responses_api: ep.supports_responses_api,
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
    /// 最終確認時刻
    pub last_checked: Option<String>,
}

impl From<EndpointModel> for EndpointModelResponse {
    fn from(m: EndpointModel) -> Self {
        EndpointModelResponse {
            model_id: m.model_id,
            capabilities: m.capabilities,
            last_checked: m.last_checked.map(|dt| dt.to_rfc3339()),
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

/// エラーレスポンス
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// エラーメッセージ
    pub error: String,
    /// エラーコード
    pub code: String,
}

/// Admin権限を確認
fn ensure_admin(claims: &Claims) -> Result<(), impl IntoResponse> {
    if claims.role != UserRole::Admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Admin permission required".to_string(),
                code: "FORBIDDEN".to_string(),
            }),
        ));
    }
    Ok(())
}

// --- Handlers ---

/// POST /v0/endpoints - エンドポイント登録
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
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Name is required".to_string(),
                code: "INVALID_NAME".to_string(),
            }),
        )
            .into_response();
    }

    if req.base_url.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Base URL is required".to_string(),
                code: "INVALID_URL".to_string(),
            }),
        )
            .into_response();
    }

    // URL形式チェック
    if Url::parse(&req.base_url).is_err() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid URL format".to_string(),
                code: "INVALID_URL_FORMAT".to_string(),
            }),
        )
            .into_response();
    }

    // ヘルスチェック間隔のバリデーション（10-300秒）
    if req.health_check_interval_secs < 10 || req.health_check_interval_secs > 300 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Health check interval must be between 10 and 300 seconds".to_string(),
                code: "INVALID_HEALTH_CHECK_INTERVAL".to_string(),
            }),
        )
            .into_response();
    }

    // 名前の重複チェック
    match db::find_by_name(&state.db_pool, &req.name).await {
        Ok(Some(_)) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Endpoint with name '{}' already exists", req.name),
                    code: "DUPLICATE_NAME".to_string(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to check name uniqueness: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to check name uniqueness".to_string(),
                    code: "DB_ERROR".to_string(),
                }),
            )
                .into_response();
        }
        Ok(None) => {} // OK - 名前は一意
    }

    let mut endpoint = Endpoint::new(req.name, req.base_url);
    endpoint.api_key = req.api_key;
    endpoint.health_check_interval_secs = req.health_check_interval_secs;
    endpoint.inference_timeout_secs = req.inference_timeout_secs;
    endpoint.notes = req.notes;
    if !req.capabilities.is_empty() {
        endpoint.capabilities = req.capabilities;
    }

    match db::create_endpoint(&state.db_pool, &endpoint).await {
        Ok(()) => {
            // EndpointRegistryキャッシュも更新（DBは既に保存済みなのでキャッシュのみ）
            state.endpoint_registry.add_to_cache(endpoint.clone()).await;
            (StatusCode::CREATED, Json(EndpointResponse::from(endpoint))).into_response()
        }
        Err(e) => {
            let error_str = e.to_string();
            if error_str.contains("UNIQUE constraint failed") {
                (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: "Endpoint with this name or URL already exists".to_string(),
                        code: "DUPLICATE_ENDPOINT".to_string(),
                    }),
                )
                    .into_response()
            } else {
                tracing::error!("Failed to create endpoint: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Failed to create endpoint".to_string(),
                        code: "DB_ERROR".to_string(),
                    }),
                )
                    .into_response()
            }
        }
    }
}

/// GET /v0/endpoints - エンドポイント一覧
pub async fn list_endpoints(
    State(state): State<AppState>,
    Query(query): Query<ListEndpointsQuery>,
) -> impl IntoResponse {
    match db::list_endpoints(&state.db_pool).await {
        Ok(endpoints) => {
            // ステータスでフィルタ
            let filtered_endpoints: Vec<Endpoint> = if let Some(ref status) = query.status {
                endpoints
                    .into_iter()
                    .filter(|ep| ep.status.as_str() == status)
                    .collect()
            } else {
                endpoints
            };

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
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to list endpoints".to_string(),
                    code: "DB_ERROR".to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// GET /v0/endpoints/:id - エンドポイント詳細
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
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Endpoint not found".to_string(),
                code: "NOT_FOUND".to_string(),
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to get endpoint: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to get endpoint".to_string(),
                    code: "DB_ERROR".to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// PUT /v0/endpoints/:id - エンドポイント更新
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
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Endpoint not found".to_string(),
                    code: "NOT_FOUND".to_string(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get endpoint for update: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to get endpoint".to_string(),
                    code: "DB_ERROR".to_string(),
                }),
            )
                .into_response();
        }
    };

    // 名前のバリデーション（空文字列は不許可）
    if let Some(ref name) = req.name {
        if name.trim().is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Name cannot be empty".to_string(),
                    code: "INVALID_NAME".to_string(),
                }),
            )
                .into_response();
        }
    }

    // URL形式チェック
    if let Some(ref url) = req.base_url {
        if Url::parse(url).is_err() {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Invalid URL format".to_string(),
                    code: "INVALID_URL_FORMAT".to_string(),
                }),
            )
                .into_response();
        }
    }

    // 名前変更時の重複チェック（他のエンドポイントと重複していないか）
    if let Some(ref new_name) = req.name {
        if new_name != &existing.name {
            match db::find_by_name(&state.db_pool, new_name).await {
                Ok(Some(_)) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            error: format!("Endpoint with name '{}' already exists", new_name),
                            code: "DUPLICATE_NAME".to_string(),
                        }),
                    )
                        .into_response()
                }
                Err(e) => {
                    tracing::error!("Failed to check name uniqueness: {}", e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: "Failed to check name uniqueness".to_string(),
                            code: "DB_ERROR".to_string(),
                        }),
                    )
                        .into_response();
                }
                Ok(None) => {} // OK - 名前は一意
            }
        }
    }

    // 更新内容を適用
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

    match db::update_endpoint(&state.db_pool, &updated).await {
        Ok(true) => (StatusCode::OK, Json(EndpointResponse::from(updated))).into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Endpoint not found".to_string(),
                code: "NOT_FOUND".to_string(),
            }),
        )
            .into_response(),
        Err(e) => {
            let error_str = e.to_string();
            if error_str.contains("UNIQUE constraint failed") {
                (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: "Endpoint with this name or URL already exists".to_string(),
                        code: "DUPLICATE_ENDPOINT".to_string(),
                    }),
                )
                    .into_response()
            } else {
                tracing::error!("Failed to update endpoint: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Failed to update endpoint".to_string(),
                        code: "DB_ERROR".to_string(),
                    }),
                )
                    .into_response()
            }
        }
    }
}

/// DELETE /v0/endpoints/:id - エンドポイント削除
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
    match state.endpoint_registry.remove(id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Endpoint not found".to_string(),
                code: "NOT_FOUND".to_string(),
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to delete endpoint: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to delete endpoint".to_string(),
                    code: "DB_ERROR".to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// POST /v0/endpoints/:id/test - 接続テスト
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
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Endpoint not found".to_string(),
                    code: "NOT_FOUND".to_string(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get endpoint for test: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to get endpoint".to_string(),
                    code: "DB_ERROR".to_string(),
                }),
            )
                .into_response();
        }
    };

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

                // ステータスを更新（DB）
                let _ = db::update_endpoint_status(
                    &state.db_pool,
                    id,
                    EndpointStatus::Online,
                    Some(latency_ms),
                    None,
                )
                .await;

                // EndpointRegistryキャッシュも更新
                let _ = state
                    .endpoint_registry
                    .update_status(id, EndpointStatus::Online, Some(latency_ms), None)
                    .await;

                // SPEC-24157000: Responses API対応を検出
                // /health エンドポイントで supports_responses_api フラグを確認
                let health_url = format!("{}/health", endpoint.base_url.trim_end_matches('/'));
                let mut health_request = state.http_client.get(&health_url);
                if let Some(ref api_key) = endpoint.api_key {
                    health_request =
                        health_request.header("Authorization", format!("Bearer {}", api_key));
                }

                if let Ok(health_response) = health_request
                    .timeout(std::time::Duration::from_secs(10))
                    .send()
                    .await
                {
                    if let Ok(health_json) = health_response.json::<serde_json::Value>().await {
                        let supports_responses_api = health_json["supports_responses_api"]
                            .as_bool()
                            .unwrap_or(false);
                        // EndpointRegistryキャッシュも更新
                        let _ = state
                            .endpoint_registry
                            .update_responses_api_support(id, supports_responses_api)
                            .await;
                        tracing::debug!(
                            endpoint_id = %id,
                            supports_responses_api = supports_responses_api,
                            "Detected Responses API support"
                        );
                    }
                }

                // モデル数を計算
                let model_count = models_found.as_ref().map(|m| m.len()).unwrap_or(0);

                (
                    StatusCode::OK,
                    Json(TestConnectionResponse {
                        success: true,
                        latency_ms: Some(latency_ms),
                        error: None,
                        models_found,
                        endpoint_info: Some(EndpointTestInfo { model_count }),
                    }),
                )
                    .into_response()
            } else {
                let error_msg = format!("HTTP {}", response.status());
                let _ = db::update_endpoint_status(
                    &state.db_pool,
                    id,
                    EndpointStatus::Error,
                    None,
                    Some(&error_msg),
                )
                .await;

                (
                    StatusCode::OK,
                    Json(TestConnectionResponse {
                        success: false,
                        latency_ms: Some(latency_ms),
                        error: Some(error_msg),
                        models_found: None,
                        endpoint_info: None,
                    }),
                )
                    .into_response()
            }
        }
        Err(e) => {
            let error_msg = e.to_string();
            let _ = db::update_endpoint_status(
                &state.db_pool,
                id,
                EndpointStatus::Error,
                None,
                Some(&error_msg),
            )
            .await;

            (
                StatusCode::OK,
                Json(TestConnectionResponse {
                    success: false,
                    latency_ms: None,
                    error: Some(error_msg),
                    models_found: None,
                    endpoint_info: None,
                }),
            )
                .into_response()
        }
    }
}

/// POST /v0/endpoints/:id/sync - モデル一覧同期
pub async fn sync_endpoint_models(
    Extension(claims): Extension<Claims>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    // Admin権限チェック
    if let Err(e) = ensure_admin(&claims) {
        return e.into_response();
    }

    use std::collections::HashSet;

    // エンドポイントを取得
    let endpoint = match db::get_endpoint(&state.db_pool, id).await {
        Ok(Some(ep)) => ep,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Endpoint not found".to_string(),
                    code: "NOT_FOUND".to_string(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get endpoint for sync: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to get endpoint".to_string(),
                    code: "DB_ERROR".to_string(),
                }),
            )
                .into_response();
        }
    };

    // 既存モデルを取得して比較用にIDセットを作成
    let existing_models: HashSet<String> = match db::list_endpoint_models(&state.db_pool, id).await
    {
        Ok(models) => models.into_iter().map(|m| m.model_id).collect(),
        Err(_) => HashSet::new(),
    };

    // GET /v1/models でモデル一覧を取得
    let url = format!("{}/v1/models", endpoint.base_url.trim_end_matches('/'));

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

    match result {
        Ok(response) => {
            if !response.status().is_success() {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(ErrorResponse {
                        error: format!("Endpoint returned HTTP {}", response.status()),
                        code: "ENDPOINT_ERROR".to_string(),
                    }),
                )
                    .into_response();
            }

            let json: serde_json::Value = match response.json().await {
                Ok(j) => j,
                Err(e) => {
                    return (
                        StatusCode::BAD_GATEWAY,
                        Json(ErrorResponse {
                            error: format!("Failed to parse response: {}", e),
                            code: "PARSE_ERROR".to_string(),
                        }),
                    )
                        .into_response()
                }
            };

            // 既存モデルを削除
            let _ = db::delete_all_endpoint_models(&state.db_pool, id).await;

            // 新しいモデルを追加
            let now = chrono::Utc::now();
            let mut synced_models = Vec::new();
            let mut new_model_ids: HashSet<String> = HashSet::new();

            // OpenAI形式: { "data": [{ "id": "model-name", ... }] }
            if let Some(data) = json["data"].as_array() {
                for model in data {
                    if let Some(model_id) = model["id"].as_str() {
                        new_model_ids.insert(model_id.to_string());
                        let ep_model = EndpointModel {
                            endpoint_id: id,
                            model_id: model_id.to_string(),
                            capabilities: None,
                            last_checked: Some(now),
                            supported_apis: vec![SupportedAPI::ChatCompletions],
                        };
                        let _ = db::add_endpoint_model(&state.db_pool, &ep_model).await;
                        synced_models.push(EndpointModelResponse::from(ep_model));
                    }
                }
            }
            // Ollama形式: { "models": [{ "name": "...", "model": "..." }] }
            else if let Some(models) = json["models"].as_array() {
                for model in models {
                    let model_id = model["name"]
                        .as_str()
                        .or(model["model"].as_str())
                        .unwrap_or_default();
                    if !model_id.is_empty() {
                        new_model_ids.insert(model_id.to_string());
                        let ep_model = EndpointModel {
                            endpoint_id: id,
                            model_id: model_id.to_string(),
                            capabilities: None,
                            last_checked: Some(now),
                            supported_apis: vec![SupportedAPI::ChatCompletions],
                        };
                        let _ = db::add_endpoint_model(&state.db_pool, &ep_model).await;
                        synced_models.push(EndpointModelResponse::from(ep_model));
                    }
                }
            }

            // EndpointRegistryキャッシュをリロードしてモデルマッピングを更新
            let _ = state.endpoint_registry.reload().await;

            // 変更カウントを計算
            let added = new_model_ids.difference(&existing_models).count();
            let removed = existing_models.difference(&new_model_ids).count();
            let updated = new_model_ids.intersection(&existing_models).count();

            (
                StatusCode::OK,
                Json(SyncModelsResponse {
                    synced_models,
                    added,
                    removed,
                    updated,
                }),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: format!("Failed to connect: {}", e),
                code: "CONNECTION_ERROR".to_string(),
            }),
        )
            .into_response(),
    }
}

/// GET /v0/endpoints/:id/models - エンドポイントのモデル一覧
pub async fn list_endpoint_models(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    // エンドポイント存在確認
    match db::get_endpoint(&state.db_pool, id).await {
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Endpoint not found".to_string(),
                    code: "NOT_FOUND".to_string(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get endpoint: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to get endpoint".to_string(),
                    code: "DB_ERROR".to_string(),
                }),
            )
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
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to list models".to_string(),
                    code: "DB_ERROR".to_string(),
                }),
            )
                .into_response()
        }
    }
}
