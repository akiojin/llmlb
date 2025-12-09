//! ノードランタイムクライアント（llama.cpp）
//!
//! ノード経由でモデル情報を取得し、事前定義リストと統合

use crate::registry::models::ModelInfo;
use llm_router_common::error::{RouterError, RouterResult};
use reqwest::Client;
use reqwest::StatusCode;
use serde::Deserialize;
use std::time::Duration;
use tracing::{debug, warn};

/// ノードランタイムクライアント
pub struct RuntimeClient {
    http_client: Client,
}

/// ノードAPIのモデル一覧レスポンス
#[derive(Debug, Deserialize)]
struct RuntimeModelsResponse {
    models: Vec<RuntimeModel>,
}

/// ノードランタイムから返されるモデル情報
#[derive(Debug, Deserialize)]
struct RuntimeModel {
    name: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    #[allow(dead_code)] // 将来の使用のために保持
    digest: Option<String>,
    #[serde(default)]
    details: Option<RuntimeModelDetails>,
}

/// ノードランタイムのモデル詳細
#[derive(Debug, Deserialize)]
struct RuntimeModelDetails {
    #[serde(default)]
    parameter_size: Option<String>,
    #[serde(default)]
    quantization_level: Option<String>,
}

impl RuntimeClient {
    /// 新しいRuntimeClientを作成
    pub fn new() -> RouterResult<Self> {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| RouterError::Internal(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { http_client })
    }

    /// ノードからモデル一覧を取得
    ///
    /// # Arguments
    /// * `agent_base_url` - ノードのベースURL（例: "http://192.168.1.10:11434"）
    pub async fn fetch_models_from_agent(
        &self,
        agent_base_url: &str,
    ) -> RouterResult<Vec<ModelInfo>> {
        let url = format!("{}/api/tags", agent_base_url);

        debug!("Fetching models from agent: {}", url);

        let response = self.http_client.get(&url).send().await.map_err(|e| {
            RouterError::Internal(format!("Failed to fetch models from agent: {}", e))
        })?;

        if !response.status().is_success() {
            return Err(RouterError::Internal(format!(
                "Failed to fetch models: HTTP {}",
                response.status()
            )));
        }

        let tags_response: RuntimeModelsResponse = response.json().await.map_err(|e| {
            RouterError::Internal(format!("Failed to parse models response: {}", e))
        })?;

        let models = tags_response
            .models
            .into_iter()
            .map(|m| self.convert_runtime_model(m))
            .collect();

        Ok(models)
    }

    /// ノード側のランタイムが起動しているか簡易ヘルスチェック
    pub async fn check_runtime_health(&self, agent_base_url: &str) -> RouterResult<()> {
        let url = format!("{}/api/version", agent_base_url);
        debug!("Checking runtime health: {}", url);

        let response = self.http_client.get(&url).send().await.map_err(|e| {
            RouterError::Internal(format!("Failed to connect to agent runtime: {}", e))
        })?;

        if response.status() == StatusCode::NOT_FOUND {
            // /api/version がない場合でも200以外ならエラーとせず通す（古いバージョン向け）
            return Ok(());
        }

        if !response.status().is_success() {
            return Err(RouterError::Internal(format!(
                "Node runtime health check failed: HTTP {}",
                response.status()
            )));
        }

        Ok(())
    }

    /// 事前定義モデルリストを取得
    pub fn get_predefined_models(&self) -> Vec<ModelInfo> {
        // プリセットモデルをソースコードに埋め込まない運用に変更
        Vec::new()
    }

    /// ノードから取得したモデルと事前定義リストをマージ
    pub async fn get_available_models(
        &self,
        agent_base_urls: Vec<String>,
    ) -> RouterResult<Vec<ModelInfo>> {
        let mut all_models = Vec::new();
        let mut model_names = std::collections::HashSet::new();

        // ノードからモデルを取得
        for node_url in agent_base_urls {
            match self.fetch_models_from_agent(&node_url).await {
                Ok(models) => {
                    for model in models {
                        if model_names.insert(model.name.clone()) {
                            all_models.push(model);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to fetch models from {}: {}", node_url, e);
                }
            }
        }

        // 事前定義モデルを追加（重複を避ける）
        for model in self.get_predefined_models() {
            if model_names.insert(model.name.clone()) {
                all_models.push(model);
            }
        }

        Ok(all_models)
    }

    /// ランタイムモデルをModelInfoに変換
    fn convert_runtime_model(&self, runtime_model: RuntimeModel) -> ModelInfo {
        let description = if let Some(details) = &runtime_model.details {
            format!(
                "{} ({})",
                details.parameter_size.as_deref().unwrap_or("unknown size"),
                details
                    .quantization_level
                    .as_deref()
                    .unwrap_or("unknown quantization")
            )
        } else {
            "No description available".to_string()
        };

        // モデルサイズから必要メモリを推定（1.5倍）
        let required_memory = (runtime_model.size as f64 * 1.5) as u64;

        ModelInfo::new(
            runtime_model.name,
            runtime_model.size,
            description,
            required_memory,
            vec!["llm".to_string()],
        )
    }
}

impl Default for RuntimeClient {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_predefined_models() {
        let client = RuntimeClient::new().unwrap();
        let models = client.get_predefined_models();

        assert!(models.is_empty());
    }

    #[test]
    fn test_convert_runtime_model() {
        let client = RuntimeClient::new().unwrap();

        let runtime_model = RuntimeModel {
            name: "test-model:latest".to_string(),
            size: 5_000_000_000,
            digest: Some("abc123".to_string()),
            details: Some(RuntimeModelDetails {
                parameter_size: Some("7B".to_string()),
                quantization_level: Some("Q4_K_M".to_string()),
            }),
        };

        let model_info = client.convert_runtime_model(runtime_model);

        assert_eq!(model_info.name, "test-model:latest");
        assert_eq!(model_info.size, 5_000_000_000);
        assert!(model_info.description.contains("7B"));
        assert!(model_info.description.contains("Q4_K_M"));
        assert_eq!(model_info.required_memory, 7_500_000_000); // 5GB * 1.5
    }
}
