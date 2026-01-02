//! モデル管理API契約テスト
//!
//! TDD RED: これらのテストは実装前に失敗する必要があります

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use llm_router::registry::models::{model_name_to_dir, router_models_dir, ModelInfo};
use llm_router::{api, balancer::LoadManager, registry::NodeRegistry, AppState};
use llm_router_common::auth::{ApiKeyScope, UserRole};
use serde_json::{json, Value};
use serial_test::serial;
use tokio::time::{sleep, Duration};
use tower::ServiceExt;
use uuid::Uuid;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct TestApp {
    app: Router,
    db_pool: sqlx::SqlitePool,
    admin_key: String,
}

// Node主導キャッシュのため、registry manifest に外部ソースURLが含まれること
#[tokio::test]
#[serial]
async fn registry_manifest_includes_origin_urls() {
    let TestApp { app, admin_key, .. } = build_app().await;

    let model_name = "openai/gpt-oss-7b";
    let repo = "openai/gpt-oss-7b";
    let filename = "model.Q4_K_M.gguf";
    let expected_url = format!("https://huggingface.co/{}/resolve/main/{}", repo, filename);

    let base = router_models_dir().unwrap();
    let dir = base.join(model_name_to_dir(model_name));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("model.gguf"), b"gguf test").unwrap();

    let mut model = ModelInfo::new(model_name.to_string(), 0, repo.to_string(), 0, vec![]);
    model.tags = vec!["gguf".into()];
    model.repo = Some(repo.to_string());
    model.filename = Some(filename.to_string());
    model.download_url = Some(expected_url.clone());
    model.status = Some("cached".into());
    llm_router::api::models::upsert_registered_model(model);

    let encoded = model_name.replace("/", "%2F");
    let response = app
        .oneshot(
            admin_request(&admin_key)
                .method("GET")
                .uri(format!("/v0/models/registry/{}/manifest.json", encoded))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    let files = v.get("files").and_then(|f| f.as_array()).unwrap();
    let entry = files
        .iter()
        .find(|f| f.get("name").and_then(|n| n.as_str()) == Some("model.gguf"))
        .expect("model.gguf not found in manifest");
    let url = entry.get("url").and_then(|u| u.as_str()).unwrap();
    assert_eq!(url, expected_url);
}

async fn build_app() -> TestApp {
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
    // NOTE: clear_hf_cache() は廃止 - HFカタログは直接参照する方針

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
        llm_router::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let convert_manager = llm_router::convert::ConvertTaskManager::new(1, db_pool.clone());
    let jwt_secret = "test-secret".to_string();
    let state = AppState {
        registry,
        load_manager,
        request_history,
        convert_manager,
        db_pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
        queue_config: llm_router::config::QueueConfig::from_env(),
        event_bus: llm_router::events::create_shared_event_bus(),
    };

    let password_hash = llm_router::auth::password::hash_password("password123").unwrap();
    let admin_user =
        llm_router::db::users::create(&state.db_pool, "admin", &password_hash, UserRole::Admin)
            .await
            .expect("create admin user");
    let admin_key = llm_router::db::api_keys::create(
        &state.db_pool,
        "admin-key",
        admin_user.id,
        None,
        vec![ApiKeyScope::Admin],
    )
    .await
    .expect("create admin api key")
    .key;

    let db_pool = state.db_pool.clone();
    let app = api::create_router(state);
    TestApp {
        app,
        db_pool,
        admin_key,
    }
}

fn admin_request(admin_key: &str) -> axum::http::request::Builder {
    Request::builder().header("authorization", format!("Bearer {}", admin_key))
}

/// モデル配布APIは廃止（ノードが /v1/models と /v0/models/blob から自律取得）
#[tokio::test]
#[serial]
async fn test_distribute_models_endpoint_is_removed() {
    let TestApp { app, admin_key, .. } = build_app().await;

    let request_body = json!({
        "model_name": "gpt-oss-20b",
        "target": "specific",
        "node_ids": []
    });

    let response = app
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri("/v0/models/distribute")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(
        matches!(
            response.status(),
            StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
        ),
        "model distribution endpoint should be removed (got {})",
        response.status()
    );
}

