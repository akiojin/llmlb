use axum::{extract::Path, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use serde_json::json;

use super::http::{spawn_lb, TestServer};

/// xLLMモックサーバーを起動する
///
/// 最低限のエンドポイントのみ実装:
/// - GET /api/system（xLLM判別用）
/// - GET /v1/models（モデル同期用）
/// - GET /api/models/:model/info（メタデータ取得用）
#[allow(dead_code)]
pub async fn spawn_mock_xllm() -> TestServer {
    async fn system() -> impl IntoResponse {
        Json(json!({
            "xllm_version": "0.1.0",
            "server_name": "mock-xllm"
        }))
    }

    async fn v1_models() -> impl IntoResponse {
        Json(json!({
            "object": "list",
            "data": [
                {
                    "id": "llama3:8b",
                    "object": "model",
                    "created": 0,
                    "owned_by": "xllm"
                }
            ]
        }))
    }

    async fn model_info(Path(model): Path<String>) -> impl IntoResponse {
        if model.contains("nonexistent-model") {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "model not found"
                })),
            )
                .into_response();
        }

        Json(json!({
            "model": model,
            "context_length": 8192,
            "size_bytes": 1_500_000_000u64,
            "quantization": "Q4_K_M",
            "family": "llama",
            "parameter_size": "1B"
        }))
        .into_response()
    }

    let app = Router::new()
        .route("/api/system", get(system))
        .route("/v1/models", get(v1_models))
        .route("/api/models/{model}/info", get(model_info));

    spawn_lb(app).await
}
