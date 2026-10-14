//! Issue #900: HTTP integration coverage for /v1/models query filters.

use crate::support::{
    http::{spawn_lb, TestServer},
    lb::{build_test_router, create_test_db_pool},
};
use llmlb::{
    db::models::ModelStorage,
    registry::{endpoints::EndpointRegistry, models::ModelInfo},
    types::{
        endpoint::{Endpoint, EndpointModel, EndpointStatus, EndpointType, SupportedAPI},
        ModelCapability,
    },
};
use reqwest::{Client, StatusCode};
use serde_json::Value;
use serial_test::serial;
use sqlx::SqlitePool;
use uuid::Uuid;

struct FilterModel {
    id: &'static str,
    canonical_name: Option<&'static str>,
    supported_apis: Vec<SupportedAPI>,
    registered_capabilities: Option<Vec<ModelCapability>>,
}

impl FilterModel {
    fn unregistered(id: &'static str, supported_apis: Vec<SupportedAPI>) -> Self {
        Self {
            id,
            canonical_name: None,
            supported_apis,
            registered_capabilities: None,
        }
    }

    fn registered(id: &'static str, capabilities: Vec<ModelCapability>) -> Self {
        Self {
            id,
            canonical_name: None,
            // Deliberately report chat support for every row: registered boolean
            // capabilities, including false values, must remain authoritative.
            supported_apis: vec![SupportedAPI::ChatCompletions],
            registered_capabilities: Some(capabilities),
        }
    }
}

async fn insert_filter_endpoint(
    pool: &SqlitePool,
    name: &str,
    status: EndpointStatus,
    models: Vec<FilterModel>,
) -> Uuid {
    let registry = EndpointRegistry::new(pool.clone())
        .await
        .expect("create fixture registry");
    let mut endpoint = Endpoint::new(
        name.to_string(),
        format!("http://127.0.0.1:1/{}", Uuid::new_v4()),
        EndpointType::OpenaiCompatible,
    );
    endpoint.status = status;
    registry
        .add(endpoint.clone())
        .await
        .expect("insert fixture endpoint");

    for model in models {
        if let Some(capabilities) = model.registered_capabilities {
            let registered = ModelInfo::with_capabilities(
                model.canonical_name.unwrap_or(model.id).to_string(),
                0,
                "Issue #900 filter fixture".to_string(),
                0,
                vec![],
                capabilities,
            );
            ModelStorage::new(pool.clone())
                .save_model(&registered)
                .await
                .expect("save registered fixture model");
        }
        registry
            .add_model(&EndpointModel {
                endpoint_id: endpoint.id,
                model_id: model.id.to_string(),
                capabilities: None,
                max_tokens: None,
                last_checked: None,
                supported_apis: model.supported_apis,
                canonical_name: model.canonical_name.map(str::to_string),
            })
            .await
            .expect("insert fixture endpoint model");
    }
    endpoint.id
}

async fn spawn_filter_lb(pool: SqlitePool) -> TestServer {
    // Build the router after seeding so its registry loads these exact fixtures.
    // Unlike spawn_test_lb, this path does not mutate process environment.
    spawn_lb(build_test_router(pool).await).await
}

async fn models_response(lb: &TestServer, query: &[(&str, &str)]) -> reqwest::Response {
    Client::new()
        .get(format!("http://{}/v1/models", lb.addr()))
        .header("authorization", "Bearer sk_debug")
        .query(query)
        .send()
        .await
        .expect("request filtered model list")
}

async fn assert_model_ids(lb: &TestServer, query: &[(&str, &str)], expected: &[&str]) {
    let response = models_response(lb, query).await;
    assert_eq!(response.status(), StatusCode::OK, "query: {query:?}");
    let body: Value = response.json().await.expect("decode model list");
    assert_eq!(body["object"], "list");
    let mut actual: Vec<&str> = body["data"]
        .as_array()
        .expect("model list data")
        .iter()
        .map(|row| row["id"].as_str().expect("model id"))
        .collect();
    actual.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected, "query: {query:?}");
}

