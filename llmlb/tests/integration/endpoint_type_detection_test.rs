//! Integration Test: US6 - エンドポイントタイプ自動判別
//!
//! SPEC-e8e9326e: エンドポイントタイプ自動判別機能
//!
//! 管理者として、エンドポイント登録時に自動的にタイプ
//! （xLLM/Ollama/vLLM/OpenAI互換）を判別してほしい。

use llmlb::common::auth::{ApiKeyPermission, UserRole};
use llmlb::health::EndpointHealthChecker;
use llmlb::registry::endpoints::EndpointRegistry;
use reqwest::Client;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

use crate::support::lb::spawn_test_lb_with_db;

fn family_fixture(engine: &str, case: &str, probe: &str) -> (u16, String, Value) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/detection")
        .join(engine);
    let manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("manifest.json"))
            .expect("Issue #908 requires upstream fixtures with recorded provenance"),
    )
    .unwrap();
    let response = manifest["responses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["case"] == case && entry["path"] == probe)
        .expect("fixture response exists");
    let body = std::fs::read_to_string(root.join(response["body_file"].as_str().unwrap())).unwrap();
    (
        response["status"].as_u64().unwrap() as u16,
        body,
        response["headers"].clone(),
    )
}

fn family_health(engine: &str, case: &str) -> Value {
    let (status, body, _) = family_fixture(engine, case, "/health");
    assert_eq!(status, 200);
    serde_json::from_str(&body).unwrap()
}

fn family_response(engine: &str, case: &str, probe: &str) -> ResponseTemplate {
    let (status, body, headers) = family_fixture(engine, case, probe);
    let mut response = ResponseTemplate::new(status).set_body_string(body);
    for (name, value) in headers.as_object().unwrap() {
        response = response.insert_header(name.as_str(), value.as_str().unwrap());
    }
    response
}

async fn create_admin_api_key(db_pool: &SqlitePool) -> String {
    let password_hash = llmlb::auth::password::hash_password("password123").unwrap();
    let created =
        llmlb::db::users::create(db_pool, "admin", &password_hash, UserRole::Admin, false).await;
    let admin_id = match created {
        Ok(user) => user.id,
        Err(_) => {
            llmlb::db::users::find_by_username(db_pool, "admin")
                .await
                .unwrap()
                .unwrap()
                .id
        }
    };

    let api_key = llmlb::db::api_keys::create(
        db_pool,
        "test-admin-key",
        admin_id,
        None,
        ApiKeyPermission::all(),
    )
    .await
    .expect("create admin api key");
    api_key.key
}

/// Issue #887 AC-5: engine identities survive registration and SQLite reload.
#[tokio::test]
async fn test_additional_engine_registration_and_persistence() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;
    // Keep each server alive: wiremock can recycle a dropped server's URL,
    // which is still registered in the database and would correctly return 409.
    let mut mocks = Vec::new();
    for engine in [
        "sglang",
        "lmdeploy",
        "xinference",
        "lemonade",
        "tgi",
        "koboldcpp",
    ] {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "object": "list", "data": [{"id": "test", "owned_by": engine}]
            })))
            .mount(&mock)
            .await;
        let probe = match engine {
            "tgi" => Some(("/info", json!({"router": "text-generation-router"}))),
            "koboldcpp" => Some(("/api/extra/version", json!({"result": "KoboldCpp"}))),
            _ => None,
        };
        if let Some((probe_path, body)) = probe {
            Mock::given(method("GET"))
                .and(path(probe_path))
                .respond_with(ResponseTemplate::new(200).set_body_json(body))
                .mount(&mock)
                .await;
        }
        if matches!(engine, "sglang" | "lemonade" | "koboldcpp") {
            // These engines expose Ollama-compatible tags alongside their
            // dedicated identities. Registration must preserve the native type.
            Mock::given(method("GET"))
                .and(path("/api/tags"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "models": [{"name": "test", "details": {"family": "koboldcpp"}}]
                })))
                .mount(&mock)
                .await;
        }
        let response = client
            .post(format!("http://{}/api/endpoints", server.addr()))
            .bearer_auth(&admin_key)
            .json(&json!({"name": format!("Issue887-{engine}"), "base_url": mock.uri()}))
            .send()
            .await
            .unwrap();
        let status = response.status();
        let body: Value = response.json().await.unwrap();
        assert_eq!(status.as_u16(), 201, "{engine}: {body}");
        assert_eq!(body["endpoint_type"], engine);
        let endpoint_id = uuid::Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
        let reloaded = llmlb::db::endpoints::get_endpoint(&db_pool, endpoint_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reloaded.endpoint_type.as_str(), engine);
        mocks.push(mock);
    }
}

