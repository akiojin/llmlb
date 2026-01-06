//! ノード管理のストレージ層
//!
//! SQLiteベースでノード情報を永続化（router.dbと統合）

use chrono::{DateTime, Utc};
use llm_router_common::{
    error::{RouterError, RouterResult},
    types::{GpuDeviceInfo, Node, NodeStatus, RuntimeType},
};
use sqlx::SqlitePool;
use std::net::IpAddr;
use uuid::Uuid;

/// ノードストレージ（SQLite版）
#[derive(Clone)]
pub struct NodeStorage {
    pool: SqlitePool,
}

/// SQLiteから読み取るノード行
#[derive(sqlx::FromRow)]
struct NodeRow {
    id: String,
    machine_name: String,
    ip_address: String,
    runtime_version: String,
    runtime_port: i64,
    status: String,
    registered_at: String,
    last_seen: String,
    online_since: Option<String>,
    custom_name: Option<String>,
    notes: Option<String>,
    gpu_available: i64,
    gpu_count: Option<i64>,
    gpu_model: Option<String>,
    gpu_model_name: Option<String>,
    gpu_compute_capability: Option<String>,
    gpu_capability_score: Option<i64>,
    node_api_port: Option<i64>,
    initializing: i64,
    ready_models_current: Option<i64>,
    ready_models_total: Option<i64>,
}

/// SQLiteから読み取るGPUデバイス行
#[derive(sqlx::FromRow)]
struct GpuDeviceRow {
    model: String,
    count: i64,
    memory_bytes: Option<i64>,
}

/// SQLiteから読み取るロード済みモデル行
#[derive(sqlx::FromRow)]
struct LoadedModelRow {
    model_name: String,
    model_type: String,
}

/// SQLiteから読み取るタグ行
#[derive(sqlx::FromRow)]
struct TagRow {
    tag: String,
}

/// SQLiteから読み取るサポートランタイム行
#[derive(sqlx::FromRow)]
struct RuntimeRow {
    runtime_type: String,
}

