//! ノード登録管理
//!
//! ノードの状態をメモリ内で管理し、データベースと同期

pub mod models;

use chrono::Utc;
use ollama_router_common::{
    error::{RouterError, RouterResult},
    protocol::{RegisterRequest, RegisterResponse, RegisterStatus},
    types::{AgentMetrics, GpuDeviceInfo, Node, NodeStatus},
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use uuid::Uuid;

/// ノードレジストリ
#[derive(Clone)]
pub struct NodeRegistry {
    nodes: Arc<RwLock<HashMap<Uuid, Node>>>,
    metrics: Arc<RwLock<HashMap<Uuid, AgentMetrics>>>,
    storage_enabled: bool,
}

impl NodeRegistry {
    /// 新しいレジストリを作成
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(HashMap::new())),
            storage_enabled: false,
        }
    }

    /// ストレージ初期化付きでレジストリを作成
    pub async fn with_storage() -> RouterResult<Self> {
        // ストレージ初期化
        crate::db::init_storage().await?;

        let registry = Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(HashMap::new())),
            storage_enabled: true,
        };

        // 起動時にストレージからノード情報を読み込み
        registry.load_from_storage().await?;

        Ok(registry)
    }

    /// ストレージからノード情報を読み込み
    async fn load_from_storage(&self) -> RouterResult<()> {
        if !self.storage_enabled {
            return Ok(());
        }

        let loaded_agents = crate::db::load_agents().await?;
        let mut nodes = self.nodes.write().await;

        let mut removed_count = 0;
        let mut removed_ids = Vec::new();
        let mut sanitized_agents = Vec::new();

        for mut agent in loaded_agents {
            // GPU非搭載 or 情報欠落ノードは削除対象
            if !agent.gpu_available {
                info!(
                    node_id = %agent.id,
                    machine_name = %agent.machine_name,
                    reason = "gpu_available is false",
                    "Removing GPU-less agent from database during startup cleanup"
                );
                removed_count += 1;
                removed_ids.push(agent.id);
                continue;
            }

            let mut sanitized = false;

            if agent.gpu_devices.is_empty() {
                if let Some(model) = agent.gpu_model.clone() {
                    let count = agent.gpu_count.unwrap_or(1).max(1);
                    agent.gpu_devices = vec![GpuDeviceInfo {
                        model,
                        count,
                        memory: None,
                    }];
                    sanitized = true;
                } else {
                    info!(
                        node_id = %agent.id,
                        machine_name = %agent.machine_name,
                        reason = "gpu_devices array is empty and gpu_model is None",
                        "Removing agent with missing GPU device information from database"
                    );
                    removed_count += 1;
                    removed_ids.push(agent.id);
                    continue;
                }
            }

            if !agent.gpu_devices.iter().all(|device| device.is_valid()) {
                info!(
                    node_id = %agent.id,
                    machine_name = %agent.machine_name,
                    reason = "gpu_devices contains invalid device (empty model or zero count)",
                    "Removing agent with invalid GPU device information from database"
                );
                removed_count += 1;
                removed_ids.push(agent.id);
                continue;
            }

            if sanitized {
                sanitized_agents.push(agent.clone());
            }

            nodes.insert(agent.id, agent);
        }

        info!(
            agents_loaded = nodes.len(),
            agents_removed = removed_count,
            "Completed agent registry initialization from storage"
        );

        drop(nodes);

        // 削除対象ノードをデータベースから削除
        for id in removed_ids {
            if let Err(err) = crate::db::delete_agent(id).await {
                error!(
                    node_id = %id,
                    error = %err,
                    "Failed to delete GPU-less agent from database during cleanup"
                );
            }
        }

        // サニタイズされたノード情報をストレージに保存
        for agent in sanitized_agents {
            if let Err(err) = self.save_to_storage(&agent).await {
                warn!(
                    node_id = %agent.id,
                    machine_name = %agent.machine_name,
                    error = %err,
                    "Failed to persist sanitized agent data to storage"
                );
            }
        }

        Ok(())
    }

    /// ノードをストレージに保存
    async fn save_to_storage(&self, agent: &Node) -> RouterResult<()> {
        if !self.storage_enabled {
            return Ok(());
        }

        crate::db::save_agent(agent).await
    }

    /// ノードを登録
    pub async fn register(&self, req: RegisterRequest) -> RouterResult<RegisterResponse> {
        let mut nodes = self.nodes.write().await;

        // 同じマシン名のノードが既に存在するか確認
        let existing = nodes
            .values()
            .find(|a| a.machine_name == req.machine_name && a.ollama_port == req.ollama_port)
            .map(|a| a.id);

        let (node_id, status, agent) = if let Some(id) = existing {
            // 既存ノードを更新
            let agent = nodes.get_mut(&id).unwrap();
            let now = Utc::now();
            let was_online = agent.status == NodeStatus::Online;
            agent.ip_address = req.ip_address;
            agent.ollama_version = req.ollama_version.clone();
            agent.ollama_port = req.ollama_port;
            agent.gpu_available = req.gpu_available;
            agent.gpu_devices = req.gpu_devices.clone();
            agent.gpu_count = req.gpu_count;
            agent.gpu_model = req.gpu_model.clone();
            agent.status = NodeStatus::Online;
            agent.last_seen = now;
            if !was_online || agent.online_since.is_none() {
                agent.online_since = Some(now);
            }
            agent.agent_api_port = Some(req.ollama_port + 1);
            agent.initializing = true;
            agent.ready_models = Some((0, 0));
            (id, RegisterStatus::Updated, agent.clone())
        } else {
            // 新規ノードを登録
            let node_id = Uuid::new_v4();
            let now = Utc::now();
            let agent = Node {
                id: node_id,
                machine_name: req.machine_name,
                ip_address: req.ip_address,
                ollama_version: req.ollama_version,
                ollama_port: req.ollama_port,
                status: NodeStatus::Online,
                registered_at: now,
                last_seen: now,
                online_since: Some(now),
                custom_name: None,
                tags: Vec::new(),
                notes: None,
                loaded_models: Vec::new(),
                gpu_devices: req.gpu_devices,
                gpu_available: req.gpu_available,
                gpu_count: req.gpu_count,
                gpu_model: req.gpu_model,
                gpu_model_name: None,
                gpu_compute_capability: None,
                gpu_capability_score: None,
                agent_api_port: Some(req.ollama_port + 1),
                initializing: true,
                ready_models: Some((0, 0)),
            };
            nodes.insert(node_id, agent.clone());
            (node_id, RegisterStatus::Registered, agent)
        };

        // ロックを解放してからストレージ保存
        drop(nodes);
        self.save_to_storage(&agent).await?;

        Ok(RegisterResponse {
            node_id,
            status,
            agent_api_port: Some(agent.ollama_port + 1),
            auto_distributed_model: None,
            download_task_id: None,
            agent_token: None,
        })
    }

    /// ノードを取得
    pub async fn get(&self, node_id: Uuid) -> RouterResult<Node> {
        let nodes = self.nodes.read().await;
        nodes
            .get(&node_id)
            .cloned()
            .ok_or(RouterError::AgentNotFound(node_id))
    }

    /// 全ノードを取得
    pub async fn list(&self) -> Vec<Node> {
        let nodes = self.nodes.read().await;
        let mut list: Vec<Node> = nodes.values().cloned().collect();
        list.sort_by(|a, b| a.registered_at.cmp(&b.registered_at));
        list
    }

    /// ノードの最終確認時刻を更新
    #[allow(clippy::too_many_arguments)]
    pub async fn update_last_seen(
        &self,
        node_id: Uuid,
        loaded_models: Option<Vec<String>>,
        gpu_model_name: Option<String>,
        gpu_compute_capability: Option<String>,
        gpu_capability_score: Option<u32>,
        initializing: Option<bool>,
        ready_models: Option<(u8, u8)>,
    ) -> RouterResult<()> {
        let agent_to_save = {
            let mut nodes = self.nodes.write().await;
            let agent = nodes
                .get_mut(&node_id)
                .ok_or(RouterError::AgentNotFound(node_id))?;
            let now = Utc::now();
            let was_online = agent.status == NodeStatus::Online;
            agent.last_seen = now;
            agent.status = NodeStatus::Online;
            if let Some(models) = loaded_models {
                agent.loaded_models = normalize_models(models);
            }
            // GPU能力情報を更新
            if gpu_model_name.is_some() {
                agent.gpu_model_name = gpu_model_name;
            }
            if gpu_compute_capability.is_some() {
                agent.gpu_compute_capability = gpu_compute_capability;
            }
            if gpu_capability_score.is_some() {
                agent.gpu_capability_score = gpu_capability_score;
            }
            if !was_online || agent.online_since.is_none() {
                agent.online_since = Some(now);
            }
            if let Some(init) = initializing {
                agent.initializing = init;
            }
            if ready_models.is_some() {
                agent.ready_models = ready_models;
            }
            agent.clone()
        };

        // ロック解放後にストレージ保存
        self.save_to_storage(&agent_to_save).await?;
        Ok(())
    }

    /// モデルを「インストール済み」としてマーク
    pub async fn mark_model_loaded(&self, node_id: Uuid, model_name: &str) -> RouterResult<()> {
        let normalized = normalize_models(vec![model_name.to_string()]);
        let model = normalized.first().cloned().unwrap_or_default();

        let agent_to_save = {
            let mut nodes = self.nodes.write().await;
            let agent = nodes
                .get_mut(&node_id)
                .ok_or(RouterError::AgentNotFound(node_id))?;
            if !agent.loaded_models.contains(&model) {
                agent.loaded_models.push(model);
                agent.loaded_models.sort();
            }
            agent.clone()
        };

        // 永続化（失敗しても致命ではないがログとして残す）
        if let Err(e) = self.save_to_storage(&agent_to_save).await {
            warn!(
                node_id = %node_id,
                error = %e,
                "Failed to persist loaded_models update"
            );
        }

        Ok(())
    }

    /// ノードをオフラインにする
    pub async fn mark_offline(&self, node_id: Uuid) -> RouterResult<()> {
        let agent_to_save = {
            let mut nodes = self.nodes.write().await;
            let agent = nodes
                .get_mut(&node_id)
                .ok_or(RouterError::AgentNotFound(node_id))?;
            agent.status = NodeStatus::Offline;
            agent.online_since = None;
            agent.clone()
        };

        // ロック解放後にストレージ保存
        self.save_to_storage(&agent_to_save).await?;
        Ok(())
    }
}

