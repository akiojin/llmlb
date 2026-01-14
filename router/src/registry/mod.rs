//! ノード登録管理
//!
//! ノードの状態をメモリ内で管理し、SQLiteと同期

pub mod models;

use crate::db::nodes::NodeStorage;
use chrono::Utc;
use llm_router_common::{
    error::{RouterError, RouterResult},
    protocol::{RegisterRequest, RegisterResponse, RegisterStatus},
    types::{GpuDeviceInfo, Node, NodeStatus, SyncProgress, SyncState},
};
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use uuid::Uuid;

/// ノードレジストリ
#[derive(Clone)]
pub struct NodeRegistry {
    nodes: Arc<RwLock<HashMap<Uuid, Node>>>,
    storage: Option<NodeStorage>,
}

impl NodeRegistry {
    /// 新しいレジストリを作成（ストレージなし、テスト用）
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            storage: None,
        }
    }

    /// SQLiteストレージ付きでレジストリを作成
    pub async fn with_storage(pool: SqlitePool) -> RouterResult<Self> {
        let storage = NodeStorage::new(pool);

        let registry = Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            storage: Some(storage),
        };

        // 起動時にストレージからノード情報を読み込み
        registry.load_from_storage().await?;

        Ok(registry)
    }

    /// ストレージからノード情報を読み込み
    async fn load_from_storage(&self) -> RouterResult<()> {
        let storage = match &self.storage {
            Some(s) => s,
            None => return Ok(()),
        };

        let loaded_nodes = storage.load_nodes().await?;
        let mut nodes = self.nodes.write().await;

        let mut removed_count = 0;
        let mut removed_ids = Vec::new();
        let mut sanitized_nodes = Vec::new();

        for mut node in loaded_nodes {
            // GPU非搭載 or 情報欠落ノードは削除対象
            if !node.gpu_available {
                info!(
                    node_id = %node.id,
                    machine_name = %node.machine_name,
                    reason = "gpu_available is false",
                    "Removing GPU-less node from database during startup cleanup"
                );
                removed_count += 1;
                removed_ids.push(node.id);
                continue;
            }

            let mut sanitized = false;

            if node.gpu_devices.is_empty() {
                if let Some(model) = node.gpu_model.clone() {
                    let count = node.gpu_count.unwrap_or(1).max(1);
                    node.gpu_devices = vec![GpuDeviceInfo {
                        model,
                        count,
                        memory: None,
                    }];
                    sanitized = true;
                } else {
                    info!(
                        node_id = %node.id,
                        machine_name = %node.machine_name,
                        reason = "gpu_devices array is empty and gpu_model is None",
                        "Removing node with missing GPU device information from database"
                    );
                    removed_count += 1;
                    removed_ids.push(node.id);
                    continue;
                }
            }

            if !node.gpu_devices.iter().all(|device| device.is_valid()) {
                info!(
                    node_id = %node.id,
                    machine_name = %node.machine_name,
                    reason = "gpu_devices contains invalid device (empty model or zero count)",
                    "Removing node with invalid GPU device information from database"
                );
                removed_count += 1;
                removed_ids.push(node.id);
                continue;
            }

            if sanitized {
                sanitized_nodes.push(node.clone());
            }

            nodes.insert(node.id, node);
        }

        info!(
            nodes_loaded = nodes.len(),
            nodes_removed = removed_count,
            "Completed node registry initialization from storage"
        );

        drop(nodes);

        // 削除対象ノードをデータベースから削除
        for id in removed_ids {
            if let Err(err) = storage.delete_node(id).await {
                error!(
                    node_id = %id,
                    error = %err,
                    "Failed to delete GPU-less node from database during cleanup"
                );
            }
        }

        // サニタイズされたノード情報をストレージに保存
        for node in sanitized_nodes {
            if let Err(err) = self.save_to_storage(&node).await {
                warn!(
                    node_id = %node.id,
                    machine_name = %node.machine_name,
                    error = %err,
                    "Failed to persist sanitized node data to storage"
                );
            }
        }

        Ok(())
    }

    /// ノードをストレージに保存
    async fn save_to_storage(&self, node: &Node) -> RouterResult<()> {
        match &self.storage {
            Some(storage) => storage.save_node(node).await,
            None => Ok(()),
        }
    }

    /// ノードを登録
    pub async fn register(&self, req: RegisterRequest) -> RouterResult<RegisterResponse> {
        let mut nodes = self.nodes.write().await;

        // 同じIPアドレス+ポートのノードが既に存在するか確認
        // （同じ端末でも異なるポートで動作するノード/Ollamaは別々に登録可能）
        let existing = nodes
            .values()
            .find(|n| n.ip_address == req.ip_address && n.runtime_port == req.runtime_port)
            .map(|n| n.id);

        let (node_id, status, node) = if let Some(id) = existing {
            // 既存ノードを更新
            let node = nodes.get_mut(&id).unwrap();
            let now = Utc::now();
            node.machine_name = req.machine_name.clone();
            node.ip_address = req.ip_address;
            node.runtime_version = req.runtime_version.clone();
            node.runtime_port = req.runtime_port;
            node.gpu_available = req.gpu_available;
            node.gpu_devices = req.gpu_devices.clone();
            node.gpu_count = req.gpu_count;
            node.gpu_model = req.gpu_model.clone();
            node.supported_runtimes = req.supported_runtimes.clone();
            // 再登録時は Pending に戻す（承認後に Registering/Online に遷移）
            node.status = NodeStatus::Pending;
            node.last_seen = now;
            // online_since はモデル同期完了（Online遷移）時に設定
            node.online_since = None;
            node.node_api_port = Some(req.runtime_port + 1);
            node.initializing = true;
            node.ready_models = Some((0, 0));
            node.sync_state = None;
            node.sync_progress = None;
            node.sync_updated_at = None;
            node.executable_models.clear();
            node.excluded_models.clear();
            (id, RegisterStatus::Updated, node.clone())
        } else {
            // 新規ノードを登録
            let node_id = Uuid::new_v4();
            let now = Utc::now();
            let node = Node {
                id: node_id,
                machine_name: req.machine_name,
                ip_address: req.ip_address,
                runtime_version: req.runtime_version,
                runtime_port: req.runtime_port,
                status: NodeStatus::Pending,
                registered_at: now,
                last_seen: now,
                // online_since はモデル同期完了（Online遷移）時に設定
                online_since: None,
                custom_name: None,
                tags: Vec::new(),
                notes: None,
                loaded_models: Vec::new(),
                loaded_embedding_models: Vec::new(),
                loaded_asr_models: Vec::new(),
                loaded_tts_models: Vec::new(),
                executable_models: Vec::new(),
                excluded_models: HashSet::new(),
                supported_runtimes: req.supported_runtimes,
                gpu_devices: req.gpu_devices,
                gpu_available: req.gpu_available,
                gpu_count: req.gpu_count,
                gpu_model: req.gpu_model,
                gpu_model_name: None,
                gpu_compute_capability: None,
                gpu_capability_score: None,
                node_api_port: Some(req.runtime_port + 1),
                initializing: true,
                ready_models: Some((0, 0)),
                sync_state: None,
                sync_progress: None,
                sync_updated_at: None,
            };
            nodes.insert(node_id, node.clone());
            (node_id, RegisterStatus::Registered, node)
        };

        // ロックを解放してからストレージ保存
        drop(nodes);
        self.save_to_storage(&node).await?;

        Ok(RegisterResponse {
            node_id,
            status,
            node_api_port: Some(node.runtime_port + 1),
            node_token: None,
        })
    }

    /// ノードを取得
    pub async fn get(&self, node_id: Uuid) -> RouterResult<Node> {
        let nodes = self.nodes.read().await;
        nodes
            .get(&node_id)
            .cloned()
            .ok_or(RouterError::NodeNotFound(node_id))
    }

    /// 全ノードを取得
    pub async fn list(&self) -> Vec<Node> {
        let nodes = self.nodes.read().await;
        let mut list: Vec<Node> = nodes.values().cloned().collect();
        list.sort_by(|a, b| a.registered_at.cmp(&b.registered_at));
        list
    }

    /// SPEC-93536000: 指定モデルを実行可能なノード一覧を取得
    /// executable_modelsに含まれ、excluded_modelsに含まれないノードを返す
    pub async fn get_nodes_for_model(&self, model_id: &str) -> Vec<Node> {
        let nodes = self.nodes.read().await;
        nodes
            .values()
            .filter(|n| {
                // Onlineノードのみ対象
                n.status == NodeStatus::Online
                    // executable_modelsに含まれている
                    && n.executable_models.iter().any(|m| m == model_id)
                    // excluded_modelsに含まれていない
                    && !n.excluded_models.iter().any(|m| m == model_id)
            })
            .cloned()
            .collect()
    }

    /// SPEC-93536000: 指定モデルがいずれかのオンラインノードのexecutable_modelsに存在するかチェック
    /// excluded_modelsは考慮しない（モデルの「存在」のみを確認）
    pub async fn model_exists_in_any_node(&self, model_id: &str) -> bool {
        let nodes = self.nodes.read().await;
        nodes.values().any(|n| {
            n.status == NodeStatus::Online && n.executable_models.iter().any(|m| m == model_id)
        })
    }

    /// ノードの最終確認時刻を更新
    #[allow(clippy::too_many_arguments)]
    pub async fn update_last_seen(
        &self,
        node_id: Uuid,
        loaded_models: Option<Vec<String>>,
        loaded_embedding_models: Option<Vec<String>>,
        gpu_model_name: Option<String>,
        gpu_compute_capability: Option<String>,
        gpu_capability_score: Option<u32>,
        initializing: Option<bool>,
        ready_models: Option<(u8, u8)>,
        sync_state: Option<SyncState>,
        sync_progress: Option<SyncProgress>,
        executable_models: Option<Vec<String>>,
    ) -> RouterResult<()> {
        let node_to_save = {
            let mut nodes = self.nodes.write().await;
            let node = nodes
                .get_mut(&node_id)
                .ok_or(RouterError::NodeNotFound(node_id))?;
            let now = Utc::now();
            node.last_seen = now;

            if let Some(models) = loaded_models {
                node.loaded_models = normalize_models(models);
            }
            if let Some(embedding_models) = loaded_embedding_models {
                node.loaded_embedding_models = normalize_models(embedding_models);
            }
            // SPEC-93536000: executable_modelsを更新
            if let Some(models) = executable_models {
                node.executable_models = normalize_models(models);
            }
            // GPU能力情報を更新
            if gpu_model_name.is_some() {
                node.gpu_model_name = gpu_model_name;
            }
            if gpu_compute_capability.is_some() {
                node.gpu_compute_capability = gpu_compute_capability;
            }
            if gpu_capability_score.is_some() {
                node.gpu_capability_score = gpu_capability_score;
            }
            if let Some(init) = initializing {
                node.initializing = init;
            }
            if ready_models.is_some() {
                node.ready_models = ready_models;
            }
            if sync_state.is_some() || sync_progress.is_some() {
                node.sync_state = sync_state;
                node.sync_progress = sync_progress;
                node.sync_updated_at = Some(now);
            }

            // 状態遷移ロジック
            let current_ready = ready_models.or(node.ready_models);
            match node.status {
                NodeStatus::Pending => {
                    // 承認待ちのため状態遷移しない
                }
                NodeStatus::Registering => {
                    // モデル同期完了したらOnlineに遷移
                    if let Some((ready, total)) = current_ready {
                        if ready >= total {
                            node.status = NodeStatus::Online;
                            node.initializing = false;
                            node.online_since = Some(now);
                        }
                    }
                }
                NodeStatus::Online => {
                    // 既にOnlineならそのまま維持
                }
                NodeStatus::Offline => {
                    // Offlineからの復帰はRegisteringへ
                    node.status = NodeStatus::Registering;
                }
            }
            node.clone()
        };

        // ロック解放後にストレージ保存
        self.save_to_storage(&node_to_save).await?;
        Ok(())
    }

    /// ノードが実行可能なモデル一覧を更新（再登録時のリセット含む）
    pub async fn update_executable_models(
        &self,
        node_id: Uuid,
        models: Vec<String>,
    ) -> RouterResult<()> {
        let normalized = normalize_models(models);
        let mut nodes = self.nodes.write().await;
        let node = nodes
            .get_mut(&node_id)
            .ok_or(RouterError::NodeNotFound(node_id))?;
        node.executable_models = normalized;
        node.excluded_models.clear();
        Ok(())
    }

    /// 指定モデルを報告済みのノードが存在するか（オンライン/オフライン問わず）
    pub async fn has_model_reported(&self, model_id: &str) -> bool {
        let nodes = self.nodes.read().await;
        nodes
            .values()
            .any(|node| node.executable_models.iter().any(|m| m == model_id))
    }

    /// オンラインノードの実行可能モデル一覧を取得（除外モデルは除く）
    pub async fn list_executable_models_online(&self) -> HashSet<String> {
        let nodes = self.nodes.read().await;
        let mut models = HashSet::new();
        for node in nodes.values() {
            if node.status != NodeStatus::Online {
                continue;
            }
            for model in &node.executable_models {
                if node.excluded_models.contains(model) {
                    continue;
                }
                models.insert(model.clone());
            }
        }
        models
    }

    /// モデルを「インストール済み」としてマーク
    pub async fn mark_model_loaded(&self, node_id: Uuid, model_name: &str) -> RouterResult<()> {
        let normalized = normalize_models(vec![model_name.to_string()]);
        let model = normalized.first().cloned().unwrap_or_default();

        let node_to_save = {
            let mut nodes = self.nodes.write().await;
            let node = nodes
                .get_mut(&node_id)
                .ok_or(RouterError::NodeNotFound(node_id))?;
            if !node.loaded_models.contains(&model) {
                node.loaded_models.push(model);
                node.loaded_models.sort();
            }
            node.clone()
        };

        // 永続化（失敗しても致命ではないがログとして残す）
        if let Err(e) = self.save_to_storage(&node_to_save).await {
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
        let node_to_save = {
            let mut nodes = self.nodes.write().await;
            let node = nodes
                .get_mut(&node_id)
                .ok_or(RouterError::NodeNotFound(node_id))?;
            node.status = NodeStatus::Offline;
            node.online_since = None;
            node.excluded_models.clear();
            node.clone()
        };

        // ロック解放後にストレージ保存
        self.save_to_storage(&node_to_save).await?;
        Ok(())
    }

    /// SPEC-93536000: ノードから特定モデルを除外
    /// 推論失敗などで一時的にモデルを無効化する場合に使用
    pub async fn exclude_model_from_node(&self, node_id: Uuid, model_id: &str) -> RouterResult<()> {
        let node_to_save = {
            let mut nodes = self.nodes.write().await;
            let node = nodes
                .get_mut(&node_id)
                .ok_or(RouterError::NodeNotFound(node_id))?;
            // HashSetなので重複は自動的に回避される
            node.excluded_models.insert(model_id.to_string());
            node.clone()
        };

        // 永続化（失敗しても致命ではないがログとして残す）
        if let Err(e) = self.save_to_storage(&node_to_save).await {
            warn!(
                node_id = %node_id,
                model_id = %model_id,
                error = %e,
                "Failed to persist excluded_models update"
            );
        }

        Ok(())
    }

    /// ノードを承認
    pub async fn approve(&self, node_id: Uuid) -> RouterResult<Node> {
        let node_to_save = {
            let mut nodes = self.nodes.write().await;
            let node = nodes
                .get_mut(&node_id)
                .ok_or(RouterError::NodeNotFound(node_id))?;

            if node.status != NodeStatus::Pending {
                return Err(RouterError::Common(
                    llm_router_common::error::CommonError::Validation(
                        "Node is not pending".to_string(),
                    ),
                ));
            }

            let now = Utc::now();
            let ready = node
                .ready_models
                .map(|(ready, total)| ready >= total)
                .unwrap_or(false);

            if ready {
                node.status = NodeStatus::Online;
                node.initializing = false;
                node.online_since = Some(now);
            } else {
                node.status = NodeStatus::Registering;
                node.initializing = true;
                node.online_since = None;
            }

            node.clone()
        };

        self.save_to_storage(&node_to_save).await?;
        Ok(node_to_save)
    }
}