#[tokio::test]
#[serial]
async fn models_filter_registered_capabilities_and_type_aliases() {
    use ModelCapability::{
        Embedding, ImageGeneration, ImageInput, SpeechToText, TextGeneration, TextToSpeech,
    };

    let pool = create_test_db_pool().await;
    insert_filter_endpoint(
        &pool,
        "registered filter fixtures",
        EndpointStatus::Online,
        vec![
            FilterModel::registered("issue900/chat", vec![TextGeneration]),
            FilterModel::registered("issue900/tts", vec![TextToSpeech]),
            FilterModel::registered("issue900/stt", vec![SpeechToText]),
            FilterModel::registered("issue900/image", vec![ImageGeneration]),
            FilterModel::registered("issue900/vision", vec![ImageInput]),
            FilterModel::registered("issue900/embedding", vec![Embedding]),
            FilterModel::registered("issue900/multimodal", vec![TextGeneration, ImageInput]),
        ],
    )
    .await;
    let lb = spawn_filter_lb(pool).await;
    let all = [
        "issue900/chat",
        "issue900/tts",
        "issue900/stt",
        "issue900/image",
        "issue900/vision",
        "issue900/embedding",
        "issue900/multimodal",
    ];

    let cases: Vec<(&str, &str, Vec<&str>)> = vec![
        (
            "capability",
            "chat_completion",
            vec!["issue900/chat", "issue900/multimodal"],
        ),
        (
            "capability",
            "completion",
            vec!["issue900/chat", "issue900/multimodal"],
        ),
        ("capability", "embeddings", vec!["issue900/embedding"]),
        ("capability", "text_to_speech", vec!["issue900/tts"]),
        ("capability", "speech_to_text", vec!["issue900/stt"]),
        ("capability", "image_generation", vec!["issue900/image"]),
        (
            "capability",
            "image_input",
            vec!["issue900/vision", "issue900/multimodal"],
        ),
        ("capability", "inference", all.to_vec()),
        ("capability", "fine_tune", vec![]),
        (
            "capability",
            "chat_completion,image_input",
            vec!["issue900/multimodal"],
        ),
        ("capability", "text_to_speech,speech_to_text", vec![]),
        ("type", "llm", vec!["issue900/chat", "issue900/multimodal"]),
        ("type", "tts", vec!["issue900/tts"]),
        ("type", "stt", vec!["issue900/stt"]),
        ("type", "image", vec!["issue900/image"]),
        (
            "type",
            "vision",
            vec!["issue900/vision", "issue900/multimodal"],
        ),
        ("type", "embedding", vec!["issue900/embedding"]),
        ("type", "tts,stt", vec!["issue900/tts", "issue900/stt"]),
        (
            "type",
            "llm,vision",
            vec!["issue900/chat", "issue900/vision", "issue900/multimodal"],
        ),
    ];

    for (parameter, value, expected) in cases {
        assert_model_ids(
            &lb,
            &[("owned_by", "issue900"), (parameter, value)],
            &expected,
        )
        .await;
    }
    for (types, capability, expected) in [
        ("tts,stt", "text_to_speech", vec!["issue900/tts"]),
        ("llm", "image_input", vec!["issue900/multimodal"]),
        ("tts", "speech_to_text", vec![]),
    ] {
        assert_model_ids(
            &lb,
            &[
                ("owned_by", "issue900"),
                ("type", types),
                ("capability", capability),
            ],
            &expected,
        )
        .await;
    }
    lb.stop().await;
}

#[tokio::test]
#[serial]
async fn models_filter_unregistered_capabilities_derive_only_from_supported_apis() {
    use SupportedAPI::{ChatCompletions, Embeddings, ImageGeneration, ImageInput, Responses};

    let pool = create_test_db_pool().await;
    insert_filter_endpoint(
        &pool,
        "unregistered filter fixtures",
        EndpointStatus::Online,
        vec![
            FilterModel::unregistered("issue900/chat", vec![ChatCompletions]),
            FilterModel::unregistered("issue900/embedding", vec![Embeddings]),
            FilterModel::unregistered("issue900/image", vec![ImageGeneration]),
            FilterModel::unregistered("issue900/vision", vec![ImageInput]),
            FilterModel::unregistered("issue900/multimodal", vec![ChatCompletions, ImageInput]),
            FilterModel::unregistered("issue900/responses-only", vec![Responses]),
        ],
    )
    .await;
    let lb = spawn_filter_lb(pool).await;
    let baseline: Value = models_response(&lb, &[("owned_by", "issue900")])
        .await
        .json()
        .await
        .expect("decode unregistered rows");
    assert!(baseline["data"]
        .as_array()
        .expect("unregistered data")
        .iter()
        .all(|row| row.get("capabilities").is_none()));

    let cases: Vec<(&str, &str, Vec<&str>)> = vec![
        (
            "capability",
            "chat_completion",
            vec!["issue900/chat", "issue900/multimodal"],
        ),
        (
            "capability",
            "completion",
            vec!["issue900/chat", "issue900/multimodal"],
        ),
        ("capability", "embeddings", vec!["issue900/embedding"]),
        ("capability", "image_generation", vec!["issue900/image"]),
        (
            "capability",
            "image_input",
            vec!["issue900/vision", "issue900/multimodal"],
        ),
        (
            "capability",
            "chat_completion,image_input",
            vec!["issue900/multimodal"],
        ),
        ("capability", "text_to_speech", vec![]),
        ("capability", "speech_to_text", vec![]),
        ("capability", "fine_tune", vec![]),
        ("capability", "inference", vec![]),
        (
            "supported_api",
            "chat_completions",
            vec!["issue900/chat", "issue900/multimodal"],
        ),
        ("supported_api", "embeddings", vec!["issue900/embedding"]),
        ("supported_api", "image_generation", vec!["issue900/image"]),
        (
            "supported_api",
            "image_input",
            vec!["issue900/vision", "issue900/multimodal"],
        ),
        (
            "supported_api",
            "responses",
            vec![
                "issue900/chat",
                "issue900/embedding",
                "issue900/image",
                "issue900/vision",
                "issue900/multimodal",
                "issue900/responses-only",
            ],
        ),
    ];
    for (parameter, value, expected) in cases {
        assert_model_ids(
            &lb,
            &[("owned_by", "issue900"), (parameter, value)],
            &expected,
        )
        .await;
    }
    lb.stop().await;
}

