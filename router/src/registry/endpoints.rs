//! エンドポイントレジストリ
//!
//! エンドポイントの状態をメモリ内で管理し、SQLiteと同期

use crate::db::endpoints as db;
use crate::types::endpoint::{Endpoint, EndpointCapability, EndpointModel, EndpointStatus};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;

/// エンドポイントレジストリ
///
/// エンドポイント情報をメモリにキャッシュし、高速な参照を提供する。
/// 変更はDBと同期される。
#[derive(Clone)]
pub struct EndpointRegistry {
    /// エンドポイントのインメモリキャッシュ
    endpoints: Arc<RwLock<HashMap<Uuid, Endpoint>>>,
    /// モデル→エンドポイントIDのマッピング
    model_to_endpoints: Arc<RwLock<HashMap<String, Vec<Uuid>>>>,
    /// データベースプール
    pool: SqlitePool,
}

impl EndpointRegistry {
    /// SQLiteプールからレジストリを作成し、DBからデータを読み込む
    pub async fn new(pool: SqlitePool) -> Result<Self, sqlx::Error> {
        let registry = Self {
            endpoints: Arc::new(RwLock::new(HashMap::new())),
            model_to_endpoints: Arc::new(RwLock::new(HashMap::new())),
            pool,
        };

        // DBからエンドポイントを読み込み
        registry.load_from_db().await?;

        Ok(registry)
    }

    /// DBからエンドポイントとモデルマッピングを読み込み
    async fn load_from_db(&self) -> Result<(), sqlx::Error> {
        let loaded_endpoints = db::list_endpoints(&self.pool).await?;

        let mut endpoints = self.endpoints.write().await;
        let mut model_map = self.model_to_endpoints.write().await;

        for endpoint in loaded_endpoints {
            let endpoint_id = endpoint.id;

            // モデル一覧を取得
            let models = db::list_endpoint_models(&self.pool, endpoint_id).await?;

            // モデルマッピングを更新
            for model in &models {
                model_map
                    .entry(model.model_id.clone())
                    .or_default()
                    .push(endpoint_id);
            }

            endpoints.insert(endpoint_id, endpoint);
        }

        info!(
            endpoint_count = endpoints.len(),
            model_mappings = model_map.len(),
            "Loaded endpoints from database"
        );

        Ok(())
    }

    /// エンドポイントを取得
    pub async fn get(&self, id: Uuid) -> Option<Endpoint> {
        self.endpoints.read().await.get(&id).cloned()
    }

    /// すべてのエンドポイントを取得
    pub async fn list(&self) -> Vec<Endpoint> {
        self.endpoints.read().await.values().cloned().collect()
    }

    /// オンラインのエンドポイントのみを取得
    pub async fn list_online(&self) -> Vec<Endpoint> {
        self.endpoints
            .read()
            .await
            .values()
            .filter(|e| e.status == EndpointStatus::Online)
            .cloned()
            .collect()
    }

    /// 特定ステータスのエンドポイントを取得
    pub async fn list_by_status(&self, status: EndpointStatus) -> Vec<Endpoint> {
        self.endpoints
            .read()
            .await
            .values()
            .filter(|e| e.status == status)
            .cloned()
            .collect()
    }

    /// 指定した機能を持つオンラインエンドポイントを取得（SPEC-66555000移行用）
    ///
    /// NodeRegistryのRuntimeTypeベースのフィルタリングを置き換える。
    /// 例: ImageGeneration機能を持つエンドポイント → 画像生成リクエストの転送先
    pub async fn list_online_by_capability(&self, capability: EndpointCapability) -> Vec<Endpoint> {
        self.endpoints
            .read()
            .await
            .values()
            .filter(|e| e.status == EndpointStatus::Online && e.has_capability(capability))
            .cloned()
            .collect()
    }

    /// 指定した機能を持つオンラインエンドポイントをレイテンシ順で取得
    ///
    /// 複数エンドポイントがある場合、レイテンシが低いものを優先する。
    pub async fn list_online_by_capability_sorted(
        &self,
        capability: EndpointCapability,
    ) -> Vec<Endpoint> {
        let mut endpoints = self.list_online_by_capability(capability).await;
        endpoints.sort_by(|a, b| match (a.latency_ms, b.latency_ms) {
            (Some(a_lat), Some(b_lat)) => a_lat.cmp(&b_lat),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });
        endpoints
    }

    /// 指定した機能を持つオンラインエンドポイントが存在するか確認
    pub async fn has_capability_online(&self, capability: EndpointCapability) -> bool {
        self.endpoints
            .read()
            .await
            .values()
            .any(|e| e.status == EndpointStatus::Online && e.has_capability(capability))
    }

