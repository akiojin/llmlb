//! エンドポイントレジストリ
//!
//! エンドポイントの状態をメモリ内で管理し、SQLiteと同期

use crate::db::endpoints as db;
use crate::types::endpoint::{
    Endpoint, EndpointCapability, EndpointModel, EndpointStatus, EndpointType, SupportedAPI,
};
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// 単一エンドポイントが「異常な数」のモデルを申告したと判定する閾値。
/// この値を超えたら `warn!()` ログで運用者に通知する（実態を持たない誤申告の早期検知用）。
const SUSPICIOUS_MODEL_COUNT_THRESHOLD: usize = 50;
use uuid::Uuid;

fn model_lookup_keys(model_id: &str) -> Vec<String> {
    let mut keys = vec![model_id.to_string()];
    if let Some(mapping) = crate::models::mapping::find_mapping(model_id) {
        if !keys.iter().any(|key| key == mapping.canonical) {
            keys.push(mapping.canonical.to_string());
        }
        for alias in mapping.aliases {
            if !keys.iter().any(|key| key == alias.name) {
                keys.push(alias.name.to_string());
            }
        }
    }
    keys
}

fn endpoint_model_lookup_keys(model: &EndpointModel) -> Vec<String> {
    let mut keys = model_lookup_keys(&model.model_id);
    if let Some(canonical) = model.canonical_name.as_deref() {
        for key in model_lookup_keys(canonical) {
            if !keys.iter().any(|existing| existing == &key) {
                keys.push(key);
            }
        }
    }
    keys
}

fn insert_model_mapping(
    model_map: &mut HashMap<String, Vec<Uuid>>,
    model: &EndpointModel,
    endpoint_id: Uuid,
) {
    for key in endpoint_model_lookup_keys(model) {
        let endpoints = model_map.entry(key).or_default();
        if !endpoints.contains(&endpoint_id) {
            endpoints.push(endpoint_id);
        }
    }
}

