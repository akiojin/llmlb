//! SPEC-e8e9326e: モデルのダウンロード・削除・メタデータ取得ハンドラー

use super::{
    ensure_admin, sync_error_to_lb_error, DownloadModelRequest, DownloadProgressResponse,
    DownloadTaskResponse, ModelInfoResponse,
};
use crate::api::error::AppError;
use crate::common::auth::Claims;
use crate::common::error::{CommonError, LbError};
use crate::db::{download_tasks as tasks_db, endpoints as db};
use crate::sync;
use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use serde::Deserialize;
use uuid::Uuid;

/// POST /api/endpoints/:id/download - モデルダウンロードリクエスト（マルチエンジン対応）
///
/// 対応エンドポイントタイプ:
/// - xLLM: タスク作成のみ（xLLM側がダウンロードを管理）
/// - Ollama: POST /api/pull でバックグラウンドダウンロード
/// - LM Studio: POST /api/v1/models/download でファイアアンドフォーゲット
/// - vLLM/OpenaiCompatible: 非対応
pub async fn download_model(
    Extension(claims): Extension<Claims>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<DownloadModelRequest>,
) -> impl IntoResponse {
    // Admin権限チェック
    if let Err(e) = ensure_admin(&claims) {
        return e.into_response();
    }

    // エンドポイント取得
    let endpoint = match db::get_endpoint(&state.db_pool, id).await {
        Ok(Some(ep)) => ep,
        Ok(None) => return AppError(LbError::EndpointNotFound(id)).into_response(),
        Err(e) => {
            tracing::error!("Failed to get endpoint: {}", e);
            return AppError(LbError::Database("Failed to get endpoint".to_string()))
                .into_response();
        }
    };

    // ダウンロード対応チェック
    if !endpoint.endpoint_type.supports_model_download() {
        return AppError(LbError::Common(CommonError::Validation(format!(
            "Model download is not supported for {} endpoints",
            endpoint.endpoint_type.as_str()
        ))))
        .into_response();
    }

    // ダウンロードディスパッチャーへ委譲
    let download_req = crate::download::DownloadRequest {
        model: req.model,
        hf_repo: req.hf_repo,
        quantization: req.quantization,
    };

    match crate::download::download_model(
        &state.http_client,
        &endpoint.base_url,
        endpoint.api_key.as_deref(),
        &endpoint.endpoint_type,
        &download_req,
        &state.db_pool,
        id,
    )
    .await
    {
        Ok(task) => (StatusCode::ACCEPTED, Json(DownloadTaskResponse::from(task))).into_response(),
        Err(e) => {
            tracing::error!("Download failed for endpoint {}: {}", id, e);
            AppError(LbError::Common(CommonError::Validation(e.to_string()))).into_response()
        }
    }
}

/// DELETE /api/endpoints/:id/models/:model - モデル削除
///
/// 対応エンドポイントタイプ:
/// - Ollama: DELETE /api/delete で削除
/// - xLLM: 未対応（APIなし）
/// - LM Studio: 未対応（0.4.6時点でAPIなし）
/// - vLLM/OpenaiCompatible: 未対応
pub async fn delete_endpoint_model_handler(
    Extension(claims): Extension<Claims>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<DeleteModelRequest>,
) -> impl IntoResponse {
    // Admin権限チェック
    if let Err(e) = ensure_admin(&claims) {
        return e.into_response();
    }

    let model = req.model;

    // エンドポイント取得
    let endpoint = match db::get_endpoint(&state.db_pool, id).await {
        Ok(Some(ep)) => ep,
        Ok(None) => return AppError(LbError::EndpointNotFound(id)).into_response(),
        Err(e) => {
            tracing::error!("Failed to get endpoint: {}", e);
            return AppError(LbError::Database("Failed to get endpoint".to_string()))
                .into_response();
        }
    };

    // 削除対応チェック
    if !endpoint.endpoint_type.supports_model_delete() {
        return AppError(LbError::Common(CommonError::Validation(format!(
            "Model delete is not supported for {} endpoints",
            endpoint.endpoint_type.as_str()
        ))))
        .into_response();
    }

    // 削除ディスパッチャーへ委譲
    match crate::delete::delete_model(
        &state.http_client,
        &endpoint.base_url,
        endpoint.api_key.as_deref(),
        &endpoint.endpoint_type,
        &model,
    )
    .await
    {
        Ok(()) => {
            tracing::info!(
                "Model '{}' deleted from endpoint {} ({})",
                model,
                id,
                endpoint.endpoint_type.as_str()
            );

            let engine_model =
                crate::models::mapping::resolve_engine_name(&model, &endpoint.endpoint_type);
            if let Err(error) = sync::sync_models_with_type(
                &state.db_pool,
                &state.http_client,
                id,
                &endpoint.base_url,
                endpoint.api_key.as_deref(),
                endpoint.inference_timeout_secs as u64,
                Some(endpoint.endpoint_type),
            )
            .await
            {
                tracing::error!(
                    endpoint_id = %id,
                    model = %model,
                    error = %error,
                    "Upstream model was deleted but post-delete model sync failed"
                );
                return AppError(sync_error_to_lb_error(error)).into_response();
            }

            let synced_models = match db::list_endpoint_models(&state.db_pool, id).await {
                Ok(models) => models,
                Err(error) => {
                    tracing::error!(
                        endpoint_id = %id,
                        model = %model,
                        error = %error,
                        "Upstream model was deleted but post-delete reconciliation could not be verified"
                    );
                    return AppError(LbError::Database(
                        "Failed to verify post-delete model reconciliation".to_string(),
                    ))
                    .into_response();
                }
            };

            if synced_models.iter().any(|synced_model| {
                synced_model.model_id == model
                    || engine_model.is_some_and(|name| synced_model.model_id == name)
            }) {
                tracing::error!(
                    endpoint_id = %id,
                    model = %model,
                    engine_model = engine_model.unwrap_or(model.as_str()),
                    error = "deleted model remains in endpoint_models after sync",
                    "Upstream model was deleted but post-delete reconciliation failed"
                );
                return AppError(LbError::Internal(
                    "Post-delete model reconciliation left stale metadata".to_string(),
                ))
                .into_response();
            }

            if let Err(error) = state
                .balancer
                .endpoint_registry
                .refresh_model_mappings(id)
                .await
            {
                tracing::error!(
                    endpoint_id = %id,
                    model = %model,
                    error = %error,
                    "Upstream model was deleted but model mappings could not be refreshed"
                );
                return AppError(LbError::Database(
                    "Failed to refresh post-delete model mappings".to_string(),
                ))
                .into_response();
            }

            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            tracing::error!(
                "Model delete failed for '{}' on endpoint {}: {}",
                model,
                id,
                e
            );
            AppError(LbError::Common(CommonError::Validation(e.to_string()))).into_response()
        }
    }
}