/// T004: GET /v0/models/available は廃止（HFは直接参照する方針）
#[tokio::test]
#[serial]
async fn test_get_available_models_endpoint_is_removed() {
    let TestApp { app, admin_key, .. } = build_app().await;

    let response = app
        .oneshot(
            admin_request(&admin_key)
                .method("GET")
                .uri("/v0/models/available?source=hf")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // エンドポイントは削除済み
    // NOTE: 405 (Method Not Allowed) は /v0/models/*model_name (DELETE用) にマッチするため
    //       404 (Not Found) または 405 のどちらかが返される
    assert!(
        response.status() == StatusCode::NOT_FOUND
            || response.status() == StatusCode::METHOD_NOT_ALLOWED,
        "/v0/models/available GET endpoint should be removed (got {})",
        response.status()
    );
}

/// ノードのモデル一覧取得APIは廃止（ロード済みモデルは /v0/nodes と /v0/dashboard/nodes から参照）
#[tokio::test]
#[serial]
async fn test_get_node_models_endpoint_is_removed() {
    let TestApp { app, admin_key, .. } = build_app().await;

    let response = app
        .oneshot(
            admin_request(&admin_key)
                .method("GET")
                .uri(format!("/v0/nodes/{}/models", Uuid::new_v4()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "node models endpoint should be removed"
    );
}

/// ノードへのモデルpull指示APIは廃止（ノードが自律的に取得）
#[tokio::test]
#[serial]
async fn test_pull_model_to_node_endpoint_is_removed() {
    let TestApp { app, admin_key, .. } = build_app().await;

    let request_body = json!({
        "model_name": "gpt-oss-3b"
    });

    let response = app
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri(format!("/v0/nodes/{}/models/pull", Uuid::new_v4()))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "node model pull endpoint should be removed"
    );
}

/// ダウンロードタスクAPIは廃止（モデル同期はノード側でオンデマンドに実行）
#[tokio::test]
#[serial]
async fn test_tasks_endpoint_is_removed() {
    let TestApp { app, admin_key, .. } = build_app().await;

    let response = app
        .oneshot(
            admin_request(&admin_key)
                .method("GET")
                .uri("/v0/tasks")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "tasks endpoint should be removed"
    );
}

/// T009: POST /v0/models/register - 正常系と重複/404異常系
#[tokio::test]
#[serial]
async fn test_register_model_contract() {
    let mock = MockServer::start().await;

    // siblings: GGUF only
    Mock::given(method("GET"))
        .and(path("/api/models/test/repo"))
        .and(query_param("expand", "siblings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "siblings": [
                {"rfilename": "model.gguf"}
            ]
        })))
        .mount(&mock)
        .await;

    // HEAD 200 for download
    Mock::given(method("HEAD"))
        .and(path("/test/repo/resolve/main/model.gguf"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock)
        .await;

    std::env::set_var("HF_BASE_URL", mock.uri());

    let TestApp { app, admin_key, .. } = build_app().await;

    // 正常登録
    let payload = json!({
        "repo": "test/repo",
        "filename": "model.gguf",
        "display_name": "Test Model"
    });

    let response = app
        .clone()
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri("/v0/models/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    // /v1/models に含まれること（ただし ready=false）
    let models_res = app
        .clone()
        .oneshot(
            admin_request(&admin_key)
                .method("GET")
                .uri("/v1/models")
                .header("x-api-key", "sk_debug")
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
    let entry = data
        .iter()
        .find(|m| m["id"] == "test/repo")
        .expect("/v1/models must include queued model");
    assert_eq!(
        entry["ready"], false,
        "model must not be ready before download completes"
    );

    // 重複登録は400
    let dup = app
        .clone()
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri("/v0/models/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(dup.status(), StatusCode::BAD_REQUEST);

    // 異常系: 指定したGGUFがsiblingsに存在しない
    Mock::given(method("GET"))
        .and(path("/api/models/missing/repo"))
        .and(query_param("expand", "siblings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "siblings": [
                {"rfilename": "other.gguf"}
            ]
        })))
        .mount(&mock)
        .await;

    let missing_payload = json!({
        "repo": "missing/repo",
        "filename": "absent.gguf"
    });

    let missing = app
        .clone()
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri("/v0/models/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&missing_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::BAD_REQUEST);

    // repoのみ指定で safetensors の場合は登録できる（config/tokenizer必須）
    Mock::given(method("GET"))
        .and(path("/api/models/safetensors-repo"))
        .and(query_param("expand", "siblings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "siblings": [
                {"rfilename": "config.json"},
                {"rfilename": "tokenizer.json"},
                {"rfilename": "model.safetensors"}
            ]
        })))
        .mount(&mock)
        .await;

    let safetensors_repo_only = app
        .clone()
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri("/v0/models/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"repo": "safetensors-repo"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(safetensors_repo_only.status(), StatusCode::CREATED);

    // DELETE: タスク完了前でもConvertTaskを削除できる（204を期待）
    // モデル名 = リポジトリ名、ワイルドカードパスなのでスラッシュをそのまま使用
    let delete_res = app
        .clone()
        .oneshot(
            admin_request(&admin_key)
                .method("DELETE")
                .uri("/v0/models/test/repo")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // タスク完了前でもConvertTaskを削除してダウンロードをキャンセルできる
    assert_eq!(delete_res.status(), StatusCode::NO_CONTENT);

    // GGUF登録後に /v1/models に出ること（LLM_CONVERT_FAKE=1でダミー生成）
    let TestApp {
        app: app_for_convert,
        admin_key,
        ..
    } = build_app().await;
    std::env::set_var("HF_BASE_URL", mock.uri());
    Mock::given(method("GET"))
        .and(path("/api/models/convertible-repo"))
        .and(query_param("expand", "siblings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "siblings": [
                {"rfilename": "model.Q4_K_M.gguf"}
            ]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("HEAD"))
        .and(path("/convertible-repo/resolve/main/model.Q4_K_M.gguf"))
        .respond_with(ResponseTemplate::new(200).append_header("content-length", "123"))
        .mount(&mock)
        .await;
    // GETリクエスト（ダウンロード）用のモック - ダミーのGGUFファイルを返す
    Mock::given(method("GET"))
        .and(path("/convertible-repo/resolve/main/model.Q4_K_M.gguf"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"GGUF dummy content"))
        .mount(&mock)
        .await;

    let reg_convert = app_for_convert
        .clone()
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri("/v0/models/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "repo": "convertible-repo",
                        "format": "gguf",
                        "gguf_policy": "quality"
                    }))
                    .unwrap(),
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
                admin_request(&admin_key)
                    .method("GET")
                    .uri("/v1/models")
                    .header("x-api-key", "sk_debug")
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
                    .any(|m| m["id"] == "convertible-repo" && m["ready"] == true)
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

