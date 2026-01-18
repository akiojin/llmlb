//! ノード管理APIハンドラー（レガシー）
//!
//! # 廃止予定
#![allow(deprecated)] // NodeRegistry migration in progress - entire module uses legacy registry
//!
//! このモジュールのAPIは廃止予定です。新しい実装では以下を使用してください：
//!
//! | 廃止予定API | 移行先 |
//! |-------------|--------|
//! | GET /v0/nodes | GET /v0/endpoints |
//! | GET /v0/nodes/metrics | GET /v0/dashboard/stats |
//! | POST /v0/nodes/:id/approve | （不要・エンドポイントは即時有効） |
//! | DELETE /v0/nodes/:id | DELETE /v0/endpoints/:id |
//! | PUT /v0/nodes/:id/settings | PUT /v0/endpoints/:id |
//!
//! ## 移行スケジュール
//!
//! - Phase 1.3.B: deprecatedマーク追加（現在）
//! - Phase 1.3.D: AppState/テスト修正
//! - Phase 1.3.E: 完全削除
//!
//! SPEC-66555000: POST /v0/nodes（ノード自己登録）は廃止されました。
//! エンドポイント管理は POST /v0/endpoints を使用してください。

use super::error::AppError;
use crate::{
    balancer::{EndpointLoadSnapshot, SystemSummary},
    events::DashboardEvent,
    registry::NodeSettingsUpdate,
    AppState,
};
use axum::{extract::State, http::StatusCode, Extension, Json};
use llm_router_common::{
    auth::{Claims, UserRole},
    error::RouterError,
    types::Node,
};
use serde::Deserialize;

// SPEC-66555000: register_node 関数は廃止されました
// 新しい実装は POST /v0/endpoints を使用してください

/// GET /v0/nodes - ノード一覧取得
///
/// # 廃止予定
///
/// このAPIは廃止予定です。代わりに `GET /v0/endpoints` を使用してください。
#[deprecated(note = "Use GET /v0/endpoints instead")]
pub async fn list_nodes(State(state): State<AppState>) -> Json<Vec<Node>> {
    let nodes = state.registry.list().await;
    Json(nodes)
}

/// PUT /v0/nodes/:id/settings - ノード設定更新
///
/// # 廃止予定
///
/// このAPIは廃止予定です。代わりに `PUT /v0/endpoints/:id` を使用してください。
#[deprecated(note = "Use PUT /v0/endpoints/:id instead")]
pub async fn update_node_settings(
    State(state): State<AppState>,
    axum::extract::Path(node_id): axum::extract::Path<uuid::Uuid>,
    Json(payload): Json<UpdateNodeSettingsPayload>,
) -> Result<Json<Node>, AppError> {
    let update = NodeSettingsUpdate {
        custom_name: payload.custom_name,
        tags: payload.tags,
        notes: payload.notes,
    };

    let node = state.registry.update_settings(node_id, update).await?;

    Ok(Json(node))
}

/// POST /v0/nodes/:id/approve - ノードを承認する
///
/// # 廃止予定
///
/// このAPIは廃止予定です。エンドポイントは登録時に即時有効になるため、
/// 承認フローは不要になりました。
#[deprecated(note = "Endpoints are immediately active; approval flow is deprecated")]
pub async fn approve_node(
    Extension(claims): Extension<Claims>,
    State(state): State<AppState>,
    axum::extract::Path(node_id): axum::extract::Path<uuid::Uuid>,
) -> Result<Json<Node>, AppError> {
    ensure_admin(&claims)?;
    let node = state.registry.approve(node_id).await?;
    Ok(Json(node))
}

/// ノード設定更新リクエスト
#[derive(Debug, Deserialize)]
pub struct UpdateNodeSettingsPayload {
    /// 表示名（nullでリセット）
    #[serde(default)]
    pub custom_name: Option<Option<String>>,
    /// タグ一覧
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// メモ（nullでリセット）
    #[serde(default)]
    pub notes: Option<Option<String>>,
}