/// US6-シナリオ1: オフラインエンドポイント登録はエラーを返す
/// NOTE: 到達不能なエンドポイントは BAD_GATEWAY で拒否される
#[tokio::test]
async fn test_endpoint_type_auto_detection_offline() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // エンドポイント登録（接続先がないのでエラーになる）
    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Unreachable Endpoint",
            "base_url": "http://localhost:9999"
        }))
        .send()
        .await
        .expect("registration request failed");

    // 到達不能なエンドポイントは BAD_GATEWAY で拒否される
    assert_eq!(response.status().as_u16(), 502);
}

/// US6-シナリオ2: 判別の優先順位（xLLM > Ollama > vLLM > OpenAI互換）
/// NOTE: モックサーバーを使用したテストが必要（実際の判別ロジック検証）
#[tokio::test]
async fn test_endpoint_type_detection_priority() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/system"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "xllm_version": "0.1.0"
        })))
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": []
        })))
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("server", "vllm")
                .set_body_json(json!({"object": "list", "data": []})),
        )
        .mount(&mock)
        .await;

    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Priority Endpoint",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .expect("registration request failed");

    assert_eq!(response.status().as_u16(), 201);
    let body: Value = response.json().await.unwrap();
    assert_eq!(
        body["endpoint_type"], "xllm",
        "xLLM should win priority over other detections"
    );
}

/// US6-シナリオ3: xLLM判別（/api/systemエンドポイント）
#[tokio::test]
async fn test_endpoint_type_detection_xllm() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/system"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "xllm_version": "0.1.0"
        })))
        .mount(&mock)
        .await;

    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "xLLM Endpoint",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .expect("registration request failed");

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["endpoint_type"], "xllm");
}

/// US6-シナリオ4: Ollama判別（/api/tagsエンドポイント）
#[tokio::test]
async fn test_endpoint_type_detection_ollama() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/system"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": []
        })))
        .mount(&mock)
        .await;

    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Ollama Endpoint",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .expect("registration request failed");

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["endpoint_type"], "ollama");
}

/// US6-シナリオ5: vLLM判別（Serverヘッダー）
#[tokio::test]
async fn test_endpoint_type_detection_vllm() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/system"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("server", "vllm")
                .set_body_json(json!({"object": "list", "data": []})),
        )
        .mount(&mock)
        .await;

    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "vLLM Endpoint",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .expect("registration request failed");

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["endpoint_type"], "vllm");
}

/// US6-シナリオ5b: llama.cpp判別（Server header `llama.cpp/...`）
///
/// SPEC #575 US-013-A (2026-04-23 delta): UI 補完作業に併せて
/// llama.cpp 検出経路の登録 API レベル統合テストを追加する。
#[tokio::test]
async fn test_endpoint_type_detection_llamacpp_via_server_header() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/system"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/models"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("server", "llama.cpp/0.3.0")
                .set_body_json(json!({"object": "list", "data": []})),
        )
        .mount(&mock)
        .await;

    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "llama.cpp Endpoint via Server header",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .expect("registration request failed");

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["endpoint_type"], "llamacpp");
}

/// Issue #887: 存在しない llama.cpp /v1/version 判定を使用しない
#[tokio::test]
async fn test_endpoint_type_detection_ignores_removed_llamacpp_version_endpoint() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/system"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/models"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    // /v1/models は Server header なしで OK 応答（vLLM とも判定されない）
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"object": "list", "data": []})),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "server": "llama.cpp",
            "version": "0.3.0"
        })))
        .mount(&mock)
        .await;

    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "llama.cpp Endpoint via /v1/version",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .expect("registration request failed");

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["endpoint_type"], "openai_compatible");
}

/// US6-シナリオ6: LM Studio判別（/v1/models owned_byフォールバック）
#[tokio::test]
async fn test_endpoint_type_detection_lm_studio_owned_by_fallback() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/system"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    // /api/v1/models が使えない環境を想定し、/v1/models の owned_by で判定する
    Mock::given(method("GET"))
        .and(path("/api/v1/models"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{"id": "meta-llama-3.1-8b-instruct", "object": "model", "owned_by": "lmstudio-community"}]
        })))
        .mount(&mock)
        .await;

    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "LM Studio OwnedBy Endpoint",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .expect("registration request failed");

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["endpoint_type"], "lm_studio");
}