/// T0xx: safetensors登録では config.json + tokenizer.json を必須とする
#[tokio::test]
#[serial]
async fn test_register_safetensors_requires_metadata_files() {
    let mock = MockServer::start().await;
    std::env::set_var("HF_BASE_URL", mock.uri());

    Mock::given(method("GET"))
        .and(path("/api/models/safetensors-missing-meta"))
        .and(query_param("expand", "siblings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "siblings": [
                {"rfilename": "model.safetensors"}
            ]
        })))
        .mount(&mock)
        .await;

    let TestApp { app, admin_key, .. } = build_app().await;

    let payload = json!({
        "repo": "safetensors-missing-meta",
        "format": "safetensors"
    });

    let response = app
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri("/v0/models/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "safetensors register should require config/tokenizer metadata"
    );
}

/// T0yy: 複数の safetensors ファイルがある場合は index.json を要求する
#[tokio::test]
#[serial]
async fn test_register_safetensors_sharded_requires_index_file() {
    let mock = MockServer::start().await;
    std::env::set_var("HF_BASE_URL", mock.uri());

    Mock::given(method("GET"))
        .and(path("/api/models/sharded-safetensors"))
        .and(query_param("expand", "siblings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "siblings": [
                {"rfilename": "config.json"},
                {"rfilename": "tokenizer.json"},
                {"rfilename": "model-00001.safetensors"},
                {"rfilename": "model-00002.safetensors"}
            ]
        })))
        .mount(&mock)
        .await;

    let TestApp { app, admin_key, .. } = build_app().await;

    let payload = json!({
        "repo": "sharded-safetensors",
        "format": "safetensors"
    });

    let response = app
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri("/v0/models/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "sharded safetensors repo should require .safetensors.index.json"
    );
}