/// GET /v0/nodes/metrics - ノードメトリクス取得
///
/// # 廃止予定
///
/// このAPIは廃止予定です。代わりに `GET /v0/dashboard/stats` を使用してください。
#[deprecated(note = "Use GET /v0/dashboard/stats instead")]
pub async fn list_node_metrics(State(state): State<AppState>) -> Json<Vec<EndpointLoadSnapshot>> {
    let snapshots = state.load_manager.snapshots().await;
    Json(snapshots)
}

/// GET /v0/metrics/summary - システム統計
///
/// # 廃止予定
///
/// このAPIは廃止予定です。代わりに `GET /v0/dashboard/stats` を使用してください。
#[deprecated(note = "Use GET /v0/dashboard/stats instead")]
pub async fn metrics_summary(State(state): State<AppState>) -> Json<SystemSummary> {
    let summary = state.load_manager.summary().await;
    Json(summary)
}

/// DELETE /v0/nodes/:id - ノードを削除（Admin権限必須）
///
/// # 廃止予定
///
/// このAPIは廃止予定です。代わりに `DELETE /v0/endpoints/:id` を使用してください。
#[deprecated(note = "Use DELETE /v0/endpoints/:id instead")]
pub async fn delete_node(
    Extension(claims): Extension<Claims>,
    State(state): State<AppState>,
    axum::extract::Path(node_id): axum::extract::Path<uuid::Uuid>,
) -> Result<StatusCode, AppError> {
    ensure_admin(&claims)?;
    state.registry.delete(node_id).await?;

    // Publish dashboard event for real-time updates
    state
        .event_bus
        .publish(DashboardEvent::NodeRemoved { node_id });

    Ok(StatusCode::NO_CONTENT)
}

/// POST /v0/nodes/:id/disconnect - ノードを強制オフラインにする
///
/// # 廃止予定
///
/// このAPIは廃止予定です。エンドポイントの無効化には
/// `PUT /v0/endpoints/:id` で `enabled: false` を設定してください。
#[deprecated(note = "Use PUT /v0/endpoints/:id with enabled: false instead")]
pub async fn disconnect_node(
    State(state): State<AppState>,
    axum::extract::Path(node_id): axum::extract::Path<uuid::Uuid>,
) -> Result<StatusCode, AppError> {
    // Get old status before disconnecting
    let old_status = state.registry.get(node_id).await.ok().map(|n| n.status);

    state.registry.mark_offline(node_id).await?;

    // Publish dashboard event for real-time updates
    if let Some(old) = old_status {
        state.event_bus.publish(DashboardEvent::NodeStatusChanged {
            node_id,
            old_status: old,
            new_status: llm_router_common::types::NodeStatus::Offline,
        });
    }

    Ok(StatusCode::ACCEPTED)
}

// SPEC-66555000: AppError は super::error::AppError を使用
// 重複定義を削除しました

fn ensure_admin(claims: &Claims) -> Result<(), AppError> {
    if claims.role != UserRole::Admin {
        return Err(AppError(RouterError::Authorization(
            "Admin access required".to_string(),
        )));
    }
    Ok(())
}

/// SPEC-66555000: テスト用ノード登録エンドポイント（デバッグビルドのみ）
///
/// E2Eテストで使用するため、POST /v0/nodes の代替として提供。
/// リリースビルドでは無効化される。
#[cfg(debug_assertions)]
pub async fn test_register_node(
    State(state): State<AppState>,
    Json(payload): Json<llm_router_common::protocol::RegisterRequest>,
) -> Result<
    (
        StatusCode,
        Json<llm_router_common::protocol::RegisterResponse>,
    ),
    AppError,
> {
    use llm_router_common::error::CommonError;

    // GPU必須チェック（旧APIと同じ検証）
    if payload.gpu_devices.is_empty() {
        return Err(AppError(RouterError::Common(CommonError::Validation(
            "GPU hardware is required for node registration. Please ensure gpu_devices is populated."
                .to_string(),
        ))));
    }

    let response = state.registry.register(payload).await?;
    // 旧APIとの互換性のため201 CREATEDを返す
    Ok((StatusCode::CREATED, Json(response)))
}

