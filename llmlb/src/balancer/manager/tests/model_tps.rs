use super::*;

// SPEC-4bb5b55f T002: ModelTpsState EMA計算テスト

#[test]
fn test_model_tps_state_initial_none() {
    let state = ModelTpsState::default();
    assert!(state.tps_ema.is_none());
    assert_eq!(state.request_count, 0);
    assert_eq!(state.total_output_tokens, 0);
    assert_eq!(state.total_duration_ms, 0);
}

#[test]
fn test_model_tps_state_first_update() {
    let mut state = ModelTpsState::default();
    state.update_tps(100, 2000);
    assert!(state.tps_ema.is_some());
    let tps = state.tps_ema.unwrap();
    assert!(
        (tps - 50.0).abs() < 0.01,
        "初回TPS: expected 50.0, got {tps}"
    );
    assert_eq!(state.request_count, 1);
    assert_eq!(state.total_output_tokens, 100);
    assert_eq!(state.total_duration_ms, 2000);
}

#[test]
fn test_model_tps_state_ema_smoothing() {
    let mut state = ModelTpsState::default();

    state.update_tps(100, 2000);
    assert!((state.tps_ema.unwrap() - 50.0).abs() < 0.01);

    state.update_tps(200, 2000);
    assert!(
        (state.tps_ema.unwrap() - 60.0).abs() < 0.01,
        "2回目EMA: expected 60.0, got {}",
        state.tps_ema.unwrap()
    );

    state.update_tps(50, 1000);
    assert!(
        (state.tps_ema.unwrap() - 58.0).abs() < 0.01,
        "3回目EMA: expected 58.0, got {}",
        state.tps_ema.unwrap()
    );

    assert_eq!(state.request_count, 3);
    assert_eq!(state.total_output_tokens, 350);
    assert_eq!(state.total_duration_ms, 5000);
}

#[test]
fn test_model_tps_state_zero_duration_skipped() {
    let mut state = ModelTpsState::default();
    state.update_tps(100, 0);
    assert!(state.tps_ema.is_none(), "duration=0ではTPS更新しない");
}

#[tokio::test]
async fn test_get_model_tps_empty_for_unknown_endpoint() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, _) = setup_test_load_manager().await;
    let unknown_id = Uuid::new_v4();
    let result = load_manager.get_model_tps(unknown_id).await;
    assert!(result.is_empty(), "未計測エンドポイントは空Vecを返す");
}

#[tokio::test]
async fn test_get_model_tps_returns_entries_after_update() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;

    load_manager
        .update_tps(
            endpoint_id,
            "model-a".to_string(),
            TpsApiKind::ChatCompletions,
            100,
            2000,
        )
        .await;

    let result = load_manager.get_model_tps(endpoint_id).await;
    assert_eq!(result.len(), 1, "1モデル分のTPS情報が返る");
    assert_eq!(result[0].model_id, "model-a");
    assert_eq!(result[0].api_kind, TpsApiKind::ChatCompletions);
    assert_eq!(result[0].request_count, 1);
    assert_eq!(result[0].total_output_tokens, 100);
    let tps = result[0].tps.expect("TPS値がSomeであること");
    assert!((tps - 50.0).abs() < 0.01, "TPS = 50.0");
}

#[tokio::test]
async fn test_get_model_tps_multiple_models() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;

    load_manager
        .update_tps(
            endpoint_id,
            "model-a".to_string(),
            TpsApiKind::ChatCompletions,
            100,
            2000,
        )
        .await;
    load_manager
        .update_tps(
            endpoint_id,
            "model-b".to_string(),
            TpsApiKind::Completions,
            200,
            1000,
        )
        .await;

    let result = load_manager.get_model_tps(endpoint_id).await;
    assert_eq!(result.len(), 2, "2モデル分のTPS情報が返る");

    let model_ids: Vec<&str> = result.iter().map(|e| e.model_id.as_str()).collect();
    assert!(model_ids.contains(&"model-a"));
    assert!(model_ids.contains(&"model-b"));
}