#[tokio::test]
#[serial]
async fn models_filter_metadata_and_combined_conditions() {
    let pool = create_test_db_pool().await;
    let mut alias = FilterModel::unregistered(
        "Engine-Alias.Unique",
        vec![SupportedAPI::ChatCompletions, SupportedAPI::ImageInput],
    );
    alias.canonical_name = Some("issue900/canonical-target");
    let primary_id = insert_filter_endpoint(
        &pool,
        "primary filter fixtures",
        EndpointStatus::Online,
        vec![
            alias,
            FilterModel::unregistered("issue900/plain-id", vec![SupportedAPI::ChatCompletions]),
        ],
    )
    .await;
    let secondary_id = insert_filter_endpoint(
        &pool,
        "secondary filter fixtures",
        EndpointStatus::Online,
        vec![
            FilterModel::unregistered(
                "issue900/canonical-target",
                vec![SupportedAPI::ChatCompletions],
            ),
            FilterModel::unregistered("other900/secondary", vec![SupportedAPI::Embeddings]),
        ],
    )
    .await;
    insert_filter_endpoint(
        &pool,
        "offline filter fixtures",
        EndpointStatus::Offline,
        vec![FilterModel::unregistered(
            "issue900/offline",
            vec![SupportedAPI::ChatCompletions],
        )],
    )
    .await;
    let lb = spawn_filter_lb(pool).await;
    let primary = primary_id.to_string();
    let secondary = secondary_id.to_string();
    let missing = Uuid::new_v4().to_string();

    let cases = vec![
        (
            vec![("owned_by", "issue900")],
            vec!["issue900/canonical-target", "issue900/plain-id"],
        ),
        (vec![("owned_by", "issue")], vec![]),
        (vec![("owned_by", "ISSUE900")], vec![]),
        (
            vec![("endpoint_id", primary.as_str())],
            vec!["issue900/canonical-target", "issue900/plain-id"],
        ),
        (
            vec![("endpoint_id", secondary.as_str())],
            vec!["issue900/canonical-target", "other900/secondary"],
        ),
        (vec![("endpoint_id", missing.as_str())], vec![]),
        (
            vec![("owned_by", "issue900"), ("ready", "true")],
            vec!["issue900/canonical-target", "issue900/plain-id"],
        ),
        (vec![("ready", "false")], vec![]),
        (vec![("q", "PLAIN-ID")], vec!["issue900/plain-id"]),
        (
            vec![("q", "CANONICAL-TAR")],
            vec!["issue900/canonical-target"],
        ),
        (vec![("q", "ALIAS.UNI")], vec!["issue900/canonical-target"]),
        (vec![("q", "no-match-issue900")], vec![]),
        (
            vec![("owned_by", "issue900"), ("q", "")],
            vec!["issue900/canonical-target", "issue900/plain-id"],
        ),
        (
            vec![
                ("type", "llm"),
                ("capability", "image_input"),
                ("supported_api", "chat_completions"),
                ("ready", "true"),
                ("endpoint_id", primary.as_str()),
                ("owned_by", "issue900"),
                ("q", "ALIAS"),
            ],
            vec!["issue900/canonical-target"],
        ),
        (
            vec![
                ("supported_api", "embeddings"),
                ("endpoint_id", primary.as_str()),
            ],
            vec![],
        ),
    ];
    for (query, expected) in cases {
        assert_model_ids(&lb, &query, &expected).await;
    }
    lb.stop().await;
}

#[tokio::test]
#[serial]
async fn models_filter_invalid_values_return_openai_bad_request() {
    let lb = spawn_filter_lb(create_test_db_pool().await).await;
    for (parameter, value) in [
        ("capability", "unknown"),
        ("capability", "chat_completion,unknown"),
        ("capability", ""),
        ("capability", "chat_completion,"),
        ("type", "unknown"),
        ("type", "tts,unknown"),
        ("type", ""),
        ("type", "tts,"),
        ("supported_api", "unknown"),
        ("supported_api", ""),
        ("supported_api", "chat_completions,embeddings"),
        ("ready", "yes"),
        ("ready", "1"),
        ("ready", ""),
        ("endpoint_id", "not-a-uuid"),
        ("endpoint_id", ""),
    ] {
        let response = models_response(&lb, &[(parameter, value)]).await;
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "{parameter}={value}"
        );
        let body: Value = response.json().await.expect("decode OpenAI error");
        assert_eq!(
            body["error"]["type"], "invalid_request_error",
            "{parameter}={value}: {body}"
        );
        assert!(body["error"]["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty()));
    }
    lb.stop().await;
}