    /// モデルIDからエンドポイントを検索
    pub async fn find_by_model(&self, model_id: &str) -> Vec<Endpoint> {
        let model_map = self.model_to_endpoints.read().await;
        let endpoints = self.endpoints.read().await;

        model_map
            .get(model_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| endpoints.get(id))
                    .filter(|e| e.status == EndpointStatus::Online)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// レイテンシ順でエンドポイントをソート（低レイテンシ優先）
    pub async fn find_by_model_sorted_by_latency(&self, model_id: &str) -> Vec<Endpoint> {
        let mut endpoints = self.find_by_model(model_id).await;
        endpoints.sort_by(|a, b| {
            match (a.latency_ms, b.latency_ms) {
                (Some(a_lat), Some(b_lat)) => a_lat.cmp(&b_lat),
                (Some(_), None) => std::cmp::Ordering::Less, // レイテンシありを優先
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        });
        endpoints
    }

    /// エンドポイントを追加（DBとキャッシュ両方に保存）
    pub async fn add(&self, endpoint: Endpoint) -> Result<(), sqlx::Error> {
        // DBに保存
        db::create_endpoint(&self.pool, &endpoint).await?;

        // キャッシュに追加
        self.endpoints.write().await.insert(endpoint.id, endpoint);

        Ok(())
    }

    /// エンドポイントをキャッシュのみに追加（DBは更新しない）
    ///
    /// 外部でDB保存が完了した後にキャッシュを同期するために使用する。
    pub async fn add_to_cache(&self, endpoint: Endpoint) {
        self.endpoints.write().await.insert(endpoint.id, endpoint);
    }

    /// エンドポイントを更新（DBとキャッシュ両方）
    pub async fn update(&self, endpoint: Endpoint) -> Result<bool, sqlx::Error> {
        // DBを更新
        let updated = db::update_endpoint(&self.pool, &endpoint).await?;

        if updated {
            // キャッシュを更新
            self.endpoints.write().await.insert(endpoint.id, endpoint);
        }

        Ok(updated)
    }

    /// エンドポイントのステータスを更新
    pub async fn update_status(
        &self,
        id: Uuid,
        status: EndpointStatus,
        latency_ms: Option<u32>,
        error: Option<&str>,
    ) -> Result<bool, sqlx::Error> {
        // DBを更新
        let updated = db::update_endpoint_status(&self.pool, id, status, latency_ms, error).await?;

        if updated {
            // キャッシュを更新
            let mut endpoints = self.endpoints.write().await;
            if let Some(endpoint) = endpoints.get_mut(&id) {
                endpoint.status = status;
                endpoint.latency_ms = latency_ms;
                if error.is_some() {
                    endpoint.last_error = error.map(String::from);
                    endpoint.error_count += 1;
                } else {
                    endpoint.error_count = 0;
                }
                endpoint.last_seen = Some(chrono::Utc::now());
            }
        }

        Ok(updated)
    }

    /// エンドポイントのGPU情報を更新（キャッシュのみ、DBには保存しない）
    ///
    /// `/v0/health`から取得したGPU情報をキャッシュに反映する。
    /// GPU情報は頻繁に変化するため、DBには保存せずメモリ上でのみ管理する。
    pub async fn update_gpu_info(
        &self,
        id: Uuid,
        gpu_device_count: Option<u32>,
        gpu_total_memory_bytes: Option<u64>,
        gpu_used_memory_bytes: Option<u64>,
        gpu_capability_score: Option<f32>,
        active_requests: Option<u32>,
    ) -> bool {
        let mut endpoints = self.endpoints.write().await;
        if let Some(endpoint) = endpoints.get_mut(&id) {
            endpoint.gpu_device_count = gpu_device_count;
            endpoint.gpu_total_memory_bytes = gpu_total_memory_bytes;
            endpoint.gpu_used_memory_bytes = gpu_used_memory_bytes;
            endpoint.gpu_capability_score = gpu_capability_score;
            endpoint.active_requests = active_requests;
            true
        } else {
            false
        }
    }

    /// エンドポイントのResponses API対応フラグを更新（DBとキャッシュ両方）
    /// （SPEC-24157000: Open Responses API対応）
    pub async fn update_responses_api_support(
        &self,
        id: Uuid,
        supports_responses_api: bool,
    ) -> Result<bool, sqlx::Error> {
        // DBを更新
        let updated =
            db::update_endpoint_responses_api_support(&self.pool, id, supports_responses_api)
                .await?;

        if updated {
            // キャッシュを更新
            let mut endpoints = self.endpoints.write().await;
            if let Some(endpoint) = endpoints.get_mut(&id) {
                endpoint.supports_responses_api = supports_responses_api;
            }
        }

        Ok(updated)
    }

    /// エンドポイントを削除（DBとキャッシュ両方）
    pub async fn remove(&self, id: Uuid) -> Result<bool, sqlx::Error> {
        // モデルマッピングから削除
        {
            let mut model_map = self.model_to_endpoints.write().await;
            for endpoints in model_map.values_mut() {
                endpoints.retain(|eid| *eid != id);
            }
            // 空になったエントリを削除
            model_map.retain(|_, v| !v.is_empty());
        }

        // DBから削除
        let deleted = db::delete_endpoint(&self.pool, id).await?;

        if deleted {
            // キャッシュから削除
            self.endpoints.write().await.remove(&id);
        }

        Ok(deleted)
    }

    /// モデルを追加
    pub async fn add_model(&self, model: &EndpointModel) -> Result<(), sqlx::Error> {
        // DBに保存
        db::add_endpoint_model(&self.pool, model).await?;

        // モデルマッピングを更新
        self.model_to_endpoints
            .write()
            .await
            .entry(model.model_id.clone())
            .or_default()
            .push(model.endpoint_id);

        Ok(())
    }

    /// エンドポイントのモデルを同期（追加/削除）
    pub async fn sync_models(
        &self,
        endpoint_id: Uuid,
        models: Vec<EndpointModel>,
    ) -> Result<SyncResult, sqlx::Error> {
        // 既存モデルを取得
        let existing = db::list_endpoint_models(&self.pool, endpoint_id).await?;
        let existing_ids: std::collections::HashSet<_> =
            existing.iter().map(|m| &m.model_id).collect();

        let new_ids: std::collections::HashSet<_> = models.iter().map(|m| &m.model_id).collect();

        // 追加されたモデル
        let added: Vec<_> = models
            .iter()
            .filter(|m| !existing_ids.contains(&m.model_id))
            .cloned()
            .collect();

        // 削除されたモデル
        let removed: Vec<_> = existing
            .iter()
            .filter(|m| !new_ids.contains(&m.model_id))
            .cloned()
            .collect();

        // DBを更新
        for model in &added {
            db::add_endpoint_model(&self.pool, model).await?;
        }

        for model in &removed {
            db::delete_endpoint_model(&self.pool, endpoint_id, &model.model_id).await?;
        }

        // モデルマッピングを更新
        {
            let mut model_map = self.model_to_endpoints.write().await;

            // 追加
            for model in &added {
                model_map
                    .entry(model.model_id.clone())
                    .or_default()
                    .push(endpoint_id);
            }

            // 削除
            for model in &removed {
                if let Some(endpoints) = model_map.get_mut(&model.model_id) {
                    endpoints.retain(|id| *id != endpoint_id);
                    if endpoints.is_empty() {
                        model_map.remove(&model.model_id);
                    }
                }
            }
        }

        debug!(
            endpoint_id = %endpoint_id,
            added = added.len(),
            removed = removed.len(),
            total = models.len(),
            "Synced endpoint models"
        );

        Ok(SyncResult {
            added: added.len(),
            removed: removed.len(),
            total: models.len(),
        })
    }

    /// エンドポイントのモデル一覧を取得
    pub async fn list_models(&self, endpoint_id: Uuid) -> Result<Vec<EndpointModel>, sqlx::Error> {
        db::list_endpoint_models(&self.pool, endpoint_id).await
    }

    /// 全モデルIDの一覧を取得
    pub async fn list_all_model_ids(&self) -> Vec<String> {
        self.model_to_endpoints
            .read()
            .await
            .keys()
            .cloned()
            .collect()
    }

    /// キャッシュをDBから再読み込み
    pub async fn reload(&self) -> Result<(), sqlx::Error> {
        // キャッシュをクリア
        {
            self.endpoints.write().await.clear();
            self.model_to_endpoints.write().await.clear();
        }

        // DBから再読み込み
        self.load_from_db().await
    }

    /// エンドポイント数を取得
    pub async fn count(&self) -> usize {
        self.endpoints.read().await.len()
    }

    /// DBプールへの参照を取得
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

/// モデル同期結果
#[derive(Debug, Clone)]
pub struct SyncResult {
    /// 追加されたモデル数
    pub added: usize,
    /// 削除されたモデル数
    pub removed: usize,
    /// 同期後のモデル総数
    pub total: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_utils::TEST_LOCK;
    use crate::types::endpoint::{EndpointCapability, SupportedAPI};

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to create test database");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");
        pool
    }

    #[tokio::test]
    async fn test_registry_basic_operations() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;
        let registry = EndpointRegistry::new(pool).await.unwrap();

        // エンドポイントを追加
        let endpoint = Endpoint::new(
            "Test Endpoint".to_string(),
            "http://localhost:11434".to_string(),
        );
        let endpoint_id = endpoint.id;

        registry.add(endpoint).await.unwrap();

        // 取得確認
        let retrieved = registry.get(endpoint_id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Test Endpoint");

        // 一覧確認
        let all = registry.list().await;
        assert_eq!(all.len(), 1);

        // 削除
        let deleted = registry.remove(endpoint_id).await.unwrap();
        assert!(deleted);

        // 削除後は取得できない
        assert!(registry.get(endpoint_id).await.is_none());
    }

    #[tokio::test]
    async fn test_registry_model_mapping() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;
        let registry = EndpointRegistry::new(pool).await.unwrap();

        // エンドポイントを追加
        let mut endpoint = Endpoint::new("Test".to_string(), "http://localhost:11434".to_string());
        endpoint.status = EndpointStatus::Online;
        let endpoint_id = endpoint.id;

        registry.add(endpoint).await.unwrap();

        // モデルを追加
        let model = EndpointModel {
            endpoint_id,
            model_id: "llama3:8b".to_string(),
            capabilities: Some(vec!["chat".to_string()]),
            last_checked: Some(chrono::Utc::now()),
            supported_apis: vec![SupportedAPI::ChatCompletions],
        };

        registry.add_model(&model).await.unwrap();

        // モデルIDで検索
        let found = registry.find_by_model("llama3:8b").await;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, endpoint_id);

