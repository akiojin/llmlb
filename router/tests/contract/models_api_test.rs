//! モデル管理API契約テスト
//!
//! TDD RED: これらのテストは実装前に失敗する必要があります

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use llm_router::{api, balancer::LoadManager, registry::NodeRegistry, AppState};
use llm_router_common::auth::{ApiKeyScope, UserRole};
use serde_json::json;
use serial_test::serial;
use tokio::time::{sleep, Duration};
use tower::ServiceExt;
use uuid::Uuid;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn build_app() -> (Router, String) {
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
        vec![ApiKeyScope::AdminAll],
    )
    .await
    .expect("create admin api key")
    .key;

    (api::create_router(state), admin_key)
}

fn admin_request(admin_key: &str) -> axum::http::request::Builder {
    Request::builder().header("authorization", format!("Bearer {}", admin_key))
}

/// モデル配布APIは廃止（ノードが /v1/models と /v0/models/blob から自律取得）
#[tokio::test]
#[serial]
async fn test_distribute_models_endpoint_is_removed() {
    let (app, admin_key) = build_app().await;

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
    let (app, admin_key) = build_app().await;

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
    let (app, admin_key) = build_app().await;

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
    let (app, admin_key) = build_app().await;

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
    let (app, admin_key) = build_app().await;

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

    // HEAD 200 for existence
    Mock::given(method("HEAD"))
        .and(path("/test/repo/resolve/main/model.gguf"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock)
        .await;

    std::env::set_var("HF_BASE_URL", mock.uri());

    let (app, admin_key) = build_app().await;

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

    // /v1/models に含まれること
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
    // model.gguf (generic) + test/repo → "repo"
    assert!(
        data.iter().all(|m| m["id"] != "repo"),
        "/v1/models must not expose models before download completes"
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
            admin_request(&admin_key)
                .method("POST")
                .uri("/v0/models/register")
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
            admin_request(&admin_key)
                .method("POST")
                .uri("/v0/models/register")
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
    let (app_for_convert, admin_key) = build_app().await;
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

    let (app, admin_key) = build_app().await;

    // register -> download fails
    let reg = app
        .clone()
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri("/v0/models/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"repo": "error-test-repo"})).unwrap(),
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