impl NodeStorage {
    /// 新しいストレージインスタンスを作成
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// ノードを保存（存在する場合は更新）
    pub async fn save_node(&self, node: &Node) -> RouterResult<()> {
        let node_id = node.id.to_string();
        let ip_address = node.ip_address.to_string();
        let status = format!("{:?}", node.status).to_lowercase();
        let registered_at = node.registered_at.to_rfc3339();
        let last_seen = node.last_seen.to_rfc3339();
        let online_since = node.online_since.map(|dt| dt.to_rfc3339());
        let ready_models = node.ready_models.map(|(c, t)| (c as i64, t as i64));

        // トランザクション開始
        let mut tx =
            self.pool.begin().await.map_err(|e| {
                RouterError::Database(format!("Failed to begin transaction: {}", e))
            })?;

        // メインノードをUPSERT
        sqlx::query(
            r#"
            INSERT INTO nodes (
                id, machine_name, ip_address, runtime_version, runtime_port,
                status, registered_at, last_seen, online_since, custom_name, notes,
                gpu_available, gpu_count, gpu_model, gpu_model_name,
                gpu_compute_capability, gpu_capability_score, node_api_port,
                initializing, ready_models_current, ready_models_total
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                machine_name = excluded.machine_name,
                ip_address = excluded.ip_address,
                runtime_version = excluded.runtime_version,
                runtime_port = excluded.runtime_port,
                status = excluded.status,
                last_seen = excluded.last_seen,
                online_since = excluded.online_since,
                custom_name = excluded.custom_name,
                notes = excluded.notes,
                gpu_available = excluded.gpu_available,
                gpu_count = excluded.gpu_count,
                gpu_model = excluded.gpu_model,
                gpu_model_name = excluded.gpu_model_name,
                gpu_compute_capability = excluded.gpu_compute_capability,
                gpu_capability_score = excluded.gpu_capability_score,
                node_api_port = excluded.node_api_port,
                initializing = excluded.initializing,
                ready_models_current = excluded.ready_models_current,
                ready_models_total = excluded.ready_models_total
            "#,
        )
        .bind(&node_id)
        .bind(&node.machine_name)
        .bind(&ip_address)
        .bind(&node.runtime_version)
        .bind(node.runtime_port as i64)
        .bind(&status)
        .bind(&registered_at)
        .bind(&last_seen)
        .bind(&online_since)
        .bind(&node.custom_name)
        .bind(&node.notes)
        .bind(node.gpu_available as i64)
        .bind(node.gpu_count.map(|c| c as i64))
        .bind(&node.gpu_model)
        .bind(&node.gpu_model_name)
        .bind(&node.gpu_compute_capability)
        .bind(node.gpu_capability_score.map(|s| s as i64))
        .bind(node.node_api_port.map(|p| p as i64))
        .bind(node.initializing as i64)
        .bind(ready_models.map(|(c, _)| c))
        .bind(ready_models.map(|(_, t)| t))
        .execute(&mut *tx)
        .await
        .map_err(|e| RouterError::Database(format!("Failed to save node: {}", e)))?;

        // 関連テーブルをクリアして再挿入
        self.clear_and_insert_gpu_devices(&mut tx, &node_id, &node.gpu_devices)
            .await?;
        self.clear_and_insert_loaded_models(&mut tx, &node_id, node)
            .await?;
        self.clear_and_insert_tags(&mut tx, &node_id, &node.tags)
            .await?;
        self.clear_and_insert_runtimes(&mut tx, &node_id, &node.supported_runtimes)
            .await?;

        tx.commit()
            .await
            .map_err(|e| RouterError::Database(format!("Failed to commit transaction: {}", e)))?;

        Ok(())
    }

    /// GPUデバイスをクリアして再挿入
    async fn clear_and_insert_gpu_devices(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        node_id: &str,
        devices: &[GpuDeviceInfo],
    ) -> RouterResult<()> {
        sqlx::query("DELETE FROM node_gpu_devices WHERE node_id = ?")
            .bind(node_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| RouterError::Database(format!("Failed to clear GPU devices: {}", e)))?;

        for device in devices {
            sqlx::query(
                "INSERT INTO node_gpu_devices (node_id, model, count, memory_bytes) VALUES (?, ?, ?, ?)",
            )
            .bind(node_id)
            .bind(&device.model)
            .bind(device.count as i64)
            .bind(device.memory.map(|m| m as i64))
            .execute(&mut **tx)
            .await
            .map_err(|e| RouterError::Database(format!("Failed to insert GPU device: {}", e)))?;
        }

        Ok(())
    }

    /// ロード済みモデルをクリアして再挿入
    async fn clear_and_insert_loaded_models(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        node_id: &str,
        node: &Node,
    ) -> RouterResult<()> {
        sqlx::query("DELETE FROM node_loaded_models WHERE node_id = ?")
            .bind(node_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| RouterError::Database(format!("Failed to clear loaded models: {}", e)))?;

        // LLMモデル
        for model in &node.loaded_models {
            self.insert_loaded_model(tx, node_id, model, "llm").await?;
        }
        // Embeddingモデル
        for model in &node.loaded_embedding_models {
            self.insert_loaded_model(tx, node_id, model, "embedding")
                .await?;
        }
        // ASRモデル
        for model in &node.loaded_asr_models {
            self.insert_loaded_model(tx, node_id, model, "asr").await?;
        }
        // TTSモデル
        for model in &node.loaded_tts_models {
            self.insert_loaded_model(tx, node_id, model, "tts").await?;
        }

        Ok(())
    }

    /// 単一のロード済みモデルを挿入
    async fn insert_loaded_model(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        node_id: &str,
        model_name: &str,
        model_type: &str,
    ) -> RouterResult<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO node_loaded_models (node_id, model_name, model_type) VALUES (?, ?, ?)",
        )
        .bind(node_id)
        .bind(model_name)
        .bind(model_type)
        .execute(&mut **tx)
        .await
        .map_err(|e| RouterError::Database(format!("Failed to insert loaded model: {}", e)))?;