/// US6-シナリオ6b: LM Studio判別（/api/v1/models 新形式: models[]）
#[tokio::test]
async fn test_endpoint_type_detection_lm_studio_api_models_shape() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/system"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{
                "type": "llm",
                "publisher": "openai",
                "key": "openai/gpt-oss-20b",
                "display_name": "GPT-OSS 20B",
                "architecture": "gpt_oss",
                "loaded_instances": [],
                "max_context_length": 131072,
                "format": "mlx"
            }]
        })))
        .mount(&mock)
        .await;
    // /v1/models の owned_by だけでは LM Studio 判別できないケース
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{"id": "openai/gpt-oss-20b", "object": "model", "owned_by": "organization_owner"}]
        })))
        .mount(&mock)
        .await;

    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "LM Studio API Models Endpoint",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .expect("registration request failed");

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["endpoint_type"], "lm_studio");
}

/// US6-シナリオ6c: /api/tags が 200 + error payload でも LM Studio を優先判定
#[tokio::test]
async fn test_endpoint_type_detection_lm_studio_when_api_tags_returns_error_payload() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/system"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    // LM Studio 実環境で見られる 200 + {"error": "..."} を再現
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "error": "Unexpected endpoint or method. (GET /api/tags)"
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{
                "type": "llm",
                "publisher": "openai",
                "key": "openai/gpt-oss-20b",
                "display_name": "GPT-OSS 20B",
                "architecture": "gpt_oss",
                "loaded_instances": [{"id":"openai/gpt-oss-20b","config":{"context_length":131072}}],
                "max_context_length": 131072,
                "format": "mlx"
            }]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{"id":"openai/gpt-oss-20b","object":"model","owned_by":"organization_owner"}]
        })))
        .mount(&mock)
        .await;

    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "LM Studio Tags Error Payload Endpoint",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .expect("registration request failed");

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["endpoint_type"], "lm_studio");
}

/// US6-シナリオ7: OpenAI互換判別（フォールバック）
#[tokio::test]
async fn test_endpoint_type_detection_openai_compatible() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/system"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{"id": "test-model", "object": "model"}]
        })))
        .mount(&mock)
        .await;

    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "OpenAI Compatible Endpoint",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .expect("registration request failed");

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["endpoint_type"], "openai_compatible");
}

/// US6-シナリオ8: オンライン復帰時のタイプ再判別
#[tokio::test]
async fn test_endpoint_type_redetection_on_online() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/system"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/health"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{"id": "test-model", "object": "model"}]
        })))
        .mount(&mock)
        .await;

    // モックサーバーをbase_urlにして登録（OpenAI互換として判定される）
    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Test Endpoint",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .expect("registration request failed");

    assert_eq!(response.status().as_u16(), 201);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["endpoint_type"], "openai_compatible");

    let endpoint_id = body["id"].as_str().expect("endpoint id");
    let endpoint_uuid = uuid::Uuid::parse_str(endpoint_id).expect("endpoint uuid");

    let registry = EndpointRegistry::new(db_pool.clone())
        .await
        .expect("endpoint registry init");
    let checker = EndpointHealthChecker::new(registry);
    let endpoint = llmlb::db::endpoints::get_endpoint(&db_pool, endpoint_uuid)
        .await
        .expect("get endpoint")
        .expect("endpoint exists");

    checker
        .check_endpoint(&endpoint)
        .await
        .expect("health check should succeed");

    let updated = llmlb::db::endpoints::get_endpoint(&db_pool, endpoint_uuid)
        .await
        .expect("get endpoint")
        .expect("endpoint exists");
    assert_eq!(updated.endpoint_type.as_str(), "openai_compatible");
}