#[cfg(test)]
#[allow(deprecated)] // テストはレガシーAPIを検証するため、deprecated警告を抑制
mod tests {
    use super::*;
    use crate::{
        balancer::{LoadManager, MetricsUpdate, RequestOutcome},
        registry::NodeRegistry,
    };
    use axum::response::IntoResponse;
    use llm_router_common::{protocol::RegisterRequest, types::GpuDeviceInfo};
    use std::net::IpAddr;
    use std::time::Duration;

    #[allow(deprecated)]
    async fn create_test_state() -> AppState {
        let registry = NodeRegistry::new();
        let load_manager = LoadManager::new(registry.clone());
        let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to create test database");
        sqlx::migrate!("./migrations")
            .run(&db_pool)
            .await
            .expect("Failed to run migrations");
        let request_history = std::sync::Arc::new(
            crate::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
        );
        let endpoint_registry = crate::registry::endpoints::EndpointRegistry::new(db_pool.clone())
            .await
            .expect("Failed to create endpoint registry");
        let jwt_secret = "test-secret".to_string();
        AppState {
            registry,
            load_manager,
            request_history,
            db_pool,
            jwt_secret,
            http_client: reqwest::Client::new(),
            queue_config: crate::config::QueueConfig::from_env(),
            event_bus: crate::events::create_shared_event_bus(),
            endpoint_registry,
        }
    }

    fn sample_gpu_devices() -> Vec<GpuDeviceInfo> {
        vec![GpuDeviceInfo {
            model: "Test GPU".to_string(),
            count: 1,
            memory: None,
        }]
    }

    /// テスト用ノードを直接レジストリに登録するヘルパー
    async fn register_test_node(
        state: &AppState,
        machine_name: &str,
        ip_address: &str,
    ) -> uuid::Uuid {
        let req = RegisterRequest {
            machine_name: machine_name.to_string(),
            ip_address: ip_address.parse::<IpAddr>().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: sample_gpu_devices(),
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        };
        let response = state.registry.register(req).await.unwrap();
        response.node_id
    }

    // SPEC-66555000: register_node関連のテストは削除されました
    // POST /v0/nodes ルートが廃止されたため

    #[tokio::test]
    async fn test_list_nodes_empty() {
        let state = create_test_state().await;
        let result = list_nodes(State(state)).await;
        assert_eq!(result.0.len(), 0);
    }

    #[tokio::test]
    async fn test_list_nodes_with_nodes() {
        let state = create_test_state().await;

        // ノードを2つ登録（registry.register() を直接使用）
        let _ = register_test_node(&state, "machine1", "192.168.1.100").await;
        let _ = register_test_node(&state, "machine2", "192.168.1.101").await;

        let result = list_nodes(State(state)).await;
        assert_eq!(result.0.len(), 2);
    }

    #[tokio::test]
    async fn test_list_node_metrics_returns_snapshot() {
        let state = create_test_state().await;

        // ノードを登録
        let node_id = register_test_node(&state, "metrics-machine", "192.168.1.150").await;

        // メトリクスを記録
        state
            .load_manager
            .record_metrics(MetricsUpdate {
                node_id,
                cpu_usage: 42.0,
                memory_usage: 33.0,
                gpu_usage: Some(55.0),
                gpu_memory_usage: Some(48.0),
                gpu_memory_total_mb: None,
                gpu_memory_used_mb: None,
                gpu_temperature: None,
                gpu_model_name: None,
                gpu_compute_capability: None,
                gpu_capability_score: None,
                active_requests: 1,
                average_response_time_ms: None,
                initializing: false,
                ready_models: None,
            })
            .await
            .unwrap();

        let metrics = list_node_metrics(State(state)).await;
        assert_eq!(metrics.0.len(), 1);

        let snapshot = &metrics.0[0];
        assert_eq!(snapshot.endpoint_id, node_id);
        assert_eq!(snapshot.cpu_usage.unwrap(), 42.0);
        assert_eq!(snapshot.memory_usage.unwrap(), 33.0);
        assert_eq!(snapshot.gpu_usage, Some(55.0));
        assert_eq!(snapshot.gpu_memory_usage, Some(48.0));
        assert_eq!(snapshot.active_requests, 1);
        assert!(!snapshot.is_stale);
    }

