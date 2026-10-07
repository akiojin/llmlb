use super::*;

// ===== /v1/models response correction tests (#575 US-001 補正対応) =====

/// helper: 指定 supported_apis を申告するオンラインエンドポイントを登録する
async fn add_endpoint_with_supported_apis(
    state: &AppState,
    endpoint_name: &str,
    model_id: &str,
    apis: Vec<crate::types::endpoint::SupportedAPI>,
) -> uuid::Uuid {
    add_endpoint_with_supported_apis_and_canonical_name(state, endpoint_name, model_id, apis, None)
        .await
}

/// helper: canonical_name を明示したオンラインエンドポイントを登録する
async fn add_endpoint_with_supported_apis_and_canonical_name(
    state: &AppState,
    endpoint_name: &str,
    model_id: &str,
    apis: Vec<crate::types::endpoint::SupportedAPI>,
    canonical_name: Option<&str>,
) -> uuid::Uuid {
    use crate::types::endpoint::{Endpoint, EndpointModel, EndpointStatus, EndpointType};
    let mut endpoint = Endpoint::new(
        endpoint_name.to_string(),
        format!("http://127.0.0.1:0/{endpoint_name}"),
        EndpointType::OpenaiCompatible,
    );
    endpoint.status = EndpointStatus::Online;
    let endpoint_id = endpoint.id;
    state
        .balancer
        .endpoint_registry
        .add(endpoint)
        .await
        .expect("add endpoint");
    state
        .balancer
        .endpoint_registry
        .add_model(&EndpointModel {
            endpoint_id,
            model_id: model_id.to_string(),
            capabilities: None,
            max_tokens: None,
            last_checked: None,
            supported_apis: apis,
            canonical_name: canonical_name.map(str::to_string),
        })
        .await
        .expect("add endpoint model");
    endpoint_id
}

/// helper: list_models() を呼び出し、JSON ボディを返す
async fn fetch_list_models(state: AppState) -> serde_json::Value {
    let resp = super::list_models(axum::extract::State(state))
        .await
        .expect("list_models ok");
    let bytes = to_bytes(resp.into_body(), 1_000_000)
        .await
        .expect("read body");
    serde_json::from_slice::<serde_json::Value>(&bytes).expect("parse json")
}

/// helper: GET /api/dashboard/models を呼び出し、JSON ボディを返す
async fn fetch_dashboard_models(state: AppState, view: &str) -> serde_json::Value {
    let resp = crate::api::dashboard::get_models(
        axum::extract::State(state),
        axum::extract::Query(
            serde_json::from_value(serde_json::json!({ "view": view })).expect("view query"),
        ),
    )
    .await
    .expect("dashboard get_models ok");
    let bytes = to_bytes(resp.into_body(), 1_000_000)
        .await
        .expect("read body");
    serde_json::from_slice::<serde_json::Value>(&bytes).expect("parse json")
}

fn find_model<'a>(data: &'a [serde_json::Value], id: &str) -> &'a serde_json::Value {
    data.iter()
        .find(|m| m["id"].as_str() == Some(id))
        .unwrap_or_else(|| panic!("model {id} not found in {data:?}"))
}