/// ノード設定更新用ペイロード
pub struct AgentSettingsUpdate {
    /// カスタム表示名（Noneで未指定, Some(None)でリセット）
    pub custom_name: Option<Option<String>>,
    /// タグ配列
    pub tags: Option<Vec<String>>,
    /// メモ（Noneで未指定, Some(None)でリセット）
    pub notes: Option<Option<String>>,
}

impl NodeRegistry {
    /// ノード設定を更新
    pub async fn update_settings(
        &self,
        node_id: Uuid,
        settings: AgentSettingsUpdate,
    ) -> RouterResult<Node> {
        let updated_agent = {
            let mut nodes = self.nodes.write().await;
            let agent = nodes
                .get_mut(&node_id)
                .ok_or(RouterError::AgentNotFound(node_id))?;

            if let Some(custom_name) = settings.custom_name {
                agent.custom_name = custom_name.and_then(|name| {
                    let trimmed = name.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                });
            }

            if let Some(tags) = settings.tags {
                agent.tags = tags
                    .into_iter()
                    .map(|tag| tag.trim().to_string())
                    .filter(|tag| !tag.is_empty())
                    .collect();
            }

            if let Some(notes) = settings.notes {
                agent.notes = notes.and_then(|note| {
                    let trimmed = note.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                });
            }

            agent.clone()
        };

        self.save_to_storage(&updated_agent).await?;
        Ok(updated_agent)
    }

