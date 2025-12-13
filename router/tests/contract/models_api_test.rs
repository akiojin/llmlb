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
use std::sync::Arc;
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
        convert_manager,
        db_pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
    };

    api::create_router(state)
}

/// モデル配布APIは廃止（ノードが /v1/models と /api/models/blob から自律取得）
#[tokio::test]
#[serial]
async fn test_distribute_models_endpoint_is_removed() {
    std::env::set_var("LLM_ROUTER_SKIP_HEALTH_CHECK", "1");
    let app = build_app().await;

    let request_body = json!({
        "model_name": "gpt-oss-20b",
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

    assert!(
        matches!(
            response.status(),
            StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
        ),
        "model distribution endpoint should be removed (got {})",
        response.status()
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
            // model.gguf (generic) + test/repo → "repo"
            .any(|m| m["name"] == "repo"),
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

/// ノードのモデル一覧取得APIは廃止（ロード済みモデルは /api/nodes と /api/dashboard/nodes から参照）
#[tokio::test]
#[serial]
async fn test_get_node_models_endpoint_is_removed() {
    std::env::set_var("LLM_ROUTER_SKIP_HEALTH_CHECK", "1");
    let app = build_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/nodes/{}/models", Uuid::new_v4()))
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
    std::env::set_var("LLM_ROUTER_SKIP_HEALTH_CHECK", "1");
    let app = build_app().await;

    let request_body = json!({
        "model_name": "gpt-oss-3b"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/nodes/{}/models/pull", Uuid::new_v4()))
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
    std::env::set_var("LLM_ROUTER_SKIP_HEALTH_CHECK", "1");
    let app = build_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/tasks")
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

/// T009: POST /api/models/register - 正常系と重複/404異常系
#[tokio::test]
#[serial]
async fn test_register_model_contract() {
    std::env::set_var("LLM_ROUTER_SKIP_HEALTH_CHECK", "1");
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
    // model.gguf (generic) + test/repo → "repo"
    assert!(
        data.iter().all(|m| m["id"] != "repo"),
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

    // repoのみ指定でGGUFなし→変換パスに進むため201を返す（新API仕様）
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
    // 新APIではGGUFがない場合は変換パスに進むため201を返す
    assert_eq!(repo_only.status(), StatusCode::CREATED);

    // repoのみ、GGUFなし → 変換パスに進むため201を返す（新API仕様）
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
    // 新APIではGGUFがない場合は変換パスに進むため201を返す
    assert_eq!(repo_only_fallback.status(), StatusCode::CREATED);

    // DELETE: タスク完了前なのでモデルはREGISTERED_MODELSに存在しない（400を期待）
    // モデル名 = リポジトリ名、ワイルドカードパスなのでスラッシュをそのまま使用
    let delete_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/models/test/repo")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // タスク完了前はモデルが登録されていないため400 (model not found)
    assert_eq!(delete_res.status(), StatusCode::BAD_REQUEST);

    // GGUF登録後に /v1/models に出ること（LLM_CONVERT_FAKE=1でダミー生成）
    let app_for_convert = build_app().await;
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
            .map(|arr| arr.iter().any(|m| m["id"] == "convertible-repo"))
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

    let mock = MockServer::start().await;
    std::env::set_var("HF_BASE_URL", mock.uri());

    // siblings returns GGUF file for registration to succeed
    Mock::given(method("GET"))
        .and(path("/api/models/restore-repo"))
        .and(query_param("expand", "siblings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "siblings": [
                {"rfilename": "model.Q4_K_M.gguf"}
            ]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("HEAD"))
        .and(path("/restore-repo/resolve/main/model.Q4_K_M.gguf"))
        .respond_with(ResponseTemplate::new(200).append_header("content-length", "42"))
        .mount(&mock)
        .await;
    // GETダウンロードを初回は失敗させる（500エラー）→リトライで成功させる
    let download_counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let counter_clone = download_counter.clone();
    Mock::given(method("GET"))
        .and(path("/restore-repo/resolve/main/model.Q4_K_M.gguf"))
        .respond_with(move |_req: &wiremock::Request| {
            let count = counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if count == 0 {
                // 1回目: 失敗
                ResponseTemplate::new(500)
            } else {
                // 2回目以降: 成功
                ResponseTemplate::new(200).set_body_bytes(b"GGUF dummy content")
            }
        })
        .mount(&mock)
        .await;

    let app = build_app().await;

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

    // 2回目以降はモックが成功を返すので、リトライでダウンロードが成功するはず
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
                        "filename": "model.Q4_K_M.gguf"
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
                .header("x-api-key", "sk_debug")
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
        .map(|arr| arr.iter().any(|m| m["id"] == "restore-repo"))
        .unwrap_or(false);
    assert!(present, "/v1/models must include restored model");
}