#[tokio::test]
async fn test_get_model_tps_separates_api_kind_for_same_model() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;

    load_manager
        .update_tps(
            endpoint_id,
            "shared-model".to_string(),
            TpsApiKind::ChatCompletions,
            100,
            2000,
        )
        .await;
    load_manager
        .update_tps(
            endpoint_id,
            "shared-model".to_string(),
            TpsApiKind::Responses,
            80,
            2000,
        )
        .await;

    let result = load_manager.get_model_tps(endpoint_id).await;
    assert_eq!(result.len(), 2, "同一モデルでもAPI種別ごとに分離される");
    assert!(result
        .iter()
        .any(|entry| entry.model_id == "shared-model"
            && entry.api_kind == TpsApiKind::ChatCompletions));
    assert!(result
        .iter()
        .any(|entry| entry.model_id == "shared-model" && entry.api_kind == TpsApiKind::Responses));
}

#[tokio::test]
async fn test_update_tps_skips_zero_tokens() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;

    load_manager
        .update_tps(
            endpoint_id,
            "model-a".to_string(),
            TpsApiKind::ChatCompletions,
            0,
            2000,
        )
        .await;

    let result = load_manager.get_model_tps(endpoint_id).await;
    assert!(result.is_empty(), "output_tokens=0はTPS更新しない");
}

#[tokio::test]
async fn test_get_model_tps_isolates_endpoints() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;
    let other_endpoint_id = Uuid::new_v4();

    load_manager
        .update_tps(
            endpoint_id,
            "model-a".to_string(),
            TpsApiKind::ChatCompletions,
            100,
            2000,
        )
        .await;
    load_manager
        .update_tps(
            other_endpoint_id,
            "model-b".to_string(),
            TpsApiKind::Completions,
            200,
            1000,
        )
        .await;

    let result = load_manager.get_model_tps(endpoint_id).await;
    assert_eq!(result.len(), 1, "他エンドポイントのデータは含まない");
    assert_eq!(result[0].model_id, "model-a");
}

#[tokio::test]
async fn test_get_all_endpoint_tps_empty() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, _) = setup_test_load_manager().await;

    let result = load_manager.get_all_endpoint_tps().await;
    assert!(result.is_empty(), "TPS未計測の場合は空");
}

#[tokio::test]
async fn test_get_all_endpoint_tps_returns_per_endpoint_summary() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;
    let other_endpoint_id = Uuid::new_v4();

    load_manager
        .update_tps(
            endpoint_id,
            "model-a".to_string(),
            TpsApiKind::ChatCompletions,
            100,
            2000,
        )
        .await;
    load_manager
        .update_tps(
            endpoint_id,
            "model-b".to_string(),
            TpsApiKind::Completions,
            200,
            1000,
        )
        .await;
    load_manager
        .update_tps(
            other_endpoint_id,
            "model-c".to_string(),
            TpsApiKind::Responses,
            50,
            500,
        )
        .await;

    let result = load_manager.get_all_endpoint_tps().await;
    assert_eq!(result.len(), 2, "2エンドポイント分のサマリ");

    let ep1 = result
        .iter()
        .find(|s| s.endpoint_id == endpoint_id)
        .expect("endpoint_id存在");
    assert_eq!(ep1.model_count, 2);
    assert_eq!(ep1.total_output_tokens, 300);
    assert!(ep1.aggregate_tps.is_some());

    let ep2 = result
        .iter()
        .find(|s| s.endpoint_id == other_endpoint_id)
        .expect("other存在");
    assert_eq!(ep2.model_count, 1);
    assert_eq!(ep2.total_output_tokens, 50);
}

#[tokio::test]
async fn test_forget_endpoint_clears_state_and_tps() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;

    // 負荷状態（state）を生成
    load_manager
        .upsert_initial_state(endpoint_id, false, Some((1, 1)))
        .await;
    // TPS状態（tps_tracker）を生成
    load_manager
        .update_tps(
            endpoint_id,
            "model-a".to_string(),
            TpsApiKind::ChatCompletions,
            100,
            1_000,
        )
        .await;

    assert!(
        load_manager.state.read().await.contains_key(&endpoint_id),
        "前提: 負荷状態が登録されている"
    );
    assert!(
        !load_manager.get_model_tps(endpoint_id).await.is_empty(),
        "前提: TPS状態が登録されている"
    );

    load_manager.forget_endpoint(endpoint_id).await;

    assert!(
        !load_manager.state.read().await.contains_key(&endpoint_id),
        "forget_endpoint は負荷状態を除去する"
    );
    assert!(
        load_manager.get_model_tps(endpoint_id).await.is_empty(),
        "forget_endpoint はTPS状態を除去する"
    );
}

