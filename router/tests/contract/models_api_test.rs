//! モデル管理API契約テスト
//!
//! TDD RED: これらのテストは実装前に失敗する必要があります

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use llm_router::{api, balancer::LoadManager, registry::NodeRegistry, AppState};
use serde_json::json;
use serial_test::serial;
use std::io::Write;
use tempfile::NamedTempFile;
use tokio::time::{sleep, Duration};
use tower::ServiceExt;
use uuid::Uuid;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn build_app() -> Router {
    // テスト用に一時ディレクトリを設定
    let temp_dir = std::env::temp_dir().join(format!(
        "or-test-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    std::env::set_var("LLM_ROUTER_DATA_DIR", &temp_dir);
    // router_models_dir は HOME/USERPROFILE を見るためテスト用に上書き
    std::env::set_var("HOME", &temp_dir);
    std::env::set_var("USERPROFILE", &temp_dir);
    // テスト用の軽量変換スクリプトを配置（依存ライブラリ不要）
    let mock_script_path = temp_dir.join("mock_gguf_writer.py");
    std::fs::write(
        &mock_script_path,
        r#"import sys, pathlib
outfile = sys.argv[sys.argv.index("--outfile")+1]
pathlib.Path(outfile).parent.mkdir(parents=True, exist_ok=True)
with open(outfile, "wb") as f:
    f.write(b"gguf test")
"#,
    )
    .unwrap();
    std::env::set_var("LLM_CONVERT_SCRIPT", &mock_script_path);
    // python依存チェック用にローカルの.venvがあれば優先
    if let Ok(cwd) = std::env::current_dir() {
        let candidate = cwd.join(".venv/bin/python3");
        if candidate.exists() {
            std::env::set_var("LLM_CONVERT_PYTHON", candidate);
        }
    }
    // 変換スクリプトは各テストで個別に指定する
    llm_router::api::models::clear_registered_models();
    llm_router::api::models::clear_hf_cache();

    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let request_history =
        std::sync::Arc::new(llm_router::db::request_history::RequestHistoryStorage::new().unwrap());
    let task_manager = llm_router::tasks::DownloadTaskManager::new();
    let convert_manager = llm_router::convert::ConvertTaskManager::new(1);
    let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run migrations");
    let jwt_secret = "test-secret".to_string();
    let state = AppState {
        registry,
        load_manager,
        request_history,
        task_manager,
        convert_manager,
        db_pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
    };

    api::create_router(state)
}

/// T005b: POST /api/models/distribute のバリデーション（specificでnode_ids空）
#[tokio::test]
#[serial]
async fn test_distribute_models_requires_node_ids_for_specific() {
    std::env::set_var("LLM_ROUTER_SKIP_HEALTH_CHECK", "1");
    let app = build_app().await;

    let request_body = json!({
        "model_name": "gpt-oss:20b",
        "target": "specific",
        "node_ids": []
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/models/distribute")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "specific target requires node_ids"
    );
}

/// T004: GET /api/models/available の契約テスト
#[tokio::test]
#[serial]
async fn test_get_available_models_contract() {
    std::env::set_var("LLM_ROUTER_SKIP_HEALTH_CHECK", "1");
    let mock = MockServer::start().await;
    // HF mock responds once with gguf list
    Mock::given(method("GET"))
        .and(path("/api/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(vec![json!({
            "modelId": "test/repo",
            "tags": ["gguf"],
            "siblings": [{"rfilename": "model.gguf", "size": 1234}],
            "lastModified": "2024-01-01T00:00:00Z"
        })]))
        .mount(&mock)
        .await;
    std::env::set_var("HF_BASE_URL", mock.uri());

    let app = build_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/models/available?source=hf")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // ステータスコードの検証
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Expected 200 OK for GET /api/models/available"
    );

    // レスポンスボディの検証
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // スキーマ検証
    assert!(
        body.get("models").is_some(),
        "Response must have 'models' field"
    );
    assert!(body["models"].is_array(), "'models' field must be an array");

    // source フィールドが存在することを確認
    assert!(
        body.get("source").is_some(),
        "Response must have 'source' field"
    );
    let source = body["source"].as_str().expect("'source' must be a string");
    assert!(
        ["builtin", "nodes", "hf"].contains(&source),
        "'source' must be 'builtin', 'nodes', or 'hf'"
    );

    // HFモックが返した1件が含まれること
    let models = body["models"]
        .as_array()
        .expect("'models' must be an array");
    assert!(
        models
            .iter()
            .any(|m| m["name"] == "hf/test/repo/model.gguf"),
        "hf catalog item should appear"
    );

    // models配列の各要素の検証
    if let Some(models) = body["models"].as_array() {
        for model in models {
            assert!(model.get("name").is_some(), "Model must have 'name'");
            assert!(model.get("size_gb").is_some(), "Model must have 'size_gb'");
            assert!(
                model.get("description").is_some(),
                "Model must have 'description'"
            );
            assert!(
                model.get("required_memory_gb").is_some(),
                "Model must have 'required_memory_gb'"
            );
            assert!(model.get("tags").is_some(), "Model must have 'tags'");
            assert!(model["tags"].is_array(), "'tags' must be an array");
        }
    }
}