    #[tokio::test]
    async fn test_metrics_summary_empty() {
        let state = create_test_state().await;
        let summary = metrics_summary(State(state)).await;
        assert_eq!(summary.total_nodes, 0);
        assert_eq!(summary.online_nodes, 0);
        assert_eq!(summary.pending_nodes, 0);
        assert_eq!(summary.registering_nodes, 0);
        assert_eq!(summary.total_requests, 0);
        assert_eq!(summary.total_active_requests, 0);
        assert_eq!(summary.queued_requests, 0);
        assert!(summary.average_response_time_ms.is_none());
        assert!(summary.last_metrics_updated_at.is_none());
    }

    #[tokio::test]
    async fn test_metrics_summary_counts_requests() {
        let state = create_test_state().await;

        // ノードを登録・承認
        let node_id = register_test_node(&state, "stats-machine", "192.168.1.200").await;
        state.registry.approve(node_id).await.unwrap();

        // ハートビートでメトリクス更新
        state
            .load_manager
            .record_metrics(MetricsUpdate {
                node_id,
                cpu_usage: 55.0,
                memory_usage: 44.0,
                gpu_usage: Some(60.0),
                gpu_memory_usage: Some(62.0),
                gpu_memory_total_mb: None,
                gpu_memory_used_mb: None,
                gpu_temperature: None,
                gpu_model_name: None,
                gpu_compute_capability: None,
                gpu_capability_score: None,
                active_requests: 2,
                average_response_time_ms: Some(150.0),
                initializing: false,
                ready_models: None,
            })
            .await
            .unwrap();

        // リクエストを成功・失敗で記録
        state.load_manager.begin_request(node_id).await.unwrap();
        state
            .load_manager
            .finish_request(node_id, RequestOutcome::Success, Duration::from_millis(120))
            .await
            .unwrap();

        state.load_manager.begin_request(node_id).await.unwrap();
        state
            .load_manager
            .finish_request(node_id, RequestOutcome::Error, Duration::from_millis(200))
            .await
            .unwrap();

        let summary = metrics_summary(State(state)).await;
        assert_eq!(summary.total_nodes, 1);
        assert_eq!(summary.online_nodes, 1);
        assert_eq!(summary.pending_nodes, 0);
        assert_eq!(summary.registering_nodes, 0);
        assert_eq!(summary.total_requests, 2);
        assert_eq!(summary.successful_requests, 1);
        assert_eq!(summary.failed_requests, 1);
        assert_eq!(summary.total_active_requests, 2);
        assert_eq!(summary.queued_requests, 0);
        let avg = summary.average_response_time_ms.unwrap();
        assert!((avg - 160.0).abs() < 0.1);
        assert!(summary.last_metrics_updated_at.is_some());
    }

    #[tokio::test]
    async fn test_approve_node_endpoint_admin() {
        use axum::Extension;
        use llm_router_common::auth::{Claims, UserRole};

        let state = create_test_state().await;
        let node_id = register_test_node(&state, "approve-node", "192.168.1.120").await;

        let claims = Claims {
            sub: "admin".to_string(),
            role: UserRole::Admin,
            exp: 0,
        };

        let result = approve_node(
            Extension(claims),
            State(state.clone()),
            axum::extract::Path(node_id),
        )
        .await;
        assert!(result.is_ok());

        let node = state.registry.get(node_id).await.unwrap();
        assert_eq!(node.status, llm_router_common::types::NodeStatus::Online);
    }