/// #886: llama.cpp-compatible APIs must not hide Strata / Project Maya identity.
#[tokio::test]
async fn test_health_detection_distinguishes_strata_maya_llamacpp_and_openai() {
    let client = Client::new();
    for (health, server_header, expected) in [
        (
            family_health("strata", "auth-disabled-no-bearer"),
            "llama.cpp",
            "strata",
        ),
        (
            family_health("project-maya/source-derived", "auth-disabled-no-bearer"),
            "llama.cpp",
            "project_maya",
        ),
        (json!({"status": "ok"}), "llama.cpp", "llamacpp"),
        (json!({"status": "ok"}), "generic", "openai_compatible"),
    ] {
        let mock = MockServer::start().await;
        let prefix = format!("/{}", uuid::Uuid::new_v4());
        Mock::given(method("GET"))
            .and(path(format!("{prefix}/health")))
            .respond_with(ResponseTemplate::new(200).set_body_json(health))
            .mount(&mock)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("{prefix}/v1/models")))
            .respond_with(
                ResponseTemplate::new(200)
                    .append_header("server", server_header)
                    .set_body_json(json!({"object": "list", "data": []})),
            )
            .mount(&mock)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("{prefix}/props")))
            .respond_with(ResponseTemplate::new(401))
            .mount(&mock)
            .await;
        let result = llmlb::detection::detect_endpoint_type_with_client(
            &client,
            &format!("{}{prefix}", mock.uri()),
            None,
        )
        .await
        .unwrap();
        assert_eq!(result.endpoint_type.as_str(), expected);
        if matches!(expected, "strata" | "project_maya") {
            let requests = mock.received_requests().await.unwrap();
            assert!(!requests.iter().any(|r| r.url.path().ends_with("/props")));
        }
    }
}

/// #886: only the unauthenticated health response is needed, even with an API key.
#[tokio::test]
async fn test_strata_and_maya_detection_uses_only_health() {
    for (health, expected) in [
        (
            family_health("strata", "auth-enabled-valid-bearer"),
            "strata",
        ),
        (
            family_health("project-maya/source-derived", "auth-enabled-valid-bearer"),
            "project_maya",
        ),
        (
            family_health("project-maya/source-derived", "mcp-enabled"),
            "project_maya",
        ),
        (
            // Synthetic collision: the Maya marker must win over service=strata.
            json!({"service": "strata", "mcp_tools": true}),
            "project_maya",
        ),
    ] {
        let mock = MockServer::start().await;
        let prefix = format!("/{}", uuid::Uuid::new_v4());
        Mock::given(method("GET"))
            .and(path(format!("{prefix}/health")))
            .respond_with(ResponseTemplate::new(200).set_body_json(health))
            .mount(&mock)
            .await;
        let result = llmlb::detection::detect_endpoint_type_with_client(
            &Client::new(),
            &format!("{}{prefix}/", mock.uri()),
            Some("test-key"),
        )
        .await
        .unwrap();
        assert_eq!(result.endpoint_type.as_str(), expected);
        let requests = mock.received_requests().await.unwrap();
        let health_request = requests
            .iter()
            .find(|r| r.url.path() == format!("{prefix}/health"))
            .unwrap();
        assert!(!health_request.headers.contains_key("authorization"));
        assert!(!requests.iter().any(|r| r.url.path().ends_with("/props")));
    }
}