/// T005: POST /api/models/distribute の契約テスト
#[tokio::test]
#[serial]
async fn test_distribute_models_contract() {
    std::env::set_var("LLM_ROUTER_SKIP_HEALTH_CHECK", "1");
    let app = build_app().await;

    // テスト用リクエスト
    let request_body = json!({
        "model_name": "gpt-oss:20b",
        "target": "all",
        "node_ids": []
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/models/distribute")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // ステータスコードの検証
    assert_eq!(
        response.status(),
        StatusCode::ACCEPTED,
        "Expected 202 ACCEPTED for POST /api/models/distribute"
    );

    // レスポンスボディの検証
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // スキーマ検証
    assert!(
        body.get("task_ids").is_some(),
        "Response must have 'task_ids' field"
    );
    assert!(
        body["task_ids"].is_array(),
        "'task_ids' field must be an array"
    );

    // task_ids配列の各要素がUUID文字列であることを確認
    if let Some(task_ids) = body["task_ids"].as_array() {
        for task_id in task_ids {
            let task_id_str = task_id.as_str().expect("task_id must be a string");
            Uuid::parse_str(task_id_str).expect("task_id must be a valid UUID");
        }
    }
}

/// T006: GET /api/nodes/{node_id}/models の契約テスト
#[tokio::test]
#[serial]
async fn test_get_agent_models_contract() {
    std::env::set_var("LLM_ROUTER_SKIP_HEALTH_CHECK", "1");
    let app = build_app().await;

    // テスト用のノードを登録
    let register_payload = json!({
        "machine_name": "test-node",
        "ip_address": "127.0.0.1",
        "runtime_version": "0.1.0",
        "runtime_port": 11434,
        "gpu_available": true,
        "gpu_devices": [
            {"model": "Test GPU", "count": 1}
        ]
    });

    let register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/nodes")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&register_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(register_response.status(), StatusCode::CREATED);

    // ノードIDを取得
    let body = to_bytes(register_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let node: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let node_id = node["node_id"]
        .as_str()
        .expect("Node must have 'node_id' field");

    // モデル一覧を取得
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/nodes/{}/models", node_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // ステータスコードの検証
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Expected 200 OK for GET /api/nodes/:id/models"
    );

    // レスポンスボディの検証（InstalledModelの配列）
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(body.is_array(), "Response must be an array");

    // 配列の各要素の検証
    if let Some(models) = body.as_array() {
        for model in models {
            assert!(model.get("name").is_some(), "Model must have 'name'");
            assert!(model.get("size").is_some(), "Model must have 'size'");
            assert!(
                model.get("installed_at").is_some(),
                "Model must have 'installed_at'"
            );
            // digestはオプション
        }
    }
}

/// T007: POST /api/nodes/{node_id}/models/pull の契約テスト
#[tokio::test]
#[serial]
async fn test_pull_model_contract() {
    std::env::set_var("LLM_ROUTER_SKIP_HEALTH_CHECK", "1");
    let app = build_app().await;

    // テスト用のノードを登録
    let register_payload = json!({
        "machine_name": "test-node",
        "ip_address": "127.0.0.1",
        "runtime_version": "0.1.0",
        "runtime_port": 11434,
        "gpu_available": true,
        "gpu_devices": [
            {"model": "Test GPU", "count": 1}
        ]
    });

    let register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/nodes")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&register_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(register_response.status(), StatusCode::CREATED);

    // ノードIDを取得
    let body = to_bytes(register_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let node: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let node_id = node["node_id"]
        .as_str()
        .expect("Node must have 'node_id' field");

    // モデルプル
    let request_body = json!({
        "model_name": "gpt-oss:3b"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/nodes/{}/models/pull", node_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // ステータスコードの検証
    assert_eq!(
        response.status(),
        StatusCode::ACCEPTED,
        "Expected 202 ACCEPTED for POST /api/nodes/:id/models/pull"
    );

    // レスポンスボディの検証
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // スキーマ検証
    assert!(
        body.get("task_id").is_some(),
        "Response must have 'task_id' field"
    );
    let task_id_str = body["task_id"]
        .as_str()
        .expect("'task_id' must be a string");
    Uuid::parse_str(task_id_str).expect("'task_id' must be a valid UUID");
}

/// T008: GET /api/tasks/{task_id} の契約テスト
#[tokio::test]
#[serial]
async fn test_get_task_progress_contract() {
    std::env::set_var("LLM_ROUTER_SKIP_HEALTH_CHECK", "1");
    let app = build_app().await;

    // テスト用のノードを登録
    let register_payload = json!({
        "machine_name": "test-node",
        "ip_address": "127.0.0.1",
        "runtime_version": "0.1.0",
        "runtime_port": 11434,
        "gpu_available": true,
        "gpu_devices": [
            {"model": "Test GPU", "count": 1}
        ]
    });

    let register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/nodes")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&register_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(register_response.status(), StatusCode::CREATED);

    // ノードIDを取得
    let body = to_bytes(register_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let node: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let node_id = node["node_id"]
        .as_str()
        .expect("Node must have 'node_id' field");

    // モデルプルを開始してタスクIDを取得
    let request_body = json!({
        "model_name": "gpt-oss:3b"
    });

    let pull_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/nodes/{}/models/pull", node_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = to_bytes(pull_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let pull_result: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let task_id = pull_result["task_id"]
        .as_str()
        .expect("Pull response must have 'task_id'");

    // タスク進捗を取得
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/tasks/{}", task_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // ステータスコードの検証
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Expected 200 OK for GET /api/tasks/:id"
    );

    // レスポンスボディの検証（DownloadTask構造体）
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // スキーマ検証
    assert!(body.get("id").is_some(), "Task must have 'id'");
    assert!(body.get("node_id").is_some(), "Task must have 'node_id'");
    assert!(
        body.get("model_name").is_some(),
        "Task must have 'model_name'"
    );
    assert!(body.get("status").is_some(), "Task must have 'status'");
    assert!(body.get("progress").is_some(), "Task must have 'progress'");
    assert!(
        body.get("started_at").is_some(),
        "Task must have 'started_at'"
    );

    // statusフィールドの検証
    let status = body["status"].as_str().expect("'status' must be a string");
    assert!(
        ["pending", "in_progress", "completed", "failed"].contains(&status),
        "'status' must be one of: pending, in_progress, completed, failed"
    );

    // progressフィールドの検証（0.0-1.0の範囲）
    let progress = body["progress"]
        .as_f64()
        .expect("'progress' must be a number");
    assert!(
        (0.0..=1.0).contains(&progress),
        "'progress' must be between 0.0 and 1.0"
    );

    // UUIDの検証
    let id_str = body["id"].as_str().expect("'id' must be a string");
    Uuid::parse_str(id_str).expect("'id' must be a valid UUID");

    let node_id_str = body["node_id"]
        .as_str()
        .expect("'node_id' must be a string");
    Uuid::parse_str(node_id_str).expect("'node_id' must be a valid UUID");
}

/// T009: POST /api/models/register - 正常系と重複/404異常系
#[tokio::test]
#[serial]
async fn test_register_model_contract() {
    std::env::set_var("LLM_ROUTER_SKIP_HEALTH_CHECK", "1");
    std::env::set_var("LLM_ROUTER_SKIP_API_KEY", "1");
    let mock = MockServer::start().await;

    // HEAD 200 for existence
    Mock::given(method("HEAD"))
        .and(path("/test/repo/resolve/main/model.gguf"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock)
        .await;

    std::env::set_var("HF_BASE_URL", mock.uri());

    let app = build_app().await;

    // 正常登録
    let payload = json!({
        "repo": "test/repo",
        "filename": "model.gguf",
        "display_name": "Test Model"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/models/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    // /v1/models に含まれること
    let models_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/models")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(models_res.status(), StatusCode::OK);
    let body = to_bytes(models_res.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let data = body["data"]
        .as_array()
        .expect("'data' must be an array on /v1/models");
    assert!(
        data.iter().all(|m| m["id"] != "hf/test/repo/model.gguf"),
        "/v1/models must not expose models before download completes"
    );

    // 重複登録は400
    let dup = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/models/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(dup.status(), StatusCode::BAD_REQUEST);

    // 404ケース: HEADが404を返す
    Mock::given(method("HEAD"))
        .and(path("/missing/repo/resolve/main/absent.gguf"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;

    let missing_payload = json!({
        "repo": "missing/repo",
        "filename": "absent.gguf"
    });

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/models/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&missing_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::BAD_REQUEST);

    // repoのみ指定でGGUFなし→最初の変換可能ファイルを選んでpending_conversionになる
    Mock::given(method("GET"))
        .and(path("/api/models/non-gguf-repo"))
        .and(query_param("expand", "siblings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "siblings": [
                {"rfilename": "model.safetensors"}
            ]
        })))
        .mount(&mock)
        .await;

    Mock::given(method("HEAD"))
        .and(path("/non-gguf-repo/resolve/main/model.safetensors"))
        .respond_with(ResponseTemplate::new(200).append_header("content-length", "100"))
        .mount(&mock)
        .await;

    let repo_only = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/models/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"repo": "non-gguf-repo"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(repo_only.status(), StatusCode::CREATED);
    let repo_body = to_bytes(repo_only.into_body(), usize::MAX).await.unwrap();
    let repo_json: serde_json::Value = serde_json::from_slice(&repo_body).unwrap();
    assert_eq!(repo_json["status"], "pending_conversion");

    // repoのみ、変換可能拡張子もない → 最初のファイルを拾ってpending_conversion
    Mock::given(method("GET"))
        .and(path("/api/models/unknown-repo"))
        .and(query_param("expand", "siblings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "siblings": [
                {"rfilename": "config.json"},
                {"rfilename": "other.txt"}
            ]
        })))
        .mount(&mock)
        .await;

    Mock::given(method("HEAD"))
        .and(path("/unknown-repo/resolve/main/config.json"))
        .respond_with(ResponseTemplate::new(200).append_header("content-length", "50"))
        .mount(&mock)
        .await;

    let repo_only_fallback = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/models/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"repo": "unknown-repo"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(repo_only_fallback.status(), StatusCode::CREATED);
    let repo_body_fb = to_bytes(repo_only_fallback.into_body(), usize::MAX)
        .await
        .unwrap();
    let repo_json_fb: serde_json::Value = serde_json::from_slice(&repo_body_fb).unwrap();
    assert_eq!(repo_json_fb["status"], "pending_conversion");

    // DELETE removes registered model
    let delete_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/models/hf/test/repo/model.gguf")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_res.status(), StatusCode::NO_CONTENT);

    let models_after = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/models")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body_after = to_bytes(models_after.into_body(), usize::MAX)
        .await
        .unwrap();
    let val_after: serde_json::Value = serde_json::from_slice(&body_after).unwrap();
    let data_after = val_after["data"].as_array().unwrap();
    assert!(
        !data_after
            .iter()
            .any(|m| m["id"] == "hf/test/repo/model.gguf"),
        "deleted model must disappear from /v1/models"
    );

    // 非GGUFでも変換完了後に /v1/models に出ること（LLM_CONVERT_FAKE=1でダミー生成）
    let app_for_convert = build_app().await;
    std::env::set_var("HF_BASE_URL", mock.uri());
    Mock::given(method("GET"))
        .and(path("/api/models/convertible-repo"))
        .and(query_param("expand", "siblings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "siblings": [
                {"rfilename": "model.bin"}
            ]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("HEAD"))
        .and(path("/convertible-repo/resolve/main/model.bin"))
        .respond_with(ResponseTemplate::new(200).append_header("content-length", "123"))
        .mount(&mock)
        .await;

    let reg_convert = app_for_convert
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/models/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"repo": "convertible-repo"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reg_convert.status(), StatusCode::CREATED);

    let mut converted = false;
    for _ in 0..25 {
        let resp = app_for_convert
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/models")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let val: serde_json::Value = serde_json::from_slice(&body).unwrap();
        if val["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .any(|m| m["id"] == "hf/convertible-repo/model.bin")
            })
            .unwrap_or(false)
        {
            converted = true;
            break;
        }
        sleep(Duration::from_millis(100)).await;
    }
    assert!(converted, "converted model should appear in /v1/models");
}

/// T010: convert 失敗タスクを Restore で再キューできること
#[tokio::test]
#[serial]
async fn test_convert_restore_requeues() {
    std::env::set_var("LLM_ROUTER_SKIP_HEALTH_CHECK", "1");
    std::env::set_var("LLM_ROUTER_SKIP_API_KEY", "1");
    // 1回目は意図的に失敗させる（exit 1 の簡易スクリプト）
    let mut fail_script = NamedTempFile::new().unwrap();
    writeln!(fail_script, "import sys; sys.exit(1)").unwrap();
    std::env::set_var("LLM_CONVERT_FAKE", "0");
    std::env::set_var("LLM_CONVERT_SCRIPT", fail_script.path());

    let mock = MockServer::start().await;
    std::env::set_var("HF_BASE_URL", mock.uri());

    // siblings returns non-gguf file
    Mock::given(method("GET"))
        .and(path("/api/models/restore-repo"))
        .and(query_param("expand", "siblings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "siblings": [
                {"rfilename": "model.bin"}
            ]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("HEAD"))
        .and(path("/restore-repo/resolve/main/model.bin"))
        .respond_with(ResponseTemplate::new(200).append_header("content-length", "42"))
        .mount(&mock)
        .await;

    let app = build_app().await;

    // 事前に成功用スクリプトを控えておく
    let good_script = std::env::var("LLM_CONVERT_SCRIPT").unwrap();

    // convert を失敗させるために存在しないスクリプトを指定
    std::env::set_var("LLM_CONVERT_SCRIPT", "/nonexistent/convert_hf_to_gguf.py");

    // register -> convert fails
    let reg = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/models/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"repo": "restore-repo"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reg.status(), StatusCode::CREATED);

    // wait for failed task
    let mut failed_seen = false;
    let mut last_tasks = serde_json::Value::Null;
    for _ in 0..60 {
        let tasks_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/models/convert")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(tasks_resp.status(), StatusCode::OK);
        let body = to_bytes(tasks_resp.into_body(), usize::MAX).await.unwrap();
        let tasks: serde_json::Value = serde_json::from_slice(&body).unwrap();
        last_tasks = tasks.clone();
        if tasks
            .as_array()
            .map(|arr| {
                arr.iter()
                    .any(|t| t["repo"] == "restore-repo" && t["status"] == "failed")
            })
            .unwrap_or(false)
        {
            failed_seen = true;
            break;
        }
        sleep(Duration::from_millis(200)).await;
    }
    assert!(
        failed_seen,
        "failed convert task should appear, tasks={:?}",
        last_tasks
    );

    // 2回目はテスト用モックスクリプトで成功させる（依存なしでダミーgguf生成）
    let mut mock_script = NamedTempFile::new().unwrap();
    writeln!(
        mock_script,
        r#"import sys, pathlib
outfile = sys.argv[sys.argv.index("--outfile")+1]
pathlib.Path(outfile).parent.mkdir(parents=True, exist_ok=True)
with open(outfile, "wb") as f:
    f.write(b"gguf test")
"#
    )
    .unwrap();
    let mock_path = mock_script.into_temp_path();
    std::env::set_var("LLM_CONVERT_SCRIPT", &mock_path);

    std::env::set_var("LLM_CONVERT_SCRIPT", good_script);

    let retry = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/models/convert")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "repo": "restore-repo",
                        "filename": "model.bin"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(retry.status(), StatusCode::ACCEPTED);

    // wait for success
    let mut succeeded = false;
    let mut last_tasks_after_retry = serde_json::Value::Null;
    for _ in 0..60 {
        let tasks_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/models/convert")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(tasks_resp.into_body(), usize::MAX).await.unwrap();
        let tasks: serde_json::Value = serde_json::from_slice(&body).unwrap();
        last_tasks_after_retry = tasks.clone();
        if tasks
            .as_array()
            .map(|arr| {
                arr.iter().any(|t| {
                    t["repo"] == "restore-repo"
                        && t["status"] == "completed"
                        && t["path"].is_string()
                })
            })
            .unwrap_or(false)
        {
            succeeded = true;
            break;
        }
        sleep(Duration::from_millis(200)).await;
    }
    assert!(
        succeeded,
        "restore must requeue and complete, tasks={:?}",
        last_tasks_after_retry
    );

    // /v1/models should now include the converted model
    let models_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/models")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(models_res.status(), StatusCode::OK);
    let body = to_bytes(models_res.into_body(), usize::MAX).await.unwrap();
    let val: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let present = val["data"]
        .as_array()
        .map(|arr| arr.iter().any(|m| m["id"] == "hf/restore-repo/model.bin"))
        .unwrap_or(false);
    assert!(present, "/v1/models must include restored model");
}