/// T010: safetensors と GGUF が両方ある場合、format 未指定は400
#[tokio::test]
#[serial]
async fn test_register_model_requires_format_when_both_artifacts_exist() {
    let mock = MockServer::start().await;
    std::env::set_var("HF_BASE_URL", mock.uri());

    Mock::given(method("GET"))
        .and(path("/api/models/both-repo"))
        .and(query_param("expand", "siblings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "siblings": [
                {"rfilename": "config.json"},
                {"rfilename": "tokenizer.json"},
                {"rfilename": "model.safetensors"},
                {"rfilename": "model.Q4_K_M.gguf"}
            ]
        })))
        .mount(&mock)
        .await;

    let TestApp { app, admin_key, .. } = build_app().await;

    let payload = json!({
        "repo": "both-repo"
    });

    let response = app
        .clone()
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri("/v0/models/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// T011: filename未指定 + gguf_policy 指定でGGUF siblingsから選択する
#[tokio::test]
#[serial]
async fn test_register_model_selects_gguf_by_policy_from_siblings() {
    let mock = MockServer::start().await;
    std::env::set_var("HF_BASE_URL", mock.uri());

    Mock::given(method("GET"))
        .and(path("/api/models/policy-repo"))
        .and(query_param("expand", "siblings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "siblings": [
                {"rfilename": "model.Q4_K_M.gguf"},
                {"rfilename": "model.Q8_0.gguf"},
                {"rfilename": "model.F16.gguf"}
            ]
        })))
        .mount(&mock)
        .await;

    let TestApp { app, admin_key, .. } = build_app().await;

    let payload = json!({
        "repo": "policy-repo",
        "format": "gguf",
        "gguf_policy": "quality"
    });

    let response = app
        .clone()
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri("/v0/models/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["format"], "gguf");
    assert_eq!(body["filename"], "model.F16.gguf");
}

/// T012: format=gguf かつ filename未指定で gguf_policy が無い場合は400
#[tokio::test]
#[serial]
async fn test_register_model_errors_when_gguf_policy_missing() {
    let mock = MockServer::start().await;
    std::env::set_var("HF_BASE_URL", mock.uri());

    Mock::given(method("GET"))
        .and(path("/api/models/no-policy-repo"))
        .and(query_param("expand", "siblings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "siblings": [
                {"rfilename": "model.Q4_K_M.gguf"}
            ]
        })))
        .mount(&mock)
        .await;

    let TestApp { app, admin_key, .. } = build_app().await;

    let payload = json!({
        "repo": "no-policy-repo",
        "format": "gguf"
    });

    let response = app
        .clone()
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri("/v0/models/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// T003: 0Bキャッシュはready扱いにならない
#[tokio::test]
#[serial]
async fn test_zero_byte_cache_is_not_ready() {
    let test_app = build_app().await;
    let app = test_app.app.clone();

    let model_name = "zero-byte-model";
    let base = router_models_dir().expect("router models dir should exist");
    let model_dir = base.join(model_name_to_dir(model_name));
    std::fs::create_dir_all(&model_dir).unwrap();
    let model_path = model_dir.join("model.gguf");
    std::fs::File::create(&model_path).unwrap();

    let mut model = ModelInfo::new(model_name.to_string(), 0, "test".to_string(), 0, vec![]);
    model.path = Some(model_path.to_string_lossy().to_string());
    llm_router::api::models::upsert_registered_model(model);
    llm_router::api::models::persist_registered_models(&test_app.db_pool).await;

    let models_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/models")
                .header("x-api-key", "sk_debug")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(models_res.status(), StatusCode::OK);
    let body = to_bytes(models_res.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let entry = body["data"]
        .as_array()
        .and_then(|arr| arr.iter().find(|m| m["id"] == model_name))
        .expect("model should be listed");
    assert_eq!(entry["ready"], false);
    assert_eq!(entry["lifecycle_status"], "pending");
}

/// T003: 0Bキャッシュは再ダウンロードされる
#[tokio::test]
#[serial]
async fn test_zero_byte_cache_triggers_redownload() {
    let mock = MockServer::start().await;
    std::env::set_var("HF_BASE_URL", mock.uri());

    Mock::given(method("GET"))
        .and(path("/api/models/zero/repo"))
        .and(query_param("expand", "siblings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "siblings": [
                {"rfilename": "model.gguf"}
            ]
        })))
        .mount(&mock)
        .await;

    Mock::given(method("HEAD"))
        .and(path("/zero/repo/resolve/main/model.gguf"))
        .respond_with(ResponseTemplate::new(200).append_header("content-length", "4"))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/zero/repo/resolve/main/model.gguf"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"GGUF"))
        .mount(&mock)
        .await;

    let TestApp { app, admin_key, .. } = build_app().await;

    let base = router_models_dir().expect("router models dir should exist");
    let model_dir = base.join(model_name_to_dir("zero/repo"));
    std::fs::create_dir_all(&model_dir).unwrap();
    let model_path = model_dir.join("model.gguf");
    std::fs::File::create(&model_path).unwrap();

    let payload = json!({
        "repo": "zero/repo",
        "filename": "model.gguf"
    });

    let response = app
        .clone()
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri("/v0/models/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let mut ready = false;
    for _ in 0..30 {
        let models_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/models")
                    .header("x-api-key", "sk_debug")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(models_res.status(), StatusCode::OK);
        let body = to_bytes(models_res.into_body(), usize::MAX).await.unwrap();
        let models: serde_json::Value = serde_json::from_slice(&body).unwrap();
        if models["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .any(|m| m["id"] == "zero/repo" && m["ready"] == true)
            })
            .unwrap_or(false)
        {
            ready = true;
            break;
        }
        sleep(Duration::from_millis(100)).await;
    }

    assert!(ready, "zero-byte cache should be re-downloaded");
    let meta = std::fs::metadata(&model_path).unwrap();
    assert!(meta.len() > 0);
}

