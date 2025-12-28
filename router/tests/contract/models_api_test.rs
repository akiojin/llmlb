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
use serde_json::json;
use serial_test::serial;
use tower::ServiceExt;
use uuid::Uuid;

struct TestApp {
    app: Router,
    db_pool: sqlx::SqlitePool,
    admin_key: String,
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