        Ok(())
    }

    /// タグをクリアして再挿入
    async fn clear_and_insert_tags(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        node_id: &str,
        tags: &[String],
    ) -> RouterResult<()> {
        sqlx::query("DELETE FROM node_tags WHERE node_id = ?")
            .bind(node_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| RouterError::Database(format!("Failed to clear tags: {}", e)))?;

        for tag in tags {
            sqlx::query("INSERT INTO node_tags (node_id, tag) VALUES (?, ?)")
                .bind(node_id)
                .bind(tag)
                .execute(&mut **tx)
                .await
                .map_err(|e| RouterError::Database(format!("Failed to insert tag: {}", e)))?;
        }

        Ok(())
    }

    /// サポートランタイムをクリアして再挿入
    async fn clear_and_insert_runtimes(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        node_id: &str,
        runtimes: &[RuntimeType],
    ) -> RouterResult<()> {
        sqlx::query("DELETE FROM node_supported_runtimes WHERE node_id = ?")
            .bind(node_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| RouterError::Database(format!("Failed to clear runtimes: {}", e)))?;

        for runtime in runtimes {
            let runtime_str = format!("{:?}", runtime).to_lowercase();
            sqlx::query(
                "INSERT INTO node_supported_runtimes (node_id, runtime_type) VALUES (?, ?)",
            )
            .bind(node_id)
            .bind(&runtime_str)
            .execute(&mut **tx)
            .await
            .map_err(|e| RouterError::Database(format!("Failed to insert runtime: {}", e)))?;
        }

        Ok(())
    }

    /// すべてのノードを読み込み
    pub async fn load_nodes(&self) -> RouterResult<Vec<Node>> {
        let rows = sqlx::query_as::<_, NodeRow>("SELECT * FROM nodes ORDER BY registered_at DESC")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RouterError::Database(format!("Failed to load nodes: {}", e)))?;

        let mut nodes = Vec::with_capacity(rows.len());
        for row in rows {
            let node = self.row_to_node(row).await?;
            nodes.push(node);
        }

        Ok(nodes)
    }

    /// 特定のノードを読み込み
    pub async fn load_node(&self, node_id: Uuid) -> RouterResult<Option<Node>> {
        let row = sqlx::query_as::<_, NodeRow>("SELECT * FROM nodes WHERE id = ?")
            .bind(node_id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| RouterError::Database(format!("Failed to load node: {}", e)))?;

        match row {
            Some(r) => Ok(Some(self.row_to_node(r).await?)),
            None => Ok(None),
        }
    }

    /// ノードを削除
    pub async fn delete_node(&self, node_id: Uuid) -> RouterResult<()> {
        sqlx::query("DELETE FROM nodes WHERE id = ?")
            .bind(node_id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| RouterError::Database(format!("Failed to delete node: {}", e)))?;

        // 関連テーブルはON DELETE CASCADEで自動削除

        Ok(())
    }

    /// SQLite行をNode構造体に変換
    async fn row_to_node(&self, row: NodeRow) -> RouterResult<Node> {
        let node_id = &row.id;

        // GPUデバイスを取得
        let gpu_devices = self.load_gpu_devices(node_id).await?;

        // ロード済みモデルを取得
        let (loaded_models, loaded_embedding_models, loaded_asr_models, loaded_tts_models) =
            self.load_loaded_models(node_id).await?;

        // タグを取得
        let tags = self.load_tags(node_id).await?;

        // サポートランタイムを取得
        let supported_runtimes = self.load_supported_runtimes(node_id).await?;

        // ready_modelsを復元
        let ready_models = match (row.ready_models_current, row.ready_models_total) {
            (Some(c), Some(t)) => Some((c as u8, t as u8)),
            _ => None,
        };

        Ok(Node {
            id: Uuid::parse_str(&row.id)
                .map_err(|e| RouterError::Database(format!("Invalid UUID: {}", e)))?,
            machine_name: row.machine_name,
            ip_address: row
                .ip_address
                .parse::<IpAddr>()
                .map_err(|e| RouterError::Database(format!("Invalid IP address: {}", e)))?,
            runtime_version: row.runtime_version,
            runtime_port: row.runtime_port as u16,
            status: parse_node_status(&row.status),
            registered_at: DateTime::parse_from_rfc3339(&row.registered_at)
                .map_err(|e| RouterError::Database(format!("Invalid registered_at: {}", e)))?
                .with_timezone(&Utc),
            last_seen: DateTime::parse_from_rfc3339(&row.last_seen)
                .map_err(|e| RouterError::Database(format!("Invalid last_seen: {}", e)))?
                .with_timezone(&Utc),
            online_since: row.online_since.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            }),
            custom_name: row.custom_name,
            tags,
            notes: row.notes,
            loaded_models,
            loaded_embedding_models,
            loaded_asr_models,
            loaded_tts_models,
            supported_runtimes,
            gpu_devices,
            gpu_available: row.gpu_available != 0,
            gpu_count: row.gpu_count.map(|c| c as u32),
            gpu_model: row.gpu_model,
            gpu_model_name: row.gpu_model_name,
            gpu_compute_capability: row.gpu_compute_capability,
            gpu_capability_score: row.gpu_capability_score.map(|s| s as u32),
            node_api_port: row.node_api_port.map(|p| p as u16),
            initializing: row.initializing != 0,
            ready_models,
            sync_state: None,
            sync_progress: None,
            sync_updated_at: None,
            executable_models: Vec::new(),
            excluded_models: Vec::new(),
        })
    }

    /// GPUデバイスを読み込み
    async fn load_gpu_devices(&self, node_id: &str) -> RouterResult<Vec<GpuDeviceInfo>> {
        let rows = sqlx::query_as::<_, GpuDeviceRow>(
            "SELECT model, count, memory_bytes FROM node_gpu_devices WHERE node_id = ?",
        )
        .bind(node_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RouterError::Database(format!("Failed to load GPU devices: {}", e)))?;

        Ok(rows
            .into_iter()
            .map(|r| GpuDeviceInfo {
                model: r.model,
                count: r.count as u32,
                memory: r.memory_bytes.map(|m| m as u64),
            })
            .collect())
    }

    /// ロード済みモデルを読み込み（タイプ別に分類）
    async fn load_loaded_models(
        &self,
        node_id: &str,
    ) -> RouterResult<(Vec<String>, Vec<String>, Vec<String>, Vec<String>)> {
        let rows = sqlx::query_as::<_, LoadedModelRow>(
            "SELECT model_name, model_type FROM node_loaded_models WHERE node_id = ?",
        )
        .bind(node_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RouterError::Database(format!("Failed to load models: {}", e)))?;

        let mut llm = Vec::new();
        let mut embedding = Vec::new();
        let mut asr = Vec::new();
        let mut tts = Vec::new();

        for row in rows {
            match row.model_type.as_str() {
                "llm" => llm.push(row.model_name),
                "embedding" => embedding.push(row.model_name),
                "asr" => asr.push(row.model_name),
                "tts" => tts.push(row.model_name),
                _ => llm.push(row.model_name), // フォールバック
            }
        }

        Ok((llm, embedding, asr, tts))
    }

    /// タグを読み込み
    async fn load_tags(&self, node_id: &str) -> RouterResult<Vec<String>> {
        let rows = sqlx::query_as::<_, TagRow>("SELECT tag FROM node_tags WHERE node_id = ?")
            .bind(node_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RouterError::Database(format!("Failed to load tags: {}", e)))?;

        Ok(rows.into_iter().map(|r| r.tag).collect())
    }

    /// サポートランタイムを読み込み
    async fn load_supported_runtimes(&self, node_id: &str) -> RouterResult<Vec<RuntimeType>> {
        let rows = sqlx::query_as::<_, RuntimeRow>(
            "SELECT runtime_type FROM node_supported_runtimes WHERE node_id = ?",
        )
        .bind(node_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RouterError::Database(format!("Failed to load runtimes: {}", e)))?;

        Ok(rows
            .into_iter()
            .map(|r| parse_runtime_type(&r.runtime_type))
            .collect())
    }
}

