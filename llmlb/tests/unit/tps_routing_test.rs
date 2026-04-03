//! TPS優先ルーティング選択のUnit Test
//!
//! SPEC #494 Phase 3: TPSベースエンドポイント選択
//! - 高TPSエンドポイントの優先選択
//! - TPS未計測エンドポイントはTPS=0.0で最低優先
//! - 同一TPS時のラウンドロビンタイブレーク
//! - オフライン時TPS=0.0リセット

use llmlb::balancer::LoadManager;
use llmlb::common::protocol::TpsApiKind;
use llmlb::registry::endpoints::EndpointRegistry;
use llmlb::types::endpoint::{Endpoint, EndpointModel, EndpointStatus, EndpointType, SupportedAPI};
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;
use uuid::Uuid;

/// テスト用のLoadManager + エンドポイントをセットアップ
async fn setup_load_manager_with_endpoints(
    endpoint_names: &[&str],
    model_id: &str,
) -> (LoadManager, Vec<Uuid>, Arc<EndpointRegistry>) {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    let registry = Arc::new(
        EndpointRegistry::new(pool)
            .await
            .expect("Failed to create endpoint registry"),
    );

    let mut ids = Vec::new();
    for (i, name) in endpoint_names.iter().enumerate() {
        let mut endpoint = Endpoint::new(
            name.to_string(),
            format!("http://localhost:{}", 11080 + i),
            EndpointType::OpenaiCompatible,
        );
        endpoint.status = EndpointStatus::Online;
        let id = endpoint.id;
        ids.push(id);
        registry
            .add(endpoint)
            .await
            .expect("Failed to add endpoint");

        registry
            .add_model(&EndpointModel {
                endpoint_id: id,
                model_id: model_id.to_string(),
                capabilities: None,
                max_tokens: None,
                last_checked: None,
                supported_apis: vec![SupportedAPI::ChatCompletions],
                canonical_name: None,
            })
            .await
            .expect("Failed to add endpoint model");
    }

    let load_manager = LoadManager::new(registry.clone());
    for &id in &ids {
        load_manager.upsert_initial_state(id, false, None).await;
    }

    (load_manager, ids, registry)
}

#[tokio::test]
async fn test_tps_selection_prefers_highest_tps() {
    let model_id = "test-model";
    let (lm, ids, _reg) =
        setup_load_manager_with_endpoints(&["Slow", "Fast", "Medium"], model_id).await;

    // TPS値を設定: Slow=20, Fast=100, Medium=50
    lm.update_tps(
        ids[0],
        model_id.to_string(),
        TpsApiKind::ChatCompletions,
        200,
        10000,
    )
    .await; // 20 tok/s
    lm.update_tps(
        ids[1],
        model_id.to_string(),
        TpsApiKind::ChatCompletions,
        1000,
        10000,
    )
    .await; // 100 tok/s
    lm.update_tps(
        ids[2],
        model_id.to_string(),
        TpsApiKind::ChatCompletions,
        500,
        10000,
    )
    .await; // 50 tok/s

    let selected = lm
        .select_endpoint_by_tps_for_model(model_id)
        .await
        .expect("selection should succeed");

    assert_eq!(
        selected.name, "Fast",
        "highest TPS endpoint should be selected"
    );
}

#[tokio::test]
async fn test_tps_selection_unmeasured_is_lowest_priority() {
    let model_id = "test-model";
    let (lm, ids, _reg) =
        setup_load_manager_with_endpoints(&["Measured", "Unmeasured"], model_id).await;

    // MeasuredのみTPS設定、Unmeasuredは未計測
    lm.update_tps(
        ids[0],
        model_id.to_string(),
        TpsApiKind::ChatCompletions,
        100,
        10000,
    )
    .await; // 10 tok/s

    let selected = lm
        .select_endpoint_by_tps_for_model(model_id)
        .await
        .expect("selection should succeed");

    assert_eq!(
        selected.name, "Measured",
        "measured endpoint should be preferred over unmeasured"
    );
}

#[tokio::test]
async fn test_tps_selection_all_unmeasured_uses_round_robin() {
    let model_id = "test-model";
    let (lm, _ids, _reg) = setup_load_manager_with_endpoints(&["A", "B", "C"], model_id).await;

    // TPS未計測のまま → ラウンドロビンフォールバック
    let selected1 = lm
        .select_endpoint_by_tps_for_model(model_id)
        .await
        .expect("selection should succeed");
    let selected2 = lm
        .select_endpoint_by_tps_for_model(model_id)
        .await
        .expect("selection should succeed");

    // 全て同一TPS(0.0)なのでラウンドロビンにより異なるエンドポイントが選択される
    assert_ne!(
        selected1.id, selected2.id,
        "round-robin should cycle through endpoints when all TPS are equal"
    );
}

#[tokio::test]
async fn test_tps_selection_same_tps_round_robin_tiebreak() {
    let model_id = "test-model";
    let (lm, ids, _reg) = setup_load_manager_with_endpoints(&["A", "B"], model_id).await;

    // 同一TPS: 50 tok/s
    lm.update_tps(
        ids[0],
        model_id.to_string(),
        TpsApiKind::ChatCompletions,
        500,
        10000,
    )
    .await;
    lm.update_tps(
        ids[1],
        model_id.to_string(),
        TpsApiKind::ChatCompletions,
        500,
        10000,
    )
    .await;

    let mut selected_ids = std::collections::HashSet::new();
    for _ in 0..10 {
        let selected = lm
            .select_endpoint_by_tps_for_model(model_id)
            .await
            .expect("selection should succeed");
        selected_ids.insert(selected.id);
    }

    assert_eq!(
        selected_ids.len(),
        2,
        "round-robin should distribute between endpoints with same TPS"
    );
}

#[tokio::test]
async fn test_tps_selection_direct_without_model() {
    let model_id = "test-model";
    let (lm, ids, _reg) = setup_load_manager_with_endpoints(&["Slow", "Fast"], model_id).await;

    lm.update_tps(
        ids[0],
        model_id.to_string(),
        TpsApiKind::ChatCompletions,
        100,
        10000,
    )
    .await; // 10 tok/s
    lm.update_tps(
        ids[1],
        model_id.to_string(),
        TpsApiKind::ChatCompletions,
        800,
        10000,
    )
    .await; // 80 tok/s

    let selected = lm
        .select_endpoint_by_tps_direct()
        .await
        .expect("selection should succeed");

    assert_eq!(
        selected.name, "Fast",
        "highest TPS endpoint should be selected"
    );
}

#[tokio::test]
async fn test_tps_selection_no_endpoints_returns_error() {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    let registry = Arc::new(
        EndpointRegistry::new(pool)
            .await
            .expect("Failed to create endpoint registry"),
    );
    let lm = LoadManager::new(registry);

    let result = lm.select_endpoint_by_tps_direct().await;
    assert!(
        result.is_err(),
        "should return error when no endpoints available"
    );
}

#[tokio::test]
async fn test_tps_selection_single_endpoint() {
    let model_id = "test-model";
    let (lm, ids, _reg) = setup_load_manager_with_endpoints(&["Only"], model_id).await;

    lm.update_tps(
        ids[0],
        model_id.to_string(),
        TpsApiKind::ChatCompletions,
        500,
        10000,
    )
    .await;

    let selected = lm
        .select_endpoint_by_tps_for_model(model_id)
        .await
        .expect("selection should succeed");

    assert_eq!(selected.name, "Only");
}
