//! モデル管理API
//!
//! モデル一覧取得、配布、進捗追跡のエンドポイント

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    ollama::OllamaClient,
    registry::models::{DownloadTask, InstalledModel, ModelInfo},
    tasks::DownloadTaskManager,
    AppState,
};
use ollama_coordinator_common::error::CoordinatorError;

/// 利用可能なモデル一覧のレスポンス
#[derive(Debug, Serialize)]
pub struct AvailableModelsResponse {
    /// モデル一覧
    pub models: Vec<ModelInfo>,
    /// ソース（"ollama_library" または "agents"）
    pub source: String,
}

/// モデル配布リクエスト
#[derive(Debug, Deserialize)]
pub struct DistributeModelsRequest {
    /// モデル名
    pub model_name: String,
    /// ターゲット（"all" または "specific"）
    pub target: String,
    /// エージェントID一覧（targetが"specific"の場合）
    #[serde(default)]
    pub agent_ids: Vec<Uuid>,
}

/// モデル配布レスポンス
#[derive(Debug, Serialize)]
pub struct DistributeModelsResponse {
    /// タスクID一覧
    pub task_ids: Vec<Uuid>,
}

/// モデルプルリクエスト
#[derive(Debug, Deserialize)]
pub struct PullModelRequest {
    /// モデル名
    pub model_name: String,
}

/// モデルプルレスポンス
#[derive(Debug, Serialize)]
pub struct PullModelResponse {
    /// タスクID
    pub task_id: Uuid,
}

/// Axum用のエラーレスポンス型
#[derive(Debug)]
pub struct AppError(CoordinatorError);

impl From<CoordinatorError> for AppError {
    fn from(err: CoordinatorError) -> Self {
        AppError(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self.0 {
            CoordinatorError::AgentNotFound(_) => (StatusCode::NOT_FOUND, self.0.to_string()),
            CoordinatorError::NoAgentsAvailable => {
                (StatusCode::SERVICE_UNAVAILABLE, self.0.to_string())
            }
            CoordinatorError::Database(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string())
            }
            CoordinatorError::Http(_) => (StatusCode::BAD_GATEWAY, self.0.to_string()),
            CoordinatorError::Timeout(_) => (StatusCode::GATEWAY_TIMEOUT, self.0.to_string()),
            CoordinatorError::Internal(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string())
            }
            CoordinatorError::Common(err) => (StatusCode::BAD_REQUEST, err.to_string()),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

/// T027: GET /api/models/available - 利用可能なモデル一覧を取得
pub async fn get_available_models(
    State(_state): State<AppState>,
) -> Result<Json<AvailableModelsResponse>, AppError> {
    let client = OllamaClient::new()?;

    // 事前定義モデルを取得（エージェントからの取得は後で実装）
    let models = client.get_predefined_models();

    Ok(Json(AvailableModelsResponse {
        models,
        source: "ollama_library".to_string(),
    }))
}

/// T028: POST /api/models/distribute - モデルを配布
pub async fn distribute_models(
    State(state): State<AppState>,
    Json(request): Json<DistributeModelsRequest>,
) -> Result<(StatusCode, Json<DistributeModelsResponse>), AppError> {
    // タスクマネージャーを取得（後で AppState に追加）
    let _task_manager = DownloadTaskManager::new();

    // ターゲットエージェントを決定
    let agent_ids = match request.target.as_str() {
        "all" => {
            // 全エージェントを取得
            let agents = state.registry.list().await;
            agents.into_iter().map(|a| a.id).collect()
        }
        "specific" => request.agent_ids.clone(),
        _ => {
            return Err(CoordinatorError::Internal(
                "Invalid target. Must be 'all' or 'specific'".to_string(),
            )
            .into());
        }
    };

    // 各エージェントID が存在することを確認
    for agent_id in &agent_ids {
        // get() は Result<Agent, Error> を返し、存在しない場合はエラーを返す
        state.registry.get(*agent_id).await?;
    }

    // タスクを作成（実際の配布は後で実装）
    let mut task_ids = Vec::new();
    for agent_id in agent_ids {
        let task_id = Uuid::new_v4(); // Placeholder
        task_ids.push(task_id);

        // TODO: 実際のタスクを作成してエージェントに配布
        tracing::info!(
            "Created distribution task {} for agent {} with model {}",
            task_id,
            agent_id,
            request.model_name
        );
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(DistributeModelsResponse { task_ids }),
    ))
}

/// T029: GET /api/agents/{agent_id}/models - エージェントのインストール済みモデル一覧を取得
pub async fn get_agent_models(
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<Vec<InstalledModel>>, AppError> {
    // エージェントが存在することを確認
    let agent = state.registry.get(agent_id).await?;

    // エージェントからモデル一覧を取得（実装は後で）
    let agent_url = format!("http://{}:{}", agent.ip_address, agent.ollama_port);
    tracing::info!("Fetching models from agent at {}", agent_url);

    // TODO: エージェントのOllama APIからモデル一覧を取得
    // 現在は空の配列を返す
    Ok(Json(Vec::new()))
}

/// T030: POST /api/agents/{agent_id}/models/pull - エージェントにモデルプルを指示
pub async fn pull_model_to_agent(
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
    Json(request): Json<PullModelRequest>,
) -> Result<(StatusCode, Json<PullModelResponse>), AppError> {
    // エージェントが存在することを確認
    let _agent = state.registry.get(agent_id).await?;

    // タスクを作成
    let task_id = Uuid::new_v4(); // Placeholder

    // TODO: 実際のタスクを作成してエージェントに配布
    tracing::info!(
        "Created pull task {} for agent {} with model {}",
        task_id,
        agent_id,
        request.model_name
    );

    Ok((StatusCode::ACCEPTED, Json(PullModelResponse { task_id })))
}

/// T031: GET /api/tasks/{task_id} - タスク進捗を取得
pub async fn get_task_progress(
    State(_state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> Result<Json<DownloadTask>, AppError> {
    // タスクマネージャーから取得（後で AppState に追加）
    let _task_manager = DownloadTaskManager::new();

    // TODO: タスクマネージャーからタスクを取得
    // 現在はダミーのタスクを返す
    Err(CoordinatorError::Internal(format!("Task {} not found", task_id)).into())
}