        // 存在しないモデル
        let not_found = registry.find_by_model("nonexistent").await;
        assert!(not_found.is_empty());
    }

    #[tokio::test]
    async fn test_registry_status_update() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;
        let registry = EndpointRegistry::new(pool).await.unwrap();

        // エンドポイントを追加
        let endpoint = Endpoint::new("Test".to_string(), "http://localhost:11434".to_string());
        let endpoint_id = endpoint.id;

        registry.add(endpoint).await.unwrap();

        // ステータスを更新
        registry
            .update_status(endpoint_id, EndpointStatus::Online, Some(50), None)
            .await
            .unwrap();

        // 確認
        let updated = registry.get(endpoint_id).await.unwrap();
        assert_eq!(updated.status, EndpointStatus::Online);
        assert_eq!(updated.latency_ms, Some(50));

        // オンラインエンドポイントのみ取得
        let online = registry.list_online().await;
        assert_eq!(online.len(), 1);
    }

    #[tokio::test]
    async fn test_registry_capability_filter() {
        let _lock = TEST_LOCK.lock().await;
        let pool = setup_test_db().await;
        let registry = EndpointRegistry::new(pool).await.unwrap();

        // チャット機能のみのエンドポイント
        let mut ep_chat = Endpoint::new(
            "Chat Only".to_string(),
            "http://localhost:11434".to_string(),
        );
        ep_chat.status = EndpointStatus::Online;
        ep_chat.capabilities = vec![EndpointCapability::ChatCompletion];
        registry.add(ep_chat).await.unwrap();

        // 画像生成機能を持つエンドポイント
        let mut ep_image =
            Endpoint::new("Image Gen".to_string(), "http://localhost:7860".to_string());
        ep_image.status = EndpointStatus::Online;
        ep_image.capabilities = vec![
            EndpointCapability::ChatCompletion,
            EndpointCapability::ImageGeneration,
        ];
        ep_image.latency_ms = Some(100);
        registry.add(ep_image).await.unwrap();

        // 音声認識機能を持つエンドポイント（オフライン）
        let mut ep_audio = Endpoint::new("ASR".to_string(), "http://localhost:8080".to_string());
        ep_audio.status = EndpointStatus::Offline;
        ep_audio.capabilities = vec![EndpointCapability::AudioTranscription];
        registry.add(ep_audio).await.unwrap();

        // チャット機能で検索 → 2件
        let chat_endpoints = registry
            .list_online_by_capability(EndpointCapability::ChatCompletion)
            .await;
        assert_eq!(chat_endpoints.len(), 2);

        // 画像生成機能で検索 → 1件
        let image_endpoints = registry
            .list_online_by_capability(EndpointCapability::ImageGeneration)
            .await;
        assert_eq!(image_endpoints.len(), 1);
        assert_eq!(image_endpoints[0].name, "Image Gen");

        // 音声認識機能で検索 → 0件（オフラインなので）
        let audio_endpoints = registry
            .list_online_by_capability(EndpointCapability::AudioTranscription)
            .await;
        assert!(audio_endpoints.is_empty());

        // 機能存在チェック
        assert!(
            registry
                .has_capability_online(EndpointCapability::ImageGeneration)
                .await
        );
        assert!(
            !registry
                .has_capability_online(EndpointCapability::AudioTranscription)
                .await
        );
        assert!(
            !registry
                .has_capability_online(EndpointCapability::AudioSpeech)
                .await
        );
    }
}