/// #886: missing, invalid and unsuccessful health responses keep the generic fallback.
#[tokio::test]
async fn test_strata_health_invalid_signals_fall_back_to_openai() {
    for (status, body) in [
        (200, r#"{"service":"Strata"}"#),
        (200, r#"{"service":"other"}"#),
        (200, r#"{"service":42}"#),
        (200, "not json"),
        (401, r#"{"service":"strata","mcp_tools":true}"#),
        (503, r#"{"service":"strata"}"#),
    ] {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(status).set_body_string(body))
            .mount(&mock)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": []})))
            .mount(&mock)
            .await;
        let result =
            llmlb::detection::detect_endpoint_type_with_client(&Client::new(), &mock.uri(), None)
                .await
                .unwrap();
        assert_eq!(result.endpoint_type.as_str(), "openai_compatible");
    }
}

/// #908: replay real Strata responses and explicitly source-derived Maya fixtures.
#[tokio::test]
async fn test_strata_captured_and_maya_source_derived_http_responses() {
    for (engine, expected, valid_key) in [
        ("strata", "strata", "llmlb-issue-908-strata-test-key"),
        (
            "project-maya/source-derived",
            "project_maya",
            "sk_llmlb908_maya_test_only",
        ),
    ] {
        for (case, api_key) in [
            ("auth-disabled-no-bearer", None),
            ("auth-enabled-no-bearer", None),
            ("auth-enabled-valid-bearer", Some(valid_key)),
            ("auth-enabled-invalid-bearer", Some("wrong-fixture-key")),
        ] {
            let mock = MockServer::start().await;
            let prefix = format!("/{}", uuid::Uuid::new_v4());
            for probe in ["/health", "/v1/models", "/props"] {
                let needs_auth = case != "auth-disabled-no-bearer" && probe != "/health";
                let response = family_response(engine, case, probe);
                let authorized = family_response(engine, "auth-enabled-valid-bearer", probe);
                let denied = family_response(engine, "auth-enabled-invalid-bearer", probe);
                let valid_bearer = format!("Bearer {valid_key}");
                Mock::given(method("GET"))
                    .and(path(format!("{prefix}{probe}")))
                    .respond_with(move |request: &wiremock::Request| {
                        if !needs_auth {
                            response.clone()
                        } else if request
                            .headers
                            .get("authorization")
                            .and_then(|value| value.to_str().ok())
                            == Some(valid_bearer.as_str())
                        {
                            authorized.clone()
                        } else {
                            denied.clone()
                        }
                    })
                    .mount(&mock)
                    .await;
            }
            let result = llmlb::detection::detect_endpoint_type_with_client(
                &Client::new(),
                &format!("{}{prefix}", mock.uri()),
                api_key,
            )
            .await
            .unwrap();
            assert_eq!(result.endpoint_type.as_str(), expected, "{engine}: {case}");
            let requests = mock.received_requests().await.unwrap();
            let health = requests
                .iter()
                .find(|request| request.url.path() == format!("{prefix}/health"))
                .unwrap();
            assert!(!health.headers.contains_key("authorization"), "{case}");
            let models = requests
                .iter()
                .find(|request| request.url.path() == format!("{prefix}/v1/models"))
                .unwrap();
            let expected_bearer = api_key.map(|key| format!("Bearer {key}"));
            assert_eq!(
                models
                    .headers
                    .get("authorization")
                    .map(|v| v.to_str().unwrap()),
                expected_bearer.as_deref(),
                "{case}"
            );
            assert!(!requests.iter().any(|r| r.url.path().ends_with("/props")));
            if case == "auth-enabled-valid-bearer" {
                for probe in ["/v1/models", "/props"] {
                    for (key, response_case) in [
                        (Some(valid_key), "auth-enabled-valid-bearer"),
                        (Some("wrong-fixture-key"), "auth-enabled-invalid-bearer"),
                        (None, "auth-enabled-no-bearer"),
                    ] {
                        let mut request =
                            Client::new().get(format!("{}{prefix}{probe}", mock.uri()));
                        if let Some(key) = key {
                            request = request.bearer_auth(key);
                        }
                        let response = request.send().await.unwrap();
                        let (status, body, _) = family_fixture(engine, response_case, probe);
                        assert_eq!(response.status().as_u16(), status, "{engine}: {probe}");
                        assert_eq!(response.text().await.unwrap(), body, "{engine}: {probe}");
                    }
                }
            }
        }
    }
}

/// #886: registration and DB reads preserve the new endpoint types.
#[tokio::test]
async fn test_strata_and_maya_registration_persists_detected_type() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;
    // Keep registered servers alive so wiremock cannot reuse a registered URL.
    let mut mocks = Vec::new();
    for (health, expected) in [
        (family_health("strata", "auth-disabled-no-bearer"), "strata"),
        (
            family_health("project-maya/source-derived", "auth-disabled-no-bearer"),
            "project_maya",
        ),
    ] {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200).set_body_json(health))
            .mount(&mock)
            .await;
        let engine = if expected == "strata" {
            "strata"
        } else {
            "project-maya/source-derived"
        };
        let models: Value = serde_json::from_str(
            &family_fixture(engine, "auth-disabled-no-bearer", "/v1/models").1,
        )
        .unwrap();
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(models))
            .mount(&mock)
            .await;
        let response = client
            .post(format!("http://{}/api/endpoints", server.addr()))
            .bearer_auth(&admin_key)
            .json(&json!({"name": expected, "base_url": mock.uri()}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 201);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["endpoint_type"], expected);
        let id = uuid::Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
        let stored = llmlb::db::endpoints::get_endpoint(&db_pool, id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.endpoint_type.as_str(), expected);
        assert_eq!(stored.inference_timeout_secs, 120);
        assert!(!stored.endpoint_type.supports_model_download());
        assert!(!stored.endpoint_type.supports_model_delete());
        assert!(!stored.endpoint_type.supports_model_metadata());
        mocks.push(mock);
    }
}