/// T004: キャッシュ済みモデルは再ダウンロードせず即時登録される
#[tokio::test]
#[serial]
async fn test_register_model_uses_existing_cache() {
    let mock = MockServer::start().await;
    std::env::set_var("HF_BASE_URL", mock.uri());

    Mock::given(method("GET"))
        .and(path("/api/models/cached/repo"))
        .and(query_param("expand", "siblings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "siblings": [
                {"rfilename": "model.gguf"}
            ]
        })))
        .mount(&mock)
        .await;

    Mock::given(method("HEAD"))
        .and(path("/cached/repo/resolve/main/model.gguf"))
        .respond_with(ResponseTemplate::new(200).append_header("content-length", "4"))
        .mount(&mock)
        .await;

    // ダウンロードが呼ばれたら失敗させる
    Mock::given(method("GET"))
        .and(path("/cached/repo/resolve/main/model.gguf"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock)
        .await;

    let TestApp { app, admin_key, .. } = build_app().await;

    let base = router_models_dir().expect("router models dir should exist");
    let model_dir = base.join(model_name_to_dir("cached/repo"));
    std::fs::create_dir_all(&model_dir).unwrap();
    let model_path = model_dir.join("model.gguf");
    std::fs::write(&model_path, b"GGUF").unwrap();

    let payload = json!({
        "repo": "cached/repo",
        "filename": "model.gguf"
    });

    let response = app
        .clone()
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri("/v0/models/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let mut ready = false;
    let mut last_models = serde_json::Value::Null;
    for _ in 0..30 {
        let models_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/models")
                    .header("x-api-key", "sk_debug")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(models_res.status(), StatusCode::OK);
        let body = to_bytes(models_res.into_body(), usize::MAX).await.unwrap();
        let models: serde_json::Value = serde_json::from_slice(&body).unwrap();
        last_models = models.clone();
        if models["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .any(|m| m["id"] == "cached/repo" && m["ready"] == true)
            })
            .unwrap_or(false)
        {
            ready = true;
            break;
        }
        sleep(Duration::from_millis(100)).await;
    }

    assert!(
        ready,
        "cached model should be ready without download, models={:?}",
        last_models
    );
    let meta = std::fs::metadata(&model_path).unwrap();
    assert!(meta.len() > 0);
}