fn string_list(value: &serde_json::Value) -> Vec<&str> {
    value
        .as_array()
        .expect("array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect()
}

/// #722 AC-1/AC-2: 既知 canonical 名の行だけが is_canonical=true になり、
/// エンドポイントが報告した生のモデル名は aliases として残る
#[tokio::test]
#[serial]
async fn list_models_marks_only_known_canonical_ids_as_canonical() {
    let _guard = TEST_LOCK.lock().await;
    let (state, _dir) = create_state_with_tempdir().await;
    add_canonical_fixture(&state).await;

    let body = fetch_list_models(state).await;
    let data = body["data"].as_array().expect("data array");

    let canonical = find_model(data, "Qwen/Qwen3-VL-30B-A3B-Instruct");
    assert_eq!(
        canonical["canonical_name"].as_str(),
        Some("Qwen/Qwen3-VL-30B-A3B-Instruct")
    );
    assert_eq!(canonical["is_canonical"].as_bool(), Some(true));
    let aliases = string_list(&canonical["aliases"]);
    assert!(
        aliases.contains(&"qwen/qwen3-vl-30b") && aliases.contains(&"qwen3-vl-30b-q4"),
        "runtime aliases must remain visible for canonical row (got: {aliases:?})"
    );
    assert_eq!(
        string_list(&canonical["endpoint_ids"]).len(),
        2,
        "same canonical model served by two endpoints must be one row"
    );

    let unknown = find_model(data, "vendor/not-canonical");
    assert_eq!(
        unknown["canonical_name"].as_str(),
        Some("vendor/not-canonical")
    );
    assert_eq!(
        unknown["is_canonical"].as_bool(),
        Some(false),
        "unknown self-fallbacks must not be tagged canonical"
    );
    std::env::remove_var("LLMLB_DATA_DIR");
}

async fn add_canonical_fixture(state: &AppState) {
    use crate::types::endpoint::SupportedAPI;
    add_endpoint_with_supported_apis_and_canonical_name(
        state,
        "qwen-endpoint-a",
        "qwen/qwen3-vl-30b",
        vec![SupportedAPI::ChatCompletions],
        Some("Qwen/Qwen3-VL-30B-A3B-Instruct"),
    )
    .await;
    add_endpoint_with_supported_apis_and_canonical_name(
        state,
        "qwen-endpoint-b",
        "qwen3-vl-30b-q4",
        vec![SupportedAPI::ChatCompletions],
        Some("Qwen/Qwen3-VL-30B-A3B-Instruct"),
    )
    .await;
    add_endpoint_with_supported_apis(
        state,
        "unknown-endpoint",
        "vendor/not-canonical",
        vec![SupportedAPI::ChatCompletions],
    )
    .await;
}

/// #722 AC-1/AC-2: ダッシュボード API も canonical 行を識別できる
/// （canonical ビューは集約行、detail ビューは生のモデル名ごとに canonical_name を返す）
#[tokio::test]
#[serial]
async fn dashboard_models_expose_is_canonical_for_both_views() {
    let _guard = TEST_LOCK.lock().await;
    let (state, _dir) = create_state_with_tempdir().await;
    add_canonical_fixture(&state).await;

    let body = fetch_dashboard_models(state.clone(), "canonical").await;
    let data = body["data"].as_array().expect("data array");
    let canonical = find_model(data, "Qwen/Qwen3-VL-30B-A3B-Instruct");
    assert_eq!(canonical["is_canonical"].as_bool(), Some(true));
    assert_eq!(string_list(&canonical["endpoint_ids"]).len(), 2);
    let unknown = find_model(data, "vendor/not-canonical");
    assert_eq!(unknown["is_canonical"].as_bool(), Some(false));

    let body = fetch_dashboard_models(state, "detail").await;
    let data = body["data"].as_array().expect("data array");
    for raw in ["qwen/qwen3-vl-30b", "qwen3-vl-30b-q4"] {
        let row = find_model(data, raw);
        assert_eq!(
            row["canonical_name"].as_str(),
            Some("Qwen/Qwen3-VL-30B-A3B-Instruct"),
            "detail row {raw} must point at the shared canonical name"
        );
        assert_eq!(
            row["is_canonical"].as_bool(),
            Some(false),
            "raw endpoint name {raw} is not itself canonical"
        );
    }
    std::env::remove_var("LLMLB_DATA_DIR");
}

/// A-1 RED: 登録済み embedding モデルの supported_apis に "embeddings" が含まれること
#[tokio::test]
#[serial]
async fn list_models_embedding_capability_emits_embeddings_api() {
    use crate::registry::models::ModelInfo;
    use crate::types::endpoint::SupportedAPI;
    use crate::types::ModelCapability;

    let _guard = TEST_LOCK.lock().await;
    let (state, _dir) = create_state_with_tempdir().await;

    // embedding capability で登録
    let mut model = ModelInfo::with_capabilities(
        "nomic-ai/nomic-embed-text-v1.5".to_string(),
        0,
        "embedding model".to_string(),
        0,
        vec![],
        vec![ModelCapability::Embedding],
    );
    model.repo = Some("nomic-ai/nomic-embed-text-v1.5".to_string());
    let storage = crate::db::models::ModelStorage::new(state.db_pool.clone());
    storage.save_model(&model).await.expect("save model");

    // エンドポイントは ChatCompletions しか申告しない（実環境の典型ケース）
    add_endpoint_with_supported_apis(
        &state,
        "embed-endpoint",
        "nomic-ai/nomic-embed-text-v1.5",
        vec![SupportedAPI::ChatCompletions],
    )
    .await;

    let body = fetch_list_models(state).await;
    let data = body["data"].as_array().expect("data array");
    let model = data
        .iter()
        .find(|m| m["id"].as_str() == Some("nomic-ai/nomic-embed-text-v1.5"))
        .expect("embedding model in /v1/models");

    let apis: Vec<&str> = model["supported_apis"]
        .as_array()
        .expect("supported_apis array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(
        apis.contains(&"embeddings"),
        "embedding-capability model must report 'embeddings' in supported_apis (got: {:?})",
        apis
    );
    std::env::remove_var("LLMLB_DATA_DIR");
}

/// A-2 RED: 登録済みモデルの created は last_modified の Unix epoch を反映すること
#[tokio::test]
#[serial]
async fn list_models_registered_created_reflects_last_modified() {
    use crate::registry::models::ModelInfo;
    use crate::types::endpoint::SupportedAPI;
    use chrono::TimeZone;

    let _guard = TEST_LOCK.lock().await;
    let (state, _dir) = create_state_with_tempdir().await;

    let known_ts = chrono::Utc.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();
    let mut model = ModelInfo::new(
        "vendor/known-time-model".to_string(),
        0,
        "test".to_string(),
        0,
        vec![],
    );
    model.last_modified = Some(known_ts);
    let storage = crate::db::models::ModelStorage::new(state.db_pool.clone());
    storage.save_model(&model).await.expect("save model");

    add_endpoint_with_supported_apis(
        &state,
        "ts-endpoint",
        "vendor/known-time-model",
        vec![SupportedAPI::ChatCompletions],
    )
    .await;

    let body = fetch_list_models(state).await;
    let data = body["data"].as_array().expect("data array");
    let model = data
        .iter()
        .find(|m| m["id"].as_str() == Some("vendor/known-time-model"))
        .expect("model in /v1/models");

    let created = model["created"].as_i64().expect("created is integer");
    assert_eq!(
        created,
        known_ts.timestamp(),
        "registered model created must reflect last_modified epoch (got: {})",
        created
    );
    std::env::remove_var("LLMLB_DATA_DIR");
}

/// A-3 RED: supported_apis は as_str() 昇順で並ぶこと（決定論化）
#[tokio::test]
#[serial]
async fn list_models_supported_apis_are_sorted() {
    use crate::registry::models::ModelInfo;
    use crate::types::endpoint::SupportedAPI;
    use crate::types::ModelCapability;

    let _guard = TEST_LOCK.lock().await;
    let (state, _dir) = create_state_with_tempdir().await;

    // TextGeneration + Embedding の両対応モデル
    let model = ModelInfo::with_capabilities(
        "vendor/multi-cap".to_string(),
        0,
        "multi capability".to_string(),
        0,
        vec![],
        vec![ModelCapability::TextGeneration, ModelCapability::Embedding],
    );
    let storage = crate::db::models::ModelStorage::new(state.db_pool.clone());
    storage.save_model(&model).await.expect("save model");

    add_endpoint_with_supported_apis(
        &state,
        "multi-endpoint",
        "vendor/multi-cap",
        vec![SupportedAPI::ChatCompletions, SupportedAPI::Embeddings],
    )
    .await;

    let body = fetch_list_models(state).await;
    let data = body["data"].as_array().expect("data array");
    let model = data
        .iter()
        .find(|m| m["id"].as_str() == Some("vendor/multi-cap"))
        .expect("multi-cap model in /v1/models");

    let apis: Vec<&str> = model["supported_apis"]
        .as_array()
        .expect("supported_apis array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    let mut sorted = apis.clone();
    sorted.sort();
    assert_eq!(
        apis, sorted,
        "supported_apis must be in ascending lexicographic order (got: {:?})",
        apis
    );
    // 期待される3要素を網羅していること（A-1 の効果も込み）
    assert!(apis.contains(&"chat_completions"));
    assert!(apis.contains(&"embeddings"));
    assert!(apis.contains(&"responses"));
    std::env::remove_var("LLMLB_DATA_DIR");
}

/// Regression: capability 由来 set が非空でも endpoint_reported が必ずマージされること
/// （review #642 — ModelInfo がレガシー行で `[TextGeneration]` を既定返却することにより
/// endpoint が報告する `embeddings` 等が取りこぼされていた問題の検証）
#[tokio::test]
#[serial]
async fn list_models_endpoint_apis_are_merged_even_when_model_capabilities_default() {
    use crate::registry::models::ModelInfo;
    use crate::types::endpoint::SupportedAPI;

    let _guard = TEST_LOCK.lock().await;
    let (state, _dir) = create_state_with_tempdir().await;

    // capabilities を明示せず登録（=> get_capabilities() は [TextGeneration] を既定返却）
    let model = ModelInfo::new(
        "vendor/legacy-row".to_string(),
        0,
        "legacy registration without capabilities".to_string(),
        0,
        vec![],
    );
    let storage = crate::db::models::ModelStorage::new(state.db_pool.clone());
    storage.save_model(&model).await.expect("save model");

    // endpoint は Embeddings をサポートしている（実態を申告）
    add_endpoint_with_supported_apis(
        &state,
        "embed-endpoint",
        "vendor/legacy-row",
        vec![SupportedAPI::Embeddings],
    )
    .await;

    let body = fetch_list_models(state).await;
    let data = body["data"].as_array().expect("data array");
    let model = data
        .iter()
        .find(|m| m["id"].as_str() == Some("vendor/legacy-row"))
        .expect("model in /v1/models");

    let apis: Vec<&str> = model["supported_apis"]
        .as_array()
        .expect("supported_apis array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(
            apis.contains(&"embeddings"),
            "endpoint-reported `embeddings` must not be dropped even when capability-derived set is non-empty (got: {:?})",
            apis
        );
    std::env::remove_var("LLMLB_DATA_DIR");
}

/// G-3 暫定: 量子化サフィックス付きモデル ID から `quantization` フィールドが分離されること
#[tokio::test]
#[serial]
async fn list_models_quantization_suffix_is_emitted_as_separate_field() {
    use crate::types::endpoint::{
        Endpoint, EndpointModel, EndpointStatus, EndpointType, SupportedAPI,
    };

    let _guard = TEST_LOCK.lock().await;
    let (state, _dir) = create_state_with_tempdir().await;

    // 1 つのエンドポイントに「量子化付き」と「量子化なし」両方のモデルを登録して比較
    let mut endpoint = Endpoint::new(
        "quant-endpoint".to_string(),
        "http://127.0.0.1:0".to_string(),
        EndpointType::OpenaiCompatible,
    );
    endpoint.status = EndpointStatus::Online;
    let endpoint_id = endpoint.id;
    state
        .balancer
        .endpoint_registry
        .add(endpoint)
        .await
        .expect("add endpoint");
    for model_id in ["ggml-org/gemma-4-E4B-it-GGUF:Q4_K_M", "openai/gpt-oss-20b"] {
        state
            .balancer
            .endpoint_registry
            .add_model(&EndpointModel {
                endpoint_id,
                model_id: model_id.to_string(),
                capabilities: None,
                max_tokens: None,
                last_checked: None,
                supported_apis: vec![SupportedAPI::ChatCompletions],
                canonical_name: None,
            })
            .await
            .expect("add endpoint model");
    }

    let body = fetch_list_models(state).await;
    let data = body["data"].as_array().expect("data array");

    let gguf = data
        .iter()
        .find(|m| m["id"].as_str() == Some("ggml-org/gemma-4-E4B-it-GGUF:Q4_K_M"))
        .expect("gguf model in /v1/models");
    assert_eq!(
        gguf["quantization"].as_str(),
        Some("Q4_K_M"),
        "quantization suffix must be emitted as separate field"
    );
    assert_eq!(
        gguf["id"].as_str(),
        Some("ggml-org/gemma-4-E4B-it-GGUF:Q4_K_M"),
        "id must remain unchanged for backward compatibility"
    );

    let plain = data
        .iter()
        .find(|m| m["id"].as_str() == Some("openai/gpt-oss-20b"))
        .expect("non-gguf model in /v1/models");
    assert!(
        plain["quantization"].is_null(),
        "non-quantized model must report quantization: null (got: {:?})",
        plain["quantization"]
    );
    std::env::remove_var("LLMLB_DATA_DIR");
}

/// B-1/G-7: endpoint が max_tokens を申告しない場合に既知 canonical テーブルから補填すること
#[tokio::test]
#[serial]
async fn list_models_max_tokens_falls_back_to_known_canonical_table() {
    use crate::registry::models::ModelInfo;
    use crate::types::endpoint::SupportedAPI;

    let _guard = TEST_LOCK.lock().await;
    let (state, _dir) = create_state_with_tempdir().await;

    // 既知 canonical のモデルを登録（テーブルには 131072 がある）
    let model = ModelInfo::new(
        "openai/gpt-oss-20b".to_string(),
        0,
        "test".to_string(),
        0,
        vec![],
    );
    let storage = crate::db::models::ModelStorage::new(state.db_pool.clone());
    storage.save_model(&model).await.expect("save model");

    // endpoint は max_tokens を申告しない（add_endpoint_with_supported_apis のヘルパは
    // EndpointModel.max_tokens=None で登録する）
    add_endpoint_with_supported_apis(
        &state,
        "fallback-endpoint",
        "openai/gpt-oss-20b",
        vec![SupportedAPI::ChatCompletions],
    )
    .await;

    let body = fetch_list_models(state).await;
    let data = body["data"].as_array().expect("data array");
    let model = data
        .iter()
        .find(|m| m["id"].as_str() == Some("openai/gpt-oss-20b"))
        .expect("model in /v1/models");

    let max_tokens = model["max_tokens"]
        .as_u64()
        .expect("max_tokens fallback applied");
    assert_eq!(
        max_tokens, 131_072,
        "max_tokens must fall back to KNOWN_CONTEXT_LENGTHS for known canonical (got: {})",
        max_tokens
    );
    std::env::remove_var("LLMLB_DATA_DIR");
}

/// A-5 RED: owned_by は id の組織プレフィックス由来になること（"load balancer" 固定値からの脱却）
#[tokio::test]
#[serial]
async fn list_models_owned_by_uses_id_org_prefix() {
    use crate::registry::models::ModelInfo;
    use crate::types::endpoint::SupportedAPI;

    let _guard = TEST_LOCK.lock().await;
    let (state, _dir) = create_state_with_tempdir().await;

    let model = ModelInfo::new(
        "Qwen/Qwen3-Coder-30B-A3B-Instruct".to_string(),
        0,
        "test".to_string(),
        0,
        vec![],
    );
    let storage = crate::db::models::ModelStorage::new(state.db_pool.clone());
    storage.save_model(&model).await.expect("save model");

    add_endpoint_with_supported_apis(
        &state,
        "qwen-endpoint",
        "Qwen/Qwen3-Coder-30B-A3B-Instruct",
        vec![SupportedAPI::ChatCompletions],
    )
    .await;

    let body = fetch_list_models(state).await;
    let data = body["data"].as_array().expect("data array");
    let model = data
        .iter()
        .find(|m| m["id"].as_str() == Some("Qwen/Qwen3-Coder-30B-A3B-Instruct"))
        .expect("model in /v1/models");

    let owned_by = model["owned_by"].as_str().expect("owned_by string");
    assert_eq!(
        owned_by, "Qwen",
        "owned_by must derive from id organization prefix (got: {})",
        owned_by
    );
    std::env::remove_var("LLMLB_DATA_DIR");
}