    /// ノードを削除
    pub async fn delete(&self, node_id: Uuid) -> RouterResult<()> {
        let existed = {
            let mut nodes = self.nodes.write().await;
            nodes.remove(&node_id)
        };

        if existed.is_none() {
            return Err(RouterError::AgentNotFound(node_id));
        }

        if self.storage_enabled {
            crate::db::delete_agent(node_id).await
        } else {
            Ok(())
        }
    }

    /// ノードメトリクスを更新
    ///
    /// ノードから送信されたメトリクス情報（CPU使用率、メモリ使用率、アクティブリクエスト数等）を
    /// メモリ内のHashMapに保存する。ノードが存在しない場合はエラーを返す。
    pub async fn update_metrics(&self, metrics: AgentMetrics) -> RouterResult<()> {
        // ノードが存在するか確認
        {
            let nodes = self.nodes.read().await;
            if !nodes.contains_key(&metrics.node_id) {
                return Err(RouterError::AgentNotFound(metrics.node_id));
            }
        }

        // メトリクスを保存
        let mut metrics_map = self.metrics.write().await;
        metrics_map.insert(metrics.node_id, metrics);

        Ok(())
    }
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn normalize_models(models: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();

    for model in models {
        let trimmed = model.trim();
        if trimmed.is_empty() {
            continue;
        }

        let canonical = trimmed.to_string();
        if seen.insert(canonical.clone()) {
            normalized.push(canonical);
        }
    }

    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use ollama_router_common::types::GpuDeviceInfo;
    use std::net::IpAddr;

    fn sample_gpu_devices() -> Vec<GpuDeviceInfo> {
        vec![GpuDeviceInfo {
            model: "Test GPU".to_string(),
            count: 1,
            memory: None,
        }]
    }

    #[tokio::test]
    async fn test_register_new_agent() {
        let registry = NodeRegistry::new();
        let req = RegisterRequest {
            machine_name: "test-machine".to_string(),
            ip_address: "192.168.1.100".parse::<IpAddr>().unwrap(),
            ollama_version: "0.1.0".to_string(),
            ollama_port: 11434,
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
        };

        let response = registry.register(req).await.unwrap();
        assert_eq!(response.status, RegisterStatus::Registered);

        let agent = registry.get(response.node_id).await.unwrap();
        assert_eq!(agent.machine_name, "test-machine");
        assert_eq!(agent.status, NodeStatus::Online);
        assert!(agent.loaded_models.is_empty());
    }

    #[tokio::test]
    async fn test_register_existing_agent() {
        let registry = NodeRegistry::new();
        let req = RegisterRequest {
            machine_name: "test-machine".to_string(),
            ip_address: "192.168.1.100".parse::<IpAddr>().unwrap(),
            ollama_version: "0.1.0".to_string(),
            ollama_port: 11434,
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
        };

        let first_response = registry.register(req.clone()).await.unwrap();
        assert_eq!(first_response.status, RegisterStatus::Registered);

        let second_response = registry.register(req).await.unwrap();
        assert_eq!(second_response.status, RegisterStatus::Updated);
        assert_eq!(first_response.node_id, second_response.node_id);

        let agent = registry.get(first_response.node_id).await.unwrap();
        assert!(agent.loaded_models.is_empty());
    }

    #[tokio::test]
    async fn test_list_agents() {
        let registry = NodeRegistry::new();

        let req1 = RegisterRequest {
            machine_name: "machine1".to_string(),
            ip_address: "192.168.1.100".parse().unwrap(),
            ollama_version: "0.1.0".to_string(),
            ollama_port: 11434,
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
        };
        registry.register(req1).await.unwrap();

        let req2 = RegisterRequest {
            machine_name: "machine2".to_string(),
            ip_address: "192.168.1.101".parse().unwrap(),
            ollama_version: "0.1.0".to_string(),
            ollama_port: 11434,
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
        };
        registry.register(req2).await.unwrap();

        let nodes = registry.list().await;
        assert_eq!(nodes.len(), 2);
    }

    #[tokio::test]
    async fn test_mark_offline() {
        let registry = NodeRegistry::new();
        let req = RegisterRequest {
            machine_name: "test-machine".to_string(),
            ip_address: "192.168.1.100".parse().unwrap(),
            ollama_version: "0.1.0".to_string(),
            ollama_port: 11434,
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
        };

        let response = registry.register(req).await.unwrap();
        registry.mark_offline(response.node_id).await.unwrap();

        let agent = registry.get(response.node_id).await.unwrap();
        assert_eq!(agent.status, NodeStatus::Offline);
        assert!(agent.loaded_models.is_empty());
    }

    #[tokio::test]
    async fn test_update_settings() {
        let registry = NodeRegistry::new();
        let req = RegisterRequest {
            machine_name: "settings-machine".to_string(),
            ip_address: "192.168.1.150".parse().unwrap(),
            ollama_version: "0.1.0".to_string(),
            ollama_port: 11434,
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
        };

        let node_id = registry.register(req).await.unwrap().node_id;

        let updated = registry
            .update_settings(
                node_id,
                AgentSettingsUpdate {
                    custom_name: Some(Some("Display".into())),
                    tags: Some(vec!["primary".into(), "gpu".into()]),
                    notes: Some(Some("Important".into())),
                },
            )
            .await
            .unwrap();

        assert_eq!(updated.custom_name.as_deref(), Some("Display"));
        assert_eq!(updated.tags, vec!["primary", "gpu"]);
        assert_eq!(updated.notes.as_deref(), Some("Important"));
        assert!(updated.loaded_models.is_empty());
    }

    #[tokio::test]
    async fn test_delete_agent_removes_from_registry() {
        let registry = NodeRegistry::new();
        let node_id = registry
            .register(RegisterRequest {
                machine_name: "delete-me".to_string(),
                ip_address: "127.0.0.1".parse().unwrap(),
                ollama_version: "0.1.0".to_string(),
                ollama_port: 11434,
                gpu_available: true,
                gpu_devices: sample_gpu_devices(),
                gpu_count: Some(1),
                gpu_model: Some("Test GPU".to_string()),
            })
            .await
            .unwrap()
            .node_id;

        registry.delete(node_id).await.unwrap();
        assert!(registry.list().await.is_empty());
    }

    #[tokio::test]
    async fn test_update_last_seen_updates_models() {
        let registry = NodeRegistry::new();
        let node_id = registry
            .register(RegisterRequest {
                machine_name: "models".into(),
                ip_address: "127.0.0.1".parse().unwrap(),
                ollama_version: "0.1.0".into(),
                ollama_port: 11434,
                gpu_available: true,
                gpu_devices: sample_gpu_devices(),
                gpu_count: Some(1),
                gpu_model: Some("Test GPU".to_string()),
            })
            .await
            .unwrap()
            .node_id;

        registry
            .update_last_seen(
                node_id,
                Some(vec![
                    " gpt-oss:20b ".into(),
                    "gpt-oss:20b".into(),
                    "".into(),
                    "phi-3".into(),
                ]),
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        let agent = registry.get(node_id).await.unwrap();
        assert_eq!(agent.loaded_models, vec!["gpt-oss:20b", "phi-3"]);
    }

    #[test]
    fn test_normalize_models_removes_duplicates() {
        let models = vec![
            "a ".into(),
            "b".into(),
            "a".into(),
            " ".into(),
            "".into(),
            "c".into(),
            "b".into(),
        ];
        assert_eq!(
            normalize_models(models),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }
}