/// T005: 削除後に /v1/models から消える
#[tokio::test]
#[serial]
async fn test_delete_model_removes_from_list() {
    let TestApp {
        app,
        admin_key,
        db_pool,
        ..
    } = build_app().await;

    let model_name = "delete-me";
    let base = router_models_dir().expect("router models dir should exist");
    let model_dir = base.join(model_name_to_dir(model_name));
    std::fs::create_dir_all(&model_dir).unwrap();
    let model_path = model_dir.join("model.gguf");
    std::fs::write(&model_path, b"GGUF").unwrap();

    let mut model = ModelInfo::new(model_name.to_string(), 0, "test".to_string(), 0, vec![]);
    model.path = Some(model_path.to_string_lossy().to_string());
    llm_router::api::models::upsert_registered_model(model);
    llm_router::api::models::persist_registered_models(&db_pool).await;

    let models_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/models")
                .header("x-api-key", "sk_debug")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(models_res.status(), StatusCode::OK);
    let body = to_bytes(models_res.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(
        body["data"]
            .as_array()
            .map(|arr| arr.iter().any(|m| m["id"] == model_name))
            .unwrap_or(false),
        "model should exist before delete"
    );

    let delete_res = app
        .clone()
        .oneshot(
            admin_request(&admin_key)
                .method("DELETE")
                .uri(format!("/v0/models/{}", model_name))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_res.status(), StatusCode::NO_CONTENT);

    let models_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/models")
                .header("x-api-key", "sk_debug")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(models_res.status(), StatusCode::OK);
    let body = to_bytes(models_res.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(
        body["data"]
            .as_array()
            .map(|arr| arr.iter().all(|m| m["id"] != model_name))
            .unwrap_or(false),
        "model should be removed after delete"
    );
    assert!(!model_path.exists(), "model file should be removed");
}
/// T010: ダウンロード失敗時に lifecycle_status が error になること
/// NOTE: /v0/models/convert は廃止され、/v0/models に統合された
/// NOTE: 失敗後のリトライ機能は別途実装予定
#[tokio::test]
#[serial]
async fn test_download_failure_shows_error_status() {
    let mock = MockServer::start().await;
    std::env::set_var("HF_BASE_URL", mock.uri());

    // siblings returns GGUF file for registration to succeed
    Mock::given(method("GET"))
        .and(path("/api/models/error-test-repo"))
        .and(query_param("expand", "siblings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "siblings": [
                {"rfilename": "model.Q4_K_M.gguf"}
            ]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("HEAD"))
        .and(path("/error-test-repo/resolve/main/model.Q4_K_M.gguf"))
        .respond_with(ResponseTemplate::new(200).append_header("content-length", "42"))
        .mount(&mock)
        .await;
    // ダウンロードは常に失敗
    Mock::given(method("GET"))
        .and(path("/error-test-repo/resolve/main/model.Q4_K_M.gguf"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock)
        .await;

    let TestApp { app, admin_key, .. } = build_app().await;

    // register -> download fails
    let reg = app
        .clone()
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri("/v0/models/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "repo": "error-test-repo",
                        "filename": "model.Q4_K_M.gguf"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reg.status(), StatusCode::CREATED);

    // wait for error status via /v1/models lifecycle_status (OpenAI互換エンドポイント)
    let mut error_seen = false;
    let mut last_models = serde_json::Value::Null;
    for _ in 0..60 {
        let models_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/models")
                    .header("authorization", "Bearer sk_debug")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(models_resp.status(), StatusCode::OK);
        let body = to_bytes(models_resp.into_body(), usize::MAX).await.unwrap();
        let models: serde_json::Value = serde_json::from_slice(&body).unwrap();
        last_models = models.clone();
        // /v1/models レスポンス形式: { "object": "list", "data": [...] }
        if models["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .any(|m| m["id"] == "error-test-repo" && m["lifecycle_status"] == "error")
            })
            .unwrap_or(false)
        {
            error_seen = true;
            break;
        }
        sleep(Duration::from_millis(200)).await;
    }
    assert!(
        error_seen,
        "model should have lifecycle_status=error, models={:?}",
        last_models
    );

    // エラーモデルは download_progress.error にエラーメッセージが含まれる
    let model = last_models["data"]
        .as_array()
        .and_then(|arr| arr.iter().find(|m| m["id"] == "error-test-repo"))
        .unwrap();
    assert!(
        model["download_progress"]["error"].is_string(),
        "download_progress.error should contain error message"
    );

    // エラー状態のモデルは削除可能
    let delete_resp = app
        .clone()
        .oneshot(
            admin_request(&admin_key)
                .method("DELETE")
                .uri("/v0/models/error-test-repo")
                .header("x-api-key", "sk_debug")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let delete_status = delete_resp.status();
    let delete_body = to_bytes(delete_resp.into_body(), usize::MAX).await.unwrap();
    let delete_body_str = String::from_utf8_lossy(&delete_body);
    assert!(
        delete_status == StatusCode::NO_CONTENT
            || delete_status == StatusCode::OK
            || delete_status == StatusCode::NOT_FOUND, // モデルが既に存在しない場合も許容
        "should be able to delete error model (status={}, body={})",
        delete_status,
        delete_body_str
    );
}