/// ノード設定更新用ペイロード
pub struct NodeSettingsUpdate {
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
        settings: NodeSettingsUpdate,
    ) -> RouterResult<Node> {
        let updated_node = {
            let mut nodes = self.nodes.write().await;
            let node = nodes
                .get_mut(&node_id)
                .ok_or(RouterError::NodeNotFound(node_id))?;

            if let Some(custom_name) = settings.custom_name {
                node.custom_name = custom_name.and_then(|name| {
                    let trimmed = name.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                });
            }

            if let Some(tags) = settings.tags {
                node.tags = tags
                    .into_iter()
                    .map(|tag| tag.trim().to_string())
                    .filter(|tag| !tag.is_empty())
                    .collect();
            }

            if let Some(notes) = settings.notes {
                node.notes = notes.and_then(|note| {
                    let trimmed = note.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                });
            }

            node.clone()
        };

        self.save_to_storage(&updated_node).await?;
        Ok(updated_node)
    }

    /// ノードを削除
    pub async fn delete(&self, node_id: Uuid) -> RouterResult<()> {
        let existed = {
            let mut nodes = self.nodes.write().await;
            nodes.remove(&node_id)
        };

        if existed.is_none() {
            return Err(RouterError::NodeNotFound(node_id));
        }

        match &self.storage {
            Some(storage) => storage.delete_node(node_id).await,
            None => Ok(()),
        }
    }

    /// テスト用: ノードをOnline状態にマークする
    #[cfg(test)]
    pub async fn mark_online(&self, node_id: Uuid) -> RouterResult<()> {
        let mut nodes = self.nodes.write().await;
        let node = nodes
            .get_mut(&node_id)
            .ok_or(RouterError::NodeNotFound(node_id))?;
        node.status = llm_router_common::types::NodeStatus::Online;
        node.initializing = false;
        node.online_since = Some(Utc::now());
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
    use llm_router_common::types::GpuDeviceInfo;
    use std::net::IpAddr;

    fn sample_gpu_devices() -> Vec<GpuDeviceInfo> {
        vec![GpuDeviceInfo {
            model: "Test GPU".to_string(),
            count: 1,
            memory: None,
        }]
    }

    #[tokio::test]
    async fn test_register_new_node() {
        let registry = NodeRegistry::new();
        let req = RegisterRequest {
            machine_name: "test-machine".to_string(),
            ip_address: "192.168.1.100".parse::<IpAddr>().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        };

        let response = registry.register(req).await.unwrap();
        assert_eq!(response.status, RegisterStatus::Registered);

        let node = registry.get(response.node_id).await.unwrap();
        assert_eq!(node.machine_name, "test-machine");
        // 新規登録時は Pending 状態（承認後に Registering/Online に遷移）
        assert_eq!(node.status, NodeStatus::Pending);
        assert!(node.loaded_models.is_empty());
    }

    #[tokio::test]
    async fn test_register_existing_node() {
        let registry = NodeRegistry::new();
        let req = RegisterRequest {
            machine_name: "test-machine".to_string(),
            ip_address: "192.168.1.100".parse::<IpAddr>().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        };

        let first_response = registry.register(req.clone()).await.unwrap();
        assert_eq!(first_response.status, RegisterStatus::Registered);

        let second_response = registry.register(req).await.unwrap();
        assert_eq!(second_response.status, RegisterStatus::Updated);
        assert_eq!(first_response.node_id, second_response.node_id);

        let node = registry.get(first_response.node_id).await.unwrap();
        assert!(node.loaded_models.is_empty());
    }

    #[tokio::test]
    async fn test_get_nodes_for_model_filters_excluded() {
        let registry = NodeRegistry::new();
        let req = RegisterRequest {
            machine_name: "model-node".to_string(),
            ip_address: "192.168.1.120".parse::<IpAddr>().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        };

        let response = registry.register(req).await.unwrap();
        registry.mark_online(response.node_id).await.unwrap();
        registry
            .update_executable_models(
                response.node_id,
                vec!["model-a".to_string(), "model-b".to_string()],
            )
            .await
            .unwrap();

        let nodes = registry.get_nodes_for_model("model-a").await;
        assert_eq!(nodes.len(), 1);

        registry
            .exclude_model_from_node(response.node_id, "model-a")
            .await
            .unwrap();

        let nodes = registry.get_nodes_for_model("model-a").await;
        assert!(nodes.is_empty());
    }

    #[tokio::test]
    async fn test_update_executable_models_clears_excluded() {
        let registry = NodeRegistry::new();
        let req = RegisterRequest {
            machine_name: "reset-node".to_string(),
            ip_address: "192.168.1.121".parse::<IpAddr>().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        };

        let response = registry.register(req).await.unwrap();
        registry.mark_online(response.node_id).await.unwrap();
        registry
            .update_executable_models(response.node_id, vec!["model-x".to_string()])
            .await
            .unwrap();
        registry
            .exclude_model_from_node(response.node_id, "model-x")
            .await
            .unwrap();

        registry
            .update_executable_models(response.node_id, vec!["model-x".to_string()])
            .await
            .unwrap();

        let nodes = registry.get_nodes_for_model("model-x").await;
        assert_eq!(nodes.len(), 1);
    }

    #[tokio::test]
    async fn test_approve_pending_to_online_when_ready() {
        let registry = NodeRegistry::new();
        let req = RegisterRequest {
            machine_name: "approve-ready".to_string(),
            ip_address: "192.168.1.100".parse::<IpAddr>().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        };

        let response = registry.register(req).await.unwrap();
        // pending 中でもメトリクス更新は行われるが、状態は遷移しない
        registry
            .update_last_seen(
                response.node_id,
                Some(vec!["gpt-oss-20b".to_string()]),
                None,
                None,
                None,
                None,
                Some(false),
                Some((1, 1)),
                None,
                None,
                None, // executable_models
            )
            .await
            .unwrap();

        let pending_node = registry.get(response.node_id).await.unwrap();
        assert_eq!(pending_node.status, NodeStatus::Pending);

        let approved = registry.approve(response.node_id).await.unwrap();
        assert_eq!(approved.status, NodeStatus::Online);
        assert!(approved.online_since.is_some());
    }

    #[tokio::test]
    async fn test_approve_pending_to_registering_when_not_ready() {
        let registry = NodeRegistry::new();
        let req = RegisterRequest {
            machine_name: "approve-not-ready".to_string(),
            ip_address: "192.168.1.101".parse::<IpAddr>().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        };

        let response = registry.register(req).await.unwrap();
        registry
            .update_last_seen(
                response.node_id,
                None,
                None,
                None,
                None,
                None,
                Some(true),
                Some((0, 1)),
                None,
                None,
                None, // executable_models
            )
            .await
            .unwrap();

        let approved = registry.approve(response.node_id).await.unwrap();
        assert_eq!(approved.status, NodeStatus::Registering);
        assert!(approved.online_since.is_none());
    }

    #[tokio::test]
    async fn test_list_nodes() {
        let registry = NodeRegistry::new();

        let req1 = RegisterRequest {
            machine_name: "machine1".to_string(),
            ip_address: "192.168.1.100".parse().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        };
        registry.register(req1).await.unwrap();

        let req2 = RegisterRequest {
            machine_name: "machine2".to_string(),
            ip_address: "192.168.1.101".parse().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
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
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        };

        let response = registry.register(req).await.unwrap();
        registry.mark_offline(response.node_id).await.unwrap();

        let node = registry.get(response.node_id).await.unwrap();
        assert_eq!(node.status, NodeStatus::Offline);
        assert!(node.loaded_models.is_empty());
    }

    #[tokio::test]
    async fn test_update_settings() {
        let registry = NodeRegistry::new();
        let req = RegisterRequest {
            machine_name: "settings-machine".to_string(),
            ip_address: "192.168.1.150".parse().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        };

        let node_id = registry.register(req).await.unwrap().node_id;

        let updated = registry
            .update_settings(
                node_id,
                NodeSettingsUpdate {
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
    async fn test_delete_node_removes_from_registry() {
        let registry = NodeRegistry::new();
        let node_id = registry
            .register(RegisterRequest {
                machine_name: "delete-me".to_string(),
                ip_address: "127.0.0.1".parse().unwrap(),
                runtime_version: "0.1.0".to_string(),
                runtime_port: 32768,
                gpu_available: true,
                gpu_devices: sample_gpu_devices(),
                gpu_count: Some(1),
                gpu_model: Some("Test GPU".to_string()),
                supported_runtimes: Vec::new(),
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
                runtime_version: "0.1.0".into(),
                runtime_port: 32768,
                gpu_available: true,
                gpu_devices: sample_gpu_devices(),
                gpu_count: Some(1),
                gpu_model: Some("Test GPU".to_string()),
                supported_runtimes: Vec::new(),
            })
            .await
            .unwrap()
            .node_id;

        registry
            .update_last_seen(
                node_id,
                Some(vec![
                    " gpt-oss-20b ".into(),
                    "gpt-oss-20b".into(),
                    "".into(),
                    "phi-3".into(),
                ]),
                None, // loaded_embedding_models
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None, // executable_models
            )
            .await
            .unwrap();

        let node = registry.get(node_id).await.unwrap();
        assert_eq!(node.loaded_models, vec!["gpt-oss-20b", "phi-3"]);
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

    // SPEC-93536000: get_nodes_for_model() Unit Tests (6.1)

    #[tokio::test]
    async fn test_get_nodes_for_model_returns_online_nodes_with_model() {
        let registry = NodeRegistry::new();

        // ノード1を登録してOnlineにする
        let node1_id = registry
            .register(RegisterRequest {
                machine_name: "node1".into(),
                ip_address: "127.0.0.1".parse().unwrap(),
                runtime_version: "0.1.0".into(),
                runtime_port: 32768,
                gpu_available: true,
                gpu_devices: sample_gpu_devices(),
                gpu_count: Some(1),
                gpu_model: Some("Test GPU".into()),
                supported_runtimes: Vec::new(),
            })
            .await
            .unwrap()
            .node_id;

        // executable_modelsを設定
        registry
            .update_last_seen(
                node1_id,
                None,
                None,
                None,
                None,
                None,
                Some(false),
                Some((1, 1)),
                None,
                None,
                Some(vec!["llama-3.1-8b".into(), "mistral-7b".into()]),
            )
            .await
            .unwrap();

        // Onlineに遷移
        registry.approve(node1_id).await.unwrap();

        // モデルでフィルタ
        let nodes = registry.get_nodes_for_model("llama-3.1-8b").await;
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, node1_id);

        // 存在しないモデル
        let nodes = registry.get_nodes_for_model("nonexistent-model").await;
        assert!(nodes.is_empty());
    }

    #[tokio::test]
    async fn test_get_nodes_for_model_excludes_offline_nodes() {
        let registry = NodeRegistry::new();

        let node_id = registry
            .register(RegisterRequest {
                machine_name: "offline-node".into(),
                ip_address: "127.0.0.2".parse().unwrap(),
                runtime_version: "0.1.0".into(),
                runtime_port: 32768,
                gpu_available: true,
                gpu_devices: sample_gpu_devices(),
                gpu_count: Some(1),
                gpu_model: Some("Test GPU".into()),
                supported_runtimes: Vec::new(),
            })
            .await
            .unwrap()
            .node_id;

        // executable_modelsを設定（まだPending状態）
        registry
            .update_last_seen(
                node_id,
                None,
                None,
                None,
                None,
                None,
                Some(false),
                Some((1, 1)),
                None,
                None,
                Some(vec!["llama-3.1-8b".into()]),
            )
            .await
            .unwrap();

        // Pending状態のノードは取得されない
        let nodes = registry.get_nodes_for_model("llama-3.1-8b").await;
        assert!(nodes.is_empty());

        // Onlineにしてから取得
        registry.approve(node_id).await.unwrap();
        let nodes = registry.get_nodes_for_model("llama-3.1-8b").await;
        assert_eq!(nodes.len(), 1);

        // Offlineにマーク
        registry.mark_offline(node_id).await.unwrap();
        let nodes = registry.get_nodes_for_model("llama-3.1-8b").await;
        assert!(nodes.is_empty());
    }

    #[tokio::test]
    async fn test_get_nodes_for_model_excludes_excluded_models() {
        let registry = NodeRegistry::new();

        let node_id = registry
            .register(RegisterRequest {
                machine_name: "exclude-test".into(),
                ip_address: "127.0.0.3".parse().unwrap(),
                runtime_version: "0.1.0".into(),
                runtime_port: 32768,
                gpu_available: true,
                gpu_devices: sample_gpu_devices(),
                gpu_count: Some(1),
                gpu_model: Some("Test GPU".into()),
                supported_runtimes: Vec::new(),
            })
            .await
            .unwrap()
            .node_id;

        // executable_modelsを設定
        registry
            .update_last_seen(
                node_id,
                None,
                None,
                None,
                None,
                None,
                Some(false),
                Some((1, 1)),
                None,
                None,
                Some(vec!["llama-3.1-8b".into(), "mistral-7b".into()]),
            )
            .await
            .unwrap();

        registry.approve(node_id).await.unwrap();

        // 両方のモデルが取得可能
        assert_eq!(registry.get_nodes_for_model("llama-3.1-8b").await.len(), 1);
        assert_eq!(registry.get_nodes_for_model("mistral-7b").await.len(), 1);

        // llama-3.1-8bを除外
        registry
            .exclude_model_from_node(node_id, "llama-3.1-8b")
            .await
            .unwrap();

        // llama-3.1-8bは取得されなくなる
        assert!(registry
            .get_nodes_for_model("llama-3.1-8b")
            .await
            .is_empty());
        // mistral-7bは取得可能
        assert_eq!(registry.get_nodes_for_model("mistral-7b").await.len(), 1);
    }

    // SPEC-93536000: exclude_model_from_node() Unit Tests (6.2)

    #[tokio::test]
    async fn test_exclude_model_from_node_adds_to_excluded_list() {
        let registry = NodeRegistry::new();

        let node_id = registry
            .register(RegisterRequest {
                machine_name: "exclude-add".into(),
                ip_address: "127.0.0.4".parse().unwrap(),
                runtime_version: "0.1.0".into(),
                runtime_port: 32768,
                gpu_available: true,
                gpu_devices: sample_gpu_devices(),
                gpu_count: Some(1),
                gpu_model: Some("Test GPU".into()),
                supported_runtimes: Vec::new(),
            })
            .await
            .unwrap()
            .node_id;

        let node = registry.get(node_id).await.unwrap();
        assert!(node.excluded_models.is_empty());

        registry
            .exclude_model_from_node(node_id, "failing-model")
            .await
            .unwrap();

        let node = registry.get(node_id).await.unwrap();
        assert!(node.excluded_models.contains("failing-model"));
        assert_eq!(node.excluded_models.len(), 1);
    }

    #[tokio::test]
    async fn test_exclude_model_from_node_prevents_duplicate() {
        let registry = NodeRegistry::new();

        let node_id = registry
            .register(RegisterRequest {
                machine_name: "exclude-dup".into(),
                ip_address: "127.0.0.5".parse().unwrap(),
                runtime_version: "0.1.0".into(),
                runtime_port: 32768,
                gpu_available: true,
                gpu_devices: sample_gpu_devices(),
                gpu_count: Some(1),
                gpu_model: Some("Test GPU".into()),
                supported_runtimes: Vec::new(),
            })
            .await
            .unwrap()
            .node_id;

        // 同じモデルを2回除外
        registry
            .exclude_model_from_node(node_id, "dup-model")
            .await
            .unwrap();
        registry
            .exclude_model_from_node(node_id, "dup-model")
            .await
            .unwrap();

        let node = registry.get(node_id).await.unwrap();
        // 重複は追加されない（HashSetなので自動的に重複回避）
        assert_eq!(node.excluded_models.len(), 1);
        assert!(node.excluded_models.contains("dup-model"));
    }

    #[tokio::test]
    async fn test_exclude_model_from_node_returns_error_for_nonexistent_node() {
        let registry = NodeRegistry::new();
        let fake_id = Uuid::new_v4();

        let result = registry
            .exclude_model_from_node(fake_id, "some-model")
            .await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RouterError::NodeNotFound(_)));
    }

    #[tokio::test]
    async fn test_model_exists_in_any_node() {
        let registry = NodeRegistry::new();

        let node_id = registry
            .register(RegisterRequest {
                machine_name: "exists-test".into(),
                ip_address: "127.0.0.6".parse().unwrap(),
                runtime_version: "0.1.0".into(),
                runtime_port: 32768,
                gpu_available: true,
                gpu_devices: sample_gpu_devices(),
                gpu_count: Some(1),
                gpu_model: Some("Test GPU".into()),
                supported_runtimes: Vec::new(),
            })
            .await
            .unwrap()
            .node_id;

        // executable_modelsを設定
        registry
            .update_last_seen(
                node_id,
                None,
                None,
                None,
                None,
                None,
                Some(false),
                Some((1, 1)),
                None,
                None,
                Some(vec!["gpt-4".into()]),
            )
            .await
            .unwrap();

        // Pending状態では存在しないと判定
        assert!(!registry.model_exists_in_any_node("gpt-4").await);

        // Onlineに遷移
        registry.approve(node_id).await.unwrap();

        // 存在確認
        assert!(registry.model_exists_in_any_node("gpt-4").await);
        assert!(!registry.model_exists_in_any_node("nonexistent").await);

        // excluded_modelsには影響されない（存在確認のみ）
        registry
            .exclude_model_from_node(node_id, "gpt-4")
            .await
            .unwrap();
        assert!(registry.model_exists_in_any_node("gpt-4").await);
    }

    /// T009: Offline状態のノードがハートビートで復帰するとRegistering状態になることを検証
    #[tokio::test]
    async fn test_offline_node_returns_to_registering_on_heartbeat() {
        let registry = NodeRegistry::new();

        // ノードを登録
        let node_id = registry
            .register(RegisterRequest {
                machine_name: "offline-test-node".into(),
                ip_address: "127.0.0.7".parse().unwrap(),
                runtime_version: "0.1.0".into(),
                runtime_port: 32768,
                gpu_available: true,
                gpu_devices: sample_gpu_devices(),
                gpu_count: Some(1),
                gpu_model: Some("Test GPU".into()),
                supported_runtimes: Vec::new(),
            })
            .await
            .unwrap()
            .node_id;

        // 登録時は ready_models = (0, 0) で承認すると即 Online になるため、
        // 承認前に ready_models を (0, 1) に設定してモデル同期中の状態をシミュレート
        registry
            .update_last_seen(
                node_id,
                None,
                None,
                None,
                None,
                None,
                Some(true),
                Some((0, 1)), // 0/1 = まだ同期中
                None,
                None,
                None,
            )
            .await
            .unwrap();

        // 承認してRegistering状態に遷移（ready < total なので Online にはならない）
        registry.approve(node_id).await.unwrap();
        let node = registry.get(node_id).await.unwrap();
        assert_eq!(node.status, NodeStatus::Registering);

        // ready_modelsでモデル同期完了を通知してOnlineに遷移
        registry
            .update_last_seen(
                node_id,
                None,
                None,
                None,
                None,
                None,
                Some(false),
                Some((1, 1)), // 1/1 = 同期完了
                None,
                None,
                None,
            )
            .await
            .unwrap();
        let node = registry.get(node_id).await.unwrap();
        assert_eq!(node.status, NodeStatus::Online);

        // ノードをオフラインに設定
        registry.mark_offline(node_id).await.unwrap();
        let node = registry.get(node_id).await.unwrap();
        assert_eq!(node.status, NodeStatus::Offline);

        // ハートビート（update_last_seen）を受信
        // Offline状態からの復帰はRegisteringに遷移すべき（直接Onlineではない）
        registry
            .update_last_seen(
                node_id, None, None, None, None, None, None, None, None, None, None,
            )
            .await
            .unwrap();
        let node = registry.get(node_id).await.unwrap();
        assert_eq!(
            node.status,
            NodeStatus::Registering,
            "Offline状態からの復帰はRegistering状態であるべき（直接Onlineではない）"
        );
    }

    /// 同じIP+ポートで異なるmachine_nameでも既存ノードとして更新されることを検証
    #[tokio::test]
    async fn test_register_same_ip_port_different_machine_name_updates() {
        let registry = NodeRegistry::new();

        let req1 = RegisterRequest {
            machine_name: "machine-a".to_string(),
            ip_address: "192.168.1.100".parse::<IpAddr>().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        };

        let first_response = registry.register(req1).await.unwrap();
        assert_eq!(first_response.status, RegisterStatus::Registered);

        // 同じIP+ポートだが異なるmachine_nameで登録
        let req2 = RegisterRequest {
            machine_name: "machine-b".to_string(), // 異なるmachine_name
            ip_address: "192.168.1.100".parse::<IpAddr>().unwrap(),
            runtime_version: "0.2.0".to_string(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        };

        let second_response = registry.register(req2).await.unwrap();
        // 同じIP+ポートなのでUpdatedになる（重複登録防止）
        assert_eq!(second_response.status, RegisterStatus::Updated);
        assert_eq!(first_response.node_id, second_response.node_id);

        // machine_nameも更新される
        let node = registry.get(first_response.node_id).await.unwrap();
        assert_eq!(node.machine_name, "machine-b");
        assert_eq!(node.runtime_version, "0.2.0");
    }

    /// 同じIPで異なるポートは別ノードとして登録されることを検証
    #[tokio::test]
    async fn test_register_same_ip_different_port_creates_multiple_nodes() {
        let registry = NodeRegistry::new();

        let req1 = RegisterRequest {
            machine_name: "shared-machine".to_string(),
            ip_address: "192.168.1.200".parse::<IpAddr>().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        };
        let res1 = registry.register(req1).await.unwrap();
        assert_eq!(res1.status, RegisterStatus::Registered);

        // 同じIPだが異なるポート（例：Ollama）
        let req2 = RegisterRequest {
            machine_name: "shared-machine-ollama".to_string(),
            ip_address: "192.168.1.200".parse::<IpAddr>().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 11434, // Ollamaのデフォルトポート
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        };
        let res2 = registry.register(req2).await.unwrap();
        // 異なるポートなので新規登録
        assert_eq!(res2.status, RegisterStatus::Registered);
        assert_ne!(res1.node_id, res2.node_id);

        // 2つのノードが存在する
        let nodes = registry.list().await;
        assert_eq!(nodes.len(), 2);
    }

    /// 異なるIPで同じポートは別ノードとして登録されることを検証
    #[tokio::test]
    async fn test_register_different_ip_same_port_creates_multiple_nodes() {
        let registry = NodeRegistry::new();

        let req1 = RegisterRequest {
            machine_name: "machine-1".to_string(),
            ip_address: "192.168.1.100".parse::<IpAddr>().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        };
        let res1 = registry.register(req1).await.unwrap();
        assert_eq!(res1.status, RegisterStatus::Registered);

        // 異なるIPで同じポート
        let req2 = RegisterRequest {
            machine_name: "machine-2".to_string(),
            ip_address: "192.168.1.101".parse::<IpAddr>().unwrap(), // 異なるIP
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768, // 同じポート
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        };
        let res2 = registry.register(req2).await.unwrap();
        // 異なるIPなので新規登録
        assert_eq!(res2.status, RegisterStatus::Registered);
        assert_ne!(res1.node_id, res2.node_id);

        // 2つのノードが存在する
        let nodes = registry.list().await;
        assert_eq!(nodes.len(), 2);
    }
}
