//! モデル情報の永続化 (簡易JSON)

use llm_router_common::error::{RouterError, RouterResult};
use std::path::PathBuf;
use tokio::fs;
use tracing::warn;

use crate::registry::models::ModelInfo;

fn get_models_file_path() -> RouterResult<PathBuf> {
    let data_dir = if let Ok(test_dir) = std::env::var("LLM_ROUTER_DATA_DIR") {
        PathBuf::from(test_dir)
    } else {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map_err(|_| RouterError::Database("Failed to get home directory".to_string()))?;
        PathBuf::from(home).join(".llm-router")
    };
    Ok(data_dir.join("models.json"))
}

/// モデル一覧を保存
pub async fn save_models(models: &[ModelInfo]) -> RouterResult<()> {
    let file = get_models_file_path()?;
    if let Some(parent) = file.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| RouterError::Database(format!("Failed to create data dir: {}", e)))?;
        }
    }
    let json = serde_json::to_string_pretty(models)
        .map_err(|e| RouterError::Database(format!("Failed to serialize models: {}", e)))?;
    fs::write(&file, json)
        .await
        .map_err(|e| RouterError::Database(format!("Failed to write models file: {}", e)))?;
    Ok(())
}

/// モデル一覧を読み込み
pub async fn load_models() -> RouterResult<Vec<ModelInfo>> {
    let file = get_models_file_path()?;
    if !file.exists() {
        return Ok(vec![]);
    }
    let content = fs::read_to_string(&file)
        .await
        .map_err(|e| RouterError::Database(format!("Failed to read models file: {}", e)))?;
    if content.trim().is_empty() {
        return Ok(vec![]);
    }
    match serde_json::from_str::<Vec<ModelInfo>>(&content) {
        Ok(v) => Ok(v),
        Err(e) => {
            warn!("Failed to parse models.json: {}. Resetting to empty.", e);
            Ok(vec![])
        }
    }
}