/// T018 [US4]: seed_tps_from_db が TpsSeedEntry から TpsTrackerMap に
/// 正しく TPS EMA を計算して投入できることを検証
#[tokio::test]
async fn test_seed_tps_from_db() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;

    let entries = vec![
        crate::db::endpoint_daily_stats::TpsSeedEntry {
            endpoint_id,
            model_id: "test-model".to_string(),
            api_kind: "chat_completions".to_string(),
            total_output_tokens: 100,
            total_duration_ms: 2000,
            successful_requests: 5,
        },
        // duration_ms=0 のエントリはスキップされること
        crate::db::endpoint_daily_stats::TpsSeedEntry {
            endpoint_id,
            model_id: "skip-model".to_string(),
            api_kind: "chat_completions".to_string(),
            total_output_tokens: 100,
            total_duration_ms: 0,
            successful_requests: 1,
        },
        // tokens=0 のエントリもスキップされること
        crate::db::endpoint_daily_stats::TpsSeedEntry {
            endpoint_id,
            model_id: "skip-model2".to_string(),
            api_kind: "chat_completions".to_string(),
            total_output_tokens: 0,
            total_duration_ms: 1000,
            successful_requests: 1,
        },
    ];

    load_manager.seed_tps_from_db(entries).await;

    let tps_data = load_manager.get_all_endpoint_tps().await;
    // test-model のみ seed されている
    assert!(!tps_data.is_empty(), "should have TPS data");
    let ep_tps = tps_data
        .iter()
        .find(|t| t.endpoint_id == endpoint_id)
        .expect("should have endpoint TPS");
    // TPS = 100 / (2000/1000) = 50.0
    assert_eq!(ep_tps.model_count, 1, "only valid entry should be seeded");
    assert!(ep_tps.aggregate_tps.is_some(), "should have aggregate TPS");
    let tps = ep_tps.aggregate_tps.unwrap();
    assert!((tps - 50.0).abs() < 0.1, "TPS should be ~50.0, got {tps}");
}

// ===== seed_tps_from_db api_kind マッピングテスト =====

#[tokio::test]
async fn seed_tps_from_db_maps_api_kind_correctly() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;

    let entries = vec![
        crate::db::endpoint_daily_stats::TpsSeedEntry {
            endpoint_id,
            model_id: "model-completions".to_string(),
            api_kind: "completions".to_string(),
            total_output_tokens: 100,
            total_duration_ms: 1000,
            successful_requests: 1,
        },
        crate::db::endpoint_daily_stats::TpsSeedEntry {
            endpoint_id,
            model_id: "model-responses".to_string(),
            api_kind: "responses".to_string(),
            total_output_tokens: 200,
            total_duration_ms: 2000,
            successful_requests: 2,
        },
        crate::db::endpoint_daily_stats::TpsSeedEntry {
            endpoint_id,
            model_id: "model-default".to_string(),
            api_kind: "unknown_type".to_string(),
            total_output_tokens: 50,
            total_duration_ms: 500,
            successful_requests: 1,
        },
    ];

    load_manager.seed_tps_from_db(entries).await;

    let tps = load_manager.get_model_tps(endpoint_id).await;
    assert_eq!(tps.len(), 3);
    assert!(tps
        .iter()
        .any(|t| t.model_id == "model-completions" && t.api_kind == TpsApiKind::Completions));
    assert!(tps
        .iter()
        .any(|t| t.model_id == "model-responses" && t.api_kind == TpsApiKind::Responses));
    assert!(tps
        .iter()
        .any(|t| t.model_id == "model-default" && t.api_kind == TpsApiKind::ChatCompletions));
}