/// GET /api/endpoints/:id/download/progress - ダウンロード進捗一覧
pub async fn download_progress(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    // エンドポイント存在確認
    match db::get_endpoint(&state.db_pool, id).await {
        Ok(None) => return AppError(LbError::EndpointNotFound(id)).into_response(),
        Err(e) => {
            tracing::error!("Failed to get endpoint: {}", e);
            return AppError(LbError::Database("Failed to get endpoint".to_string()))
                .into_response();
        }
        Ok(Some(_)) => {}
    }

    // ダウンロードタスク一覧取得
    match tasks_db::list_download_tasks(&state.db_pool, id).await {
        Ok(tasks) => (
            StatusCode::OK,
            Json(DownloadProgressResponse {
                endpoint_id: id,
                tasks: tasks.into_iter().map(DownloadTaskResponse::from).collect(),
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to list download tasks: {}", e);
            AppError(LbError::Database(
                "Failed to list download tasks".to_string(),
            ))
            .into_response()
        }
    }
}

/// モデル削除リクエストボディ
#[derive(Debug, Deserialize)]
pub struct DeleteModelRequest {
    /// 削除するモデル名
    pub model: String,
}

/// モデル情報取得のパスパラメータ
#[derive(Debug, Deserialize)]
pub struct ModelInfoPath {
    /// エンドポイントID
    pub id: Uuid,
    /// モデルID
    pub model: String,
}

/// GET /api/endpoints/:id/models/:model/info - モデルメタデータ取得
pub async fn get_model_info(
    State(state): State<AppState>,
    Path(params): Path<ModelInfoPath>,
) -> impl IntoResponse {
    let ModelInfoPath { id, model } = params;

    // エンドポイント取得
    let endpoint = match db::get_endpoint(&state.db_pool, id).await {
        Ok(Some(ep)) => ep,
        Ok(None) => return AppError(LbError::EndpointNotFound(id)).into_response(),
        Err(e) => {
            tracing::error!("Failed to get endpoint: {}", e);
            return AppError(LbError::Database("Failed to get endpoint".to_string()))
                .into_response();
        }
    };

    // SPEC-e8e9326e: メタデータ取得はxLLM/Ollamaのみサポート
    if !endpoint.endpoint_type.supports_model_metadata() {
        return AppError(LbError::Common(CommonError::Validation(
            "Model metadata retrieval is not supported for this endpoint type".to_string(),
        )))
        .into_response();
    }

    // モデル一覧からモデルを検索
    match db::list_endpoint_models(&state.db_pool, id).await {
        Ok(models) => {
            if let Some(found) = models.into_iter().find(|m| m.model_id == model) {
                (
                    StatusCode::OK,
                    Json(ModelInfoResponse {
                        model_id: found.model_id,
                        endpoint_id: id,
                        max_tokens: found.max_tokens,
                        last_checked: found.last_checked.map(|dt| dt.to_rfc3339()),
                    }),
                )
                    .into_response()
            } else {
                AppError(LbError::NotFound(format!(
                    "Model '{}' not found on this endpoint",
                    model
                )))
                .into_response()
            }
        }
        Err(e) => {
            tracing::error!("Failed to list endpoint models: {}", e);
            AppError(LbError::Database("Failed to get model info".to_string())).into_response()
        }
    }
}
