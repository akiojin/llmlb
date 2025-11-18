//! ノードメトリクスAPIハンドラー

use crate::AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use ollama_router_common::types::AgentMetrics;

use super::agent::AppError;

/// POST /api/nodes/:id/metrics - ノードメトリクス更新
///
/// ノードから送信されたメトリクス情報（CPU使用率、メモリ使用率、アクティブリクエスト数等）を
/// メモリ内のHashMapに保存する。ノードが存在しない場合は404を返す。
pub async fn update_metrics(
    State(state): State<AppState>,
    axum::extract::Path(node_id): axum::extract::Path<uuid::Uuid>,
    Json(mut metrics): Json<AgentMetrics>,
) -> Result<impl IntoResponse, AppError> {
    // パスパラメータのnode_idとリクエストボディのnode_idを統一
    metrics.node_id = node_id;

    // registryのupdate_metrics()を呼び出し
    state.registry.update_metrics(metrics).await?;

    // 204 No Content を返す
    Ok(StatusCode::NO_CONTENT)
}