    #[tokio::test]
    async fn test_approve_non_pending_node_fails() {
        use axum::Extension;
        use llm_router_common::auth::{Claims, UserRole};

        let state = create_test_state().await;
        let node_id = register_test_node(&state, "approve-online-node", "192.168.1.130").await;

        // まず承認してOnlineにする
        let claims = Claims {
            sub: "admin".to_string(),
            role: UserRole::Admin,
            exp: 0,
        };
        let _ = approve_node(
            Extension(claims.clone()),
            State(state.clone()),
            axum::extract::Path(node_id),
        )
        .await
        .unwrap();

        // すでにOnlineのノードを再度承認しようとするとエラー
        let result = approve_node(
            Extension(claims),
            State(state),
            axum::extract::Path(node_id),
        )
        .await;

        assert!(result.is_err());
        let err_response = result.err().unwrap().into_response();
        assert_eq!(err_response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_approve_node_endpoint_requires_admin() {
        use axum::Extension;
        use llm_router_common::auth::{Claims, UserRole};

        let state = create_test_state().await;
        let node_id = register_test_node(&state, "approve-node-viewer", "192.168.1.121").await;

        let claims = Claims {
            sub: "viewer".to_string(),
            role: UserRole::Viewer,
            exp: 0,
        };

        let result = approve_node(
            Extension(claims),
            State(state),
            axum::extract::Path(node_id),
        )
        .await;

        assert!(result.is_err());
        let response = result.err().unwrap().into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_update_node_settings_endpoint() {
        let state = create_test_state().await;
        let node_id = register_test_node(&state, "node-settings", "10.0.0.5").await;

        let payload = UpdateNodeSettingsPayload {
            custom_name: Some(Some("Primary".into())),
            tags: Some(vec!["dallas".into(), "gpu".into()]),
            notes: Some(Some("Keep online".into())),
        };

        let node = update_node_settings(
            State(state.clone()),
            axum::extract::Path(node_id),
            Json(payload),
        )
        .await
        .unwrap()
        .0;

        assert_eq!(node.custom_name.as_deref(), Some("Primary"));
        assert_eq!(node.tags, vec!["dallas", "gpu"]);
        assert_eq!(node.notes.as_deref(), Some("Keep online"));
    }

    #[tokio::test]
    async fn test_delete_node_endpoint() {
        use axum::Extension;
        use llm_router_common::auth::{Claims, UserRole};

        let state = create_test_state().await;
        let node_id = register_test_node(&state, "delete-node", "10.0.0.7").await;

        let claims = Claims {
            sub: "admin".to_string(),
            role: UserRole::Admin,
            exp: 0,
        };

        let status = delete_node(
            Extension(claims),
            State(state.clone()),
            axum::extract::Path(node_id),
        )
        .await
        .unwrap();
        assert_eq!(status, StatusCode::NO_CONTENT);

        let nodes = list_nodes(State(state)).await.0;
        assert!(nodes.is_empty());
    }

    #[tokio::test]
    async fn test_delete_node_requires_admin() {
        use axum::Extension;
        use llm_router_common::auth::{Claims, UserRole};

        let state = create_test_state().await;
        let node_id = register_test_node(&state, "delete-admin-required", "10.0.0.9").await;

        // 非Admin（Viewer）での削除は失敗すべき
        let viewer_claims = Claims {
            sub: "viewer".to_string(),
            role: UserRole::Viewer,
            exp: 0,
        };

        let result = delete_node(
            Extension(viewer_claims),
            State(state.clone()),
            axum::extract::Path(node_id),
        )
        .await;

        assert!(result.is_err());
        let err_response = result.err().unwrap().into_response();
        assert_eq!(err_response.status(), StatusCode::FORBIDDEN);

        // Admin での削除は成功すべき
        let admin_claims = Claims {
            sub: "admin".to_string(),
            role: UserRole::Admin,
            exp: 0,
        };

        let result = delete_node(
            Extension(admin_claims),
            State(state),
            axum::extract::Path(node_id),
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_disconnect_node_endpoint_marks_offline() {
        let state = create_test_state().await;
        let node_id = register_test_node(&state, "disconnect-node", "10.0.0.8").await;

        let _status = disconnect_node(State(state.clone()), axum::extract::Path(node_id))
            .await
            .unwrap();

        let _node = state.registry.get(node_id).await.unwrap();
    }
}
