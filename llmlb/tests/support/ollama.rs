use axum::{
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use super::http::{spawn_lb, TestServer};

#[derive(Debug, Deserialize)]
struct ShowRequest {
    model: String,
}

/// Ollamaモックサーバーを起動する
///
/// 最低限のエンドポイントのみ実装:
/// - GET /api/tags（Ollama判別用）
/// - POST /api/show（メタデータ取得用）
/// - GET /v1/models（モデル同期用）
#[allow(dead_code)]
pub async fn spawn_mock_ollama() -> TestServer {
    async fn tags() -> impl IntoResponse {
        Json(json!({
            "models": [
                { "name": "llama3:8b", "size": 4_000_000_000i64 }
            ]
        }))
    }

    async fn show(Json(req): Json<ShowRequest>) -> impl IntoResponse {
        Json(json!({
            "model": req.model,
            "details": {
                "family": "llama",
                "parameter_size": "8B",
                "quantization_level": "Q4_K_M"
            },
            "parameters": {
                "num_ctx": 8192
            }
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
                    "owned_by": "ollama"
                }
            ]
        }))
    }

    let app = Router::new()
        .route("/api/tags", get(tags))
        .route("/api/show", post(show))
        .route("/v1/models", get(v1_models));

    spawn_lb(app).await
}