/// ノードステータスをパース
fn parse_node_status(s: &str) -> NodeStatus {
    match s.to_lowercase().as_str() {
        "online" => NodeStatus::Online,
        "registering" => NodeStatus::Registering,
        _ => NodeStatus::Offline,
    }
}

/// ランタイムタイプをパース
fn parse_runtime_type(s: &str) -> RuntimeType {
    match s.to_lowercase().as_str() {
        "llamacpp" | "llama_cpp" => RuntimeType::LlamaCpp,
        "nemotroncpp" | "nemotron_cpp" => RuntimeType::NemotronCpp,
        "gptosscpp" | "gptoss_cpp" => RuntimeType::GptOssCpp,
        "whispercpp" | "whisper_cpp" => RuntimeType::WhisperCpp,
        "onnxruntime" | "onnx_runtime" => RuntimeType::OnnxRuntime,
        "stablediffusion" | "stable_diffusion" => RuntimeType::StableDiffusion,
        _ => RuntimeType::LlamaCpp, // デフォルト
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::net::Ipv4Addr;

    async fn create_test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to create test pool");

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        pool
    }

    fn create_test_node() -> Node {
        let now = Utc::now();
        Node {
            id: Uuid::new_v4(),
            machine_name: "test-node".to_string(),
            ip_address: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768,
            status: NodeStatus::Online,
            registered_at: now,
            last_seen: now,
            online_since: Some(now),
            custom_name: Some("My Node".to_string()),
            tags: vec!["primary".to_string(), "gpu".to_string()],
            notes: Some("Test notes".to_string()),
            loaded_models: vec!["llama-3.1-8b".to_string()],
            loaded_embedding_models: vec!["bge-small".to_string()],
            loaded_asr_models: vec!["whisper-large".to_string()],
            loaded_tts_models: vec!["vibevoice".to_string()],
            supported_runtimes: vec![RuntimeType::LlamaCpp, RuntimeType::WhisperCpp],
            gpu_devices: vec![GpuDeviceInfo {
                model: "NVIDIA RTX 4090".to_string(),
                count: 1,
                memory: Some(24_000_000_000),
            }],
            gpu_available: true,
            gpu_count: Some(1),
            gpu_model: Some("NVIDIA RTX 4090".to_string()),
            gpu_model_name: Some("GeForce RTX 4090".to_string()),
            gpu_compute_capability: Some("8.9".to_string()),
            gpu_capability_score: Some(8900),
            node_api_port: Some(32769),
            initializing: false,
            ready_models: Some((4, 4)),
            sync_state: None,
            sync_progress: None,
            sync_updated_at: None,
            executable_models: vec!["llama-3.1-8b".to_string()],
            excluded_models: Vec::new(),
        }
    }

    #[tokio::test]
    async fn test_save_and_load_node() {
        let pool = create_test_pool().await;
        let storage = NodeStorage::new(pool);

        let node = create_test_node();
        storage.save_node(&node).await.unwrap();

        let loaded = storage.load_node(node.id).await.unwrap();
        assert!(loaded.is_some());

        let loaded = loaded.unwrap();
        assert_eq!(loaded.id, node.id);
        assert_eq!(loaded.machine_name, node.machine_name);
        assert_eq!(loaded.status, NodeStatus::Online);
        assert_eq!(loaded.tags.len(), 2);
        assert_eq!(loaded.loaded_models.len(), 1);
        assert_eq!(loaded.gpu_devices.len(), 1);
    }

    #[tokio::test]
    async fn test_load_nodes() {
        let pool = create_test_pool().await;
        let storage = NodeStorage::new(pool);

        let node1 = create_test_node();
        let mut node2 = create_test_node();
        node2.machine_name = "test-node-2".to_string();

        storage.save_node(&node1).await.unwrap();
        storage.save_node(&node2).await.unwrap();

        let nodes = storage.load_nodes().await.unwrap();
        assert_eq!(nodes.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_node() {
        let pool = create_test_pool().await;
        let storage = NodeStorage::new(pool);

        let node = create_test_node();
        storage.save_node(&node).await.unwrap();

        storage.delete_node(node.id).await.unwrap();

        let loaded = storage.load_node(node.id).await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_update_node() {
        let pool = create_test_pool().await;
        let storage = NodeStorage::new(pool);

        let mut node = create_test_node();
        storage.save_node(&node).await.unwrap();

        // ノードを更新
        node.status = NodeStatus::Offline;
        node.loaded_models.push("gpt-4".to_string());
        node.tags.push("updated".to_string());

        storage.save_node(&node).await.unwrap();

        let loaded = storage.load_node(node.id).await.unwrap().unwrap();
        assert_eq!(loaded.status, NodeStatus::Offline);
        assert_eq!(loaded.loaded_models.len(), 2);
        assert_eq!(loaded.tags.len(), 3);
    }
}