fn remove_model_mapping(
    model_map: &mut HashMap<String, Vec<Uuid>>,
    model: &EndpointModel,
    endpoint_id: Uuid,
) {
    for key in endpoint_model_lookup_keys(model) {
        let remove_key = if let Some(endpoints) = model_map.get_mut(&key) {
            endpoints.retain(|id| *id != endpoint_id);
            endpoints.is_empty()
        } else {
            false
        };

        if remove_key {
            model_map.remove(&key);
        }
    }
}

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
                insert_model_mapping(&mut model_map, model, endpoint_id);
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

    /// 指定した機能を持つオンラインエンドポイントを取得
    ///
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

    /// 指定した機能を持つオンラインエンドポイントを補助指標用のレイテンシ順で取得
    ///
    /// SPEC-f8e3a1b7: 推論レイテンシ（EMA α=0.2）でソート。
    /// 複数エンドポイントがある場合、レイテンシが低いものを優先する。
    pub async fn list_online_by_capability_sorted(
        &self,
        capability: EndpointCapability,
    ) -> Vec<Endpoint> {
        let mut endpoints = self.list_online_by_capability(capability).await;
        // SPEC-f8e3a1b7: 推論レイテンシ（inference_latency_ms）でソート
        endpoints.sort_by(|a, b| {
            let a_lat = a.get_inference_latency_for_sort();
            let b_lat = b.get_inference_latency_for_sort();
            a_lat
                .partial_cmp(&b_lat)
                .unwrap_or(std::cmp::Ordering::Equal)
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
        let mut seen = HashSet::new();
        let mut resolved = Vec::new();

        for lookup_key in model_lookup_keys(model_id) {
            if let Some(ids) = model_map.get(&lookup_key) {
                for id in ids {
                    if !seen.insert(*id) {
                        continue;
                    }
                    if let Some(endpoint) = endpoints.get(id) {
                        if endpoint.status == EndpointStatus::Online {
                            resolved.push(endpoint.clone());
                        }
                    }
                }
            }
        }

        resolved
    }

    /// モデルIDと対応APIからオンラインエンドポイントを検索
    pub async fn find_by_model_and_supported_api(
        &self,
        model_id: &str,
        required_api: SupportedAPI,
    ) -> Vec<Endpoint> {
        let endpoints = self.find_by_model(model_id).await;
        let requested_keys = model_lookup_keys(model_id);
        let mut resolved = Vec::new();

        for endpoint in endpoints {
            let Ok(models) = db::list_endpoint_models(&self.pool, endpoint.id).await else {
                warn!(
                    endpoint_id = %endpoint.id,
                    model_id = %model_id,
                    required_api = %required_api,
                    "Failed to load endpoint models while filtering by supported API"
                );
                continue;
            };

            let supports_required_api = models.iter().any(|model| {
                model.supported_apis.contains(&required_api)
                    && endpoint_model_lookup_keys(model)
                        .iter()
                        .any(|key| requested_keys.iter().any(|requested| requested == key))
            });

            if supports_required_api {
                resolved.push(endpoint);
            }
        }

        resolved
    }

    /// 補助指標用にレイテンシ順でエンドポイントをソート（低レイテンシ優先）
    ///
    /// SPEC-f8e3a1b7: 推論レイテンシ（EMA α=0.2）を使用してソート。
    /// レイテンシが同じ場合はラウンドロビンでタイブレーク。
    pub async fn find_by_model_sorted_by_latency(&self, model_id: &str) -> Vec<Endpoint> {
        let mut endpoints = self.find_by_model(model_id).await;
        // SPEC-f8e3a1b7: 推論レイテンシ（inference_latency_ms）でソート
        endpoints.sort_by(|a, b| {
            let a_lat = a.get_inference_latency_for_sort();
            let b_lat = b.get_inference_latency_for_sort();
            a_lat
                .partial_cmp(&b_lat)
                .unwrap_or(std::cmp::Ordering::Equal)
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
                if let Some(v) = latency_ms {
                    endpoint.latency_ms = Some(v);
                }
                // DBと同様に、last_error は成功時にクリアし、error_count は status=error のときのみ加算する。
                endpoint.last_error = error.map(String::from);
                endpoint.error_count = if status == EndpointStatus::Error {
                    endpoint.error_count.saturating_add(1)
                } else {
                    0
                };
                endpoint.last_seen = Some(chrono::Utc::now());
            }
        }

        Ok(updated)
    }

    /// エンドポイントのGPU情報を更新（キャッシュのみ、DBには保存しない）
    ///
    /// `/api/health`から取得したGPU情報をキャッシュに反映する。
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

    /// エンドポイントのタイプを更新（DBとキャッシュ両方）（SPEC-e8e9326e）
    ///
    /// ヘルスチェック時のoffline→online遷移時に再検出して更新する。
    pub async fn update_endpoint_type(
        &self,
        id: Uuid,
        endpoint_type: EndpointType,
    ) -> Result<bool, sqlx::Error> {
        // DBを更新
        let updated = db::update_endpoint_type(&self.pool, id, endpoint_type).await?;

        if updated {
            // キャッシュを更新
            let mut endpoints = self.endpoints.write().await;
            if let Some(endpoint) = endpoints.get_mut(&id) {
                endpoint.endpoint_type = endpoint_type;
            }
        }

        Ok(updated)
    }

    /// エンドポイントの推論レイテンシを更新（DBとキャッシュ両方）（SPEC-f8e3a1b7）
    ///
    /// 推論リクエスト完了時に呼び出し、EMA（α=0.2）で平均レイテンシを計算する。
    /// オフライン時は`reset_inference_latency`を呼び出す。
    pub async fn update_inference_latency(
        &self,
        id: Uuid,
        new_latency_ms: f64,
    ) -> Result<bool, sqlx::Error> {
        // キャッシュを更新（EMA計算はEndpoint内で行う）
        let inference_latency_ms = {
            let mut endpoints = self.endpoints.write().await;
            if let Some(endpoint) = endpoints.get_mut(&id) {
                endpoint.update_inference_latency(new_latency_ms);
                endpoint.inference_latency_ms
            } else {
                return Ok(false);
            }
        };

        // DBを更新
        db::update_inference_latency(&self.pool, id, inference_latency_ms).await
    }

    /// エンドポイントの推論レイテンシをリセット（オフライン時）（SPEC-f8e3a1b7）
    ///
    /// エンドポイントがオフラインになったときに呼び出し、レイテンシをINFINITYに設定。
    pub async fn reset_inference_latency(&self, id: Uuid) -> Result<bool, sqlx::Error> {
        // キャッシュを更新
        {
            let mut endpoints = self.endpoints.write().await;
            if let Some(endpoint) = endpoints.get_mut(&id) {
                endpoint.reset_inference_latency();
            }
        }

        // DBを更新（INFINITYを保存）
        db::update_inference_latency(&self.pool, id, Some(f64::INFINITY)).await
    }

    /// エンドポイントのデバイス情報を更新（DBとキャッシュ両方）（SPEC-f8e3a1b7）
    ///
    /// /api/system APIから取得したデバイス情報を保存する。
    pub async fn update_device_info(
        &self,
        id: Uuid,
        device_info: Option<crate::types::endpoint::DeviceInfo>,
    ) -> Result<bool, sqlx::Error> {
        // キャッシュを更新
        {
            let mut endpoints = self.endpoints.write().await;
            if let Some(endpoint) = endpoints.get_mut(&id) {
                endpoint.device_info = device_info.clone();
            } else {
                return Ok(false);
            }
        }

        // DBを更新
        db::update_device_info(&self.pool, id, device_info.as_ref()).await
    }

    /// エンドポイントのリクエストカウンタをインクリメント（DBとキャッシュ両方）
    pub async fn increment_request_counters(
        &self,
        id: Uuid,
        success: bool,
    ) -> Result<bool, sqlx::Error> {
        let updated = db::increment_request_counters(&self.pool, id, success).await?;

        if updated {
            let mut endpoints = self.endpoints.write().await;
            if let Some(endpoint) = endpoints.get_mut(&id) {
                endpoint.total_requests += 1;
                if success {
                    endpoint.successful_requests += 1;
                } else {
                    endpoint.failed_requests += 1;
                }
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
        let mut model_map = self.model_to_endpoints.write().await;
        insert_model_mapping(&mut model_map, model, model.endpoint_id);

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
                insert_model_mapping(&mut model_map, model, endpoint_id);
            }

            // 削除
            for model in &removed {
                remove_model_mapping(&mut model_map, model, endpoint_id);
            }
        }

        debug!(
            endpoint_id = %endpoint_id,
            added = added.len(),
            removed = removed.len(),
            total = models.len(),
            "Synced endpoint models"
        );

        // 単一エンドポイントが過剰なモデル数を申告した場合に警告（誤申告/設定ミスの早期検知）。
        // 例: カタログ集約サーバが実体なしの全モデルを `/v1/models` で返してしまうケース。
        if models.len() > SUSPICIOUS_MODEL_COUNT_THRESHOLD {
            warn!(
                endpoint_id = %endpoint_id,
                model_count = models.len(),
                threshold = SUSPICIOUS_MODEL_COUNT_THRESHOLD,
                "Endpoint reported suspiciously many models. Verify the endpoint is not aggregating models it cannot serve (see CLAUDE.md C-1/C-2 notes)"
            );
        }

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

    /// モデルマッピングを指定エンドポイント分だけ再構築
    pub async fn refresh_model_mappings(&self, endpoint_id: Uuid) -> Result<(), sqlx::Error> {
        let models = db::list_endpoint_models(&self.pool, endpoint_id).await?;

        let mut model_map = self.model_to_endpoints.write().await;

        // 既存マッピングから当該エンドポイントを除外
        model_map.retain(|_, endpoints| {
            endpoints.retain(|id| *id != endpoint_id);
            !endpoints.is_empty()
        });

        // 取得したモデルでマッピングを再構築
        for model in models {
            insert_model_mapping(&mut model_map, &model, endpoint_id);
        }

        Ok(())
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
mod tests;
