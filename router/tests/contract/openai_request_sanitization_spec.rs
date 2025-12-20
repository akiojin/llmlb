//! OpenAI request history sanitization contract test
//!
//! TDD RED: このテストはサニタイズ実装前に失敗する必要があります。

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use llm_router::{
    api, balancer::LoadManager, db::request_history::RequestHistoryStorage, registry::NodeRegistry,
    AppState,
};
use serde_json::json;
use serial_test::serial;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tower::ServiceExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct TestApp {
    router: Router,
    request_history: Arc<RequestHistoryStorage>,
}

async fn build_app(openai_base_url: String) -> TestApp {
    // テスト用に一時ディレクトリを設定
    let temp_dir = std::env::temp_dir().join(format!(
        "or-test-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    std::env::set_var("LLM_ROUTER_DATA_DIR", &temp_dir);
    std::env::set_var("HOME", &temp_dir);
    std::env::set_var("USERPROFILE", &temp_dir);

    // OpenAI互換エンドポイントのAPIキー認証をスキップ（テスト用）
    std::env::set_var("LLM_ROUTER_SKIP_API_KEY", "1");

    // Cloud proxy用（wiremockへ向ける）
    std::env::set_var("OPENAI_API_KEY", "sk-test");
    std::env::set_var("OPENAI_BASE_URL", openai_base_url);

    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run migrations");
    let request_history = Arc::new(RequestHistoryStorage::new(db_pool.clone()));
    let convert_manager = llm_router::convert::ConvertTaskManager::new(1);
    let jwt_secret = "test-secret".to_string();
    let state = AppState {
        registry,
        load_manager,
        request_history: request_history.clone(),
        convert_manager,
        db_pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
    };

    TestApp {
        router: api::create_router(state),
        request_history,
    }
}

async fn wait_for_one_record(
    storage: &RequestHistoryStorage,
) -> llm_router_common::protocol::RequestResponseRecord {
    for _ in 0..50 {
        let records = storage.load_records().await.expect("records");
        if let Some(last) = records.last() {
            return last.clone();
        }
        sleep(Duration::from_millis(20)).await;
    }
    panic!("timed out waiting for request history record");
}

#[tokio::test]
#[serial]
async fn request_history_redacts_inline_media_data() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl_test",
            "object": "chat.completion",
            "created": 0,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": { "role": "assistant", "content": "ok" },
                "finish_reason": "stop"
            }]
        })))
        .mount(&mock_server)
        .await;

    let app = build_app(mock_server.uri()).await;

    let sensitive_audio = "SENSITIVE_AUDIO_BASE64_0123456789";
    let sensitive_image = "SENSITIVE_IMAGE_BASE64_0123456789";
    let data_image_url = format!("data:image/png;base64,{sensitive_image}");

    let request_body = json!({
        "model": "openai:gpt-4o",
        "stream": false,
        "messages": [{
            "role": "user",
            "content": [
                { "type": "text", "text": "hello" },
                { "type": "image_url", "image_url": { "url": data_image_url } },
                { "type": "input_audio", "input_audio": { "data": sensitive_audio, "format": "wav" } }
            ]
        }]
    });

    let response = app
        .router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let _ = to_bytes(response.into_body(), 128 * 1024).await.unwrap();

    let record = wait_for_one_record(&app.request_history).await;
    let stored = serde_json::to_string(&record.request_body).expect("stored json");

    // 添付の生データが履歴に残らないこと（RED: 現状は残るため失敗するはず）
    assert!(
        !stored.contains(sensitive_image),
        "request history should redact image base64"
    );
    assert!(
        !stored.contains(sensitive_audio),
        "request history should redact audio base64"
    );
}
