use super::metadata::MetadataScanner;
use super::{accept_current, capture_record, usage_capture_middleware};
use crate::auth::middleware::ApiKeyAuthContext;
use crate::auth::scope::Scope;
use crate::common::{
    auth::{ApiKeyPermission, Claims, UserRole},
    protocol::{RequestResponseRecord, RequestType},
    usage::{TokenSource, UsageTokens},
};
use crate::AppState;
use axum::{
    body::{to_bytes, Body, Bytes},
    extract::{ConnectInfo, State},
    http::{Request, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::post,
    Extension, Json, Router,
};
use serde_json::{json, Value};
use sqlx::Row;
use std::{net::SocketAddr, time::Duration};
use tower::ServiceExt;
use uuid::Uuid;

fn observe(content_type: &str, body: &[u8]) -> Option<UsageTokens> {
    let mut scanner = MetadataScanner::new(content_type);
    for chunk in body.chunks(3) {
        scanner.push(chunk);
    }
    scanner.observation()
}

#[test]
fn provider_zero_partial_and_invalid_numbers_are_not_estimates_or_missing_zero() {
    assert_eq!(
        observe(
            "application/json",
            br#"{"usage":{"prompt_tokens":0,"completion_tokens":0,"total_tokens":0}}"#
        ),
        Some(UsageTokens::provider(Some(0), Some(0), Some(0)))
    );
    assert_eq!(
        observe("application/json", br#"{"usage":{"input_tokens":7,"output_tokens":-1,"total_tokens":9223372036854775808}}"#),
        Some(UsageTokens::provider(Some(7), None, None))
    );
    assert_eq!(
        observe(
            "application/json",
            br#"{"usage":{"input_tokens":"7","output_tokens":1.5,"total_tokens":null}}"#
        ),
        Some(UsageTokens::provider(None, None, None))
    );
    assert!(observe("audio/mpeg", br#"{"usage":{"total_tokens":99}}"#).is_none());
    assert_eq!(
        observe("application/json", br#"{"usage":{"input_tokens":7,"output_tokens":999999999999999999999999999999999999999999999999999999999999999999}}"#),
        Some(UsageTokens::provider(Some(7), None, None))
    );
    assert!(observe("application/json", br#"{"usage":{"input_tokens":7,"output_tokens":99999999999999999999999999999999999999999invalid}}"#).is_none());
}

#[test]
fn sse_anthropic_and_responses_merge_only_complete_usage_objects() {
    let body = b"event: message_start\ndata: {\"message\":{\"usage\":{\"input_tokens\":12,\"output_tokens\":0}}}\n\ndata: {\"usage\":{\"output_tokens\":9}}\n\ndata: [DONE]\n\n";
    assert_eq!(
        observe("text/event-stream", body),
        Some(UsageTokens::provider(Some(12), Some(9), None))
    );
    assert_eq!(
        observe("text/event-stream; charset=utf-8", b"data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":4,\"output_tokens\":5,\"total_tokens\":9}}}\r\n\r\n"),
        Some(UsageTokens::provider(Some(4), Some(5), Some(9)))
    );
}

#[test]
fn truncated_payload_retains_complete_usage_but_not_an_unfinished_object_or_number() {
    assert_eq!(
        observe(
            "application/json",
            br#"{"usage":{"input_tokens":8,"output_tokens":2},"large":{"text":"unfinished"#
        ),
        Some(UsageTokens::provider(Some(8), Some(2), None))
    );
    for body in [
        br#"{"usage":{"input_tokens":8"#.as_slice(),
        br#"{"usage":{"input_tokens":8,"output_tokens":2"#.as_slice(),
        br#"{"usage":{"input_tokens":8,}}"#.as_slice(),
    ] {
        assert!(observe("application/json", body).is_none());
    }
    assert_eq!(
        observe(
            "text/event-stream",
            b"data: {\"usage\":{\"input_tokens\":8}}\n\ndata: {\"usage\":{\"input_tokens\":999"
        ),
        Some(UsageTokens::provider(Some(8), None, None))
    );
}

#[test]
fn huge_json_strings_do_not_hide_usage_or_grow_the_metadata_buffer() {
    let mut scanner = MetadataScanner::new("application/json");
    scanner.push(br#"{"data":[{"b64_json":""#);
    for _ in 0..4096 {
        scanner.push(&[b'A'; 1024]);
        assert!(scanner.retained_bytes() < 16 * 1024);
    }
    scanner.push(br#""}],"usage":{"input_tokens":3,"output_tokens":4,"total_tokens":7}}"#);
    assert_eq!(
        scanner.observation(),
        Some(UsageTokens::provider(Some(3), Some(4), Some(7)))
    );
}

#[test]
fn usage_in_strings_or_arbitrary_nested_payload_is_not_provider_authority() {
    for body in [
        br#"{"choices":[{"message":{"content":"{\"usage\":{\"total_tokens\":900}}"}}]}"#.as_slice(),
        br#"{"data":{"usage":{"total_tokens":900}}}"#.as_slice(),
        br#"{"usage":{"input_tokens":7,"nested":{"output_tokens":999}}}"#.as_slice(),
    ] {
        let observed = observe("application/json", body);
        assert!(observed.is_none() || observed == Some(UsageTokens::provider(Some(7), None, None)));
    }
    assert_eq!(
        observe("application/json", br#"{"\u0075sage":{"input_tokens":2}}"#),
        Some(UsageTokens::provider(Some(2), None, None))
    );
}

async fn handler(State(state): State<AppState>, Json(payload): Json<Value>) -> Response {
    if payload["invalid"] == true {
        return StatusCode::BAD_REQUEST.into_response();
    }
    if let Err(error) =
        accept_current(&state, "fixture-model", RequestType::Chat, payload.clone()).await
    {
        return crate::api::error::AppError(error).into_response();
    }
    // An internal retry calls admission again, and old capture helpers may emit
    // multiple metadata records. Neither is another accepted HTTP request.
    accept_current(&state, "fixture-model", RequestType::Chat, payload.clone())
        .await
        .unwrap();
    let mut record = RequestResponseRecord::new(
        Uuid::from_u128(42),
        "fixture-target".into(),
        "127.0.0.42".parse().unwrap(),
        "fixture-model".into(),
        RequestType::Chat,
        payload.clone(),
        StatusCode::OK,
        Duration::ZERO,
        None,
        None,
    );
    record.input_tokens = Some(100);
    record.output_tokens = Some(200);
    record.total_tokens = Some(300);
    assert!(capture_record(&record));
    assert!(capture_record(&record));
    match payload["mode"].as_str().unwrap_or("json") {
        "sse_wire" => Response::builder()
            .header("content-type", "text/event-stream")
            .body(Body::from(payload["wire"].as_str().unwrap().to_string()))
            .unwrap(),
        "error" => Response::builder()
            .header("content-type", "text/event-stream")
            .body(Body::from_stream(futures::stream::iter([
                Ok(Bytes::from_static(
                    b"data: {\"usage\":{\"input_tokens\":5}}\n\n",
                )),
                Err(std::io::Error::other("upstream body failed")),
            ])))
            .unwrap(),
        "drop" => Response::builder()
            .header("content-type", "text/event-stream")
            .body(Body::from_stream(futures::stream::pending::<
                Result<Bytes, std::io::Error>,
            >()))
            .unwrap(),
        "binary" => Response::builder()
            .header("content-type", "audio/mpeg")
            .body(Body::from(vec![0_u8, 255, 2, 3]))
            .unwrap(),
        "estimated" => Response::builder()
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .unwrap(),
        _ => Response::builder()
            .header("content-type", "application/json")
            .body(Body::from(
                "{\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":3,\"total_tokens\":5}}",
            ))
            .unwrap(),
    }
}

async fn fixture(legacy: bool) -> (Router, AppState, ApiKeyAuthContext) {
    let state = crate::db::test_utils::TestAppStateBuilder::new()
        .await
        .build()
        .await;
    let context = ApiKeyAuthContext {
        id: Uuid::new_v4(),
        created_by: Uuid::new_v4(),
        permissions: vec![ApiKeyPermission::OpenaiInference],
        expires_at: None,
        usage_account_id: (!legacy).then(Uuid::new_v4),
        organization_id: (!legacy).then(Uuid::new_v4),
    };
    let app = Router::new()
        .route("/inference", post(handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            usage_capture_middleware,
        ))
        .layer(Extension(context.clone()))
        .with_state(state.clone());
    (app, state, context)
}

async fn request(app: &Router, payload: Value) -> Response {
    let mut request = Request::builder()
        .method("POST")
        .uri("/inference")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "[::ffff:192.0.2.17]:5000, 127.0.0.1")
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo("127.0.0.1:9".parse::<SocketAddr>().unwrap()));
    app.clone().oneshot(request).await.unwrap()
}

#[tokio::test]
async fn body_eof_preserves_payload_and_counts_one_receipt_with_immutable_identity_and_ip() {
    let (app, state, context) = fixture(false).await;
    let response = request(
        &app,
        json!({"organization_id":Uuid::new_v4(),"request_id":"client-spoof"}),
    )
    .await;
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        bytes,
        "{\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":3,\"total_tokens\":5}}"
    );
    state
        .balancer
        .request_history
        .drain_usage_writers()
        .await
        .unwrap();
    let receipts = sqlx::query(
        "SELECT request_id,account_id,organization_id,api_key_id,token_source FROM usage_receipts",
    )
    .fetch_all(&state.db_pool)
    .await
    .unwrap();
    assert_eq!(receipts.len(), 1);
    let receipt = &receipts[0];
    let request_id: String = receipt.get("request_id");
    assert!(Uuid::parse_str(&request_id).is_ok());
    assert_eq!(
        receipt.get::<Option<String>, _>("account_id"),
        context.usage_account_id.map(|id| id.to_string())
    );
    assert_eq!(
        receipt.get::<Option<String>, _>("organization_id"),
        context.organization_id.map(|id| id.to_string())
    );
    assert_eq!(
        receipt.get::<String, _>("api_key_id"),
        context.id.to_string()
    );
    assert_eq!(receipt.get::<String, _>("token_source"), "provider");
    let row = sqlx::query("SELECT request_count,input_tokens,output_tokens,total_tokens,pending_requests FROM usage_daily")
        .fetch_one(&state.db_pool).await.unwrap();
    assert_eq!(row.get::<i64, _>("request_count"), 1);
    assert_eq!(row.get::<i64, _>("input_tokens"), 2);
    assert_eq!(row.get::<i64, _>("output_tokens"), 3);
    assert_eq!(row.get::<i64, _>("total_tokens"), 5);
    assert_eq!(row.get::<i64, _>("pending_requests"), 0);
    let record = state
        .balancer
        .request_history
        .get_record_by_id(request_id.parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.client_ip.unwrap().to_string(), "192.0.2.17");
    assert_eq!(record.endpoint_id, Uuid::from_u128(42));
    assert_eq!(record.api_key_id, Some(context.id));
}

#[tokio::test]
async fn body_error_and_drop_finalize_once_and_preserve_partial_provider_coverage() {
    for mode in ["error", "drop"] {
        let (app, state, _) = fixture(false).await;
        let response = request(&app, json!({"mode":mode})).await;
        if mode == "error" {
            assert!(to_bytes(response.into_body(), usize::MAX).await.is_err());
        } else {
            drop(response);
        }
        state
            .balancer
            .request_history
            .drain_usage_writers()
            .await
            .unwrap();
        let row = sqlx::query("SELECT request_count,pending_requests,input_tokens,output_missing_count,missing_count FROM usage_daily")
            .fetch_one(&state.db_pool).await.unwrap();
        assert_eq!(row.get::<i64, _>("request_count"), 1);
        assert_eq!(row.get::<i64, _>("pending_requests"), 0);
        assert_eq!(
            row.get::<i64, _>("input_tokens"),
            if mode == "error" { 5 } else { 0 }
        );
        assert_eq!(row.get::<i64, _>("output_missing_count"), 1);
        assert_eq!(
            row.get::<i64, _>("missing_count"),
            i64::from(mode == "drop")
        );
    }
}

#[tokio::test]
async fn estimated_fallback_is_labeled_and_binary_or_legacy_never_acquire_an_issuer_identity() {
    for (mode, source) in [
        ("estimated", TokenSource::Estimated),
        ("binary", TokenSource::Missing),
    ] {
        let (app, state, _) = fixture(true).await;
        let response = request(&app, json!({"mode":mode})).await;
        to_bytes(response.into_body(), usize::MAX).await.unwrap();
        state
            .balancer
            .request_history
            .drain_usage_writers()
            .await
            .unwrap();
        let row = sqlx::query("SELECT account_id,organization_id,token_source FROM usage_receipts")
            .fetch_one(&state.db_pool)
            .await
            .unwrap();
        assert!(row.get::<Option<String>, _>("account_id").is_none());
        assert!(row.get::<Option<String>, _>("organization_id").is_none());
        assert_eq!(row.get::<String, _>("token_source"), source.as_str());
    }
}

#[tokio::test]
async fn input_rejection_or_failed_durable_admission_cannot_count_or_forward_inference() {
    let (app, state, _) = fixture(false).await;
    assert_eq!(
        request(&app, json!({"invalid":true})).await.status(),
        StatusCode::BAD_REQUEST
    );
    state
        .balancer
        .request_history
        .drain_usage_writers()
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM usage_receipts")
        .fetch_one(&state.db_pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    state.db_pool.close().await;
    assert_eq!(
        request(&app, json!({})).await.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn jwt_unassigned_scope_is_rejected_before_the_handler_can_accept() {
    let state = crate::db::test_utils::TestAppStateBuilder::new()
        .await
        .build()
        .await;
    let id = Uuid::new_v4();
    let claims = Claims {
        sub: id.to_string(),
        role: UserRole::Viewer,
        exp: usize::MAX,
        must_change_password: false,
        password_changed_at: 0,
    };
    let app = Router::new()
        .route("/inference", post(handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            usage_capture_middleware,
        ))
        .layer(Extension(Scope::Unassigned(id)))
        .layer(Extension(claims))
        .with_state(state.clone());
    assert_eq!(
        request(&app, json!({})).await.status(),
        StatusCode::FORBIDDEN
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM usage_receipts")
        .fetch_one(&state.db_pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn missing_capture_refuses_inference_admission_instead_of_silently_skipping_usage() {
    let state = crate::db::test_utils::TestAppStateBuilder::new()
        .await
        .build()
        .await;
    let result = accept_current(
        &state,
        "fixture-model",
        RequestType::Chat,
        json!({"model":"fixture-model"}),
    )
    .await;
    assert!(
        result.is_err(),
        "an omitted inference capture layer must fail closed"
    );
    let receipts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM usage_receipts")
        .fetch_one(&state.db_pool)
        .await
        .unwrap();
    assert_eq!(receipts, 0);
}

fn cancellation_capture(
    storage: std::sync::Arc<crate::db::request_history::RequestHistoryStorage>,
    attribution: crate::common::usage::UsageAttribution,
) -> std::sync::Arc<super::Capture> {
    std::sync::Arc::new(super::Capture {
        attribution,
        client_ip: Some("192.0.2.17".parse().unwrap()),
        started: std::time::Instant::now(),
        storage,
        admission: tokio::sync::Mutex::new(()),
        state: std::sync::Mutex::new(super::CapturedState::default()),
    })
}

fn cancellation_attribution() -> crate::common::usage::UsageAttribution {
    crate::common::usage::UsageAttribution {
        request_id: Uuid::new_v4(),
        account_id: Some(Uuid::new_v4()),
        organization_id: Some(Uuid::new_v4()),
        received_at: chrono::Utc::now(),
        api_key_id: None,
        auth_source: "jwt".to_string(),
    }
}

async fn cancellation_pool(directory: &tempfile::TempDir) -> sqlx::SqlitePool {
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(directory.path().join("capture-cancellation.db"))
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

struct AdmissionWake(tokio::sync::Notify);

impl futures::task::ArcWake for AdmissionWake {
    fn wake_by_ref(wake: &std::sync::Arc<Self>) {
        wake.0.notify_one();
    }
}

#[tokio::test]
async fn cancellation_after_sqlite_commit_before_parent_repoll_finalizes_the_existing_receipt() {
    use std::future::Future;
    let directory = tempfile::tempdir().unwrap();
    let pool = cancellation_pool(&directory).await;
    let storage = std::sync::Arc::new(crate::db::request_history::RequestHistoryStorage::new(
        pool.clone(),
    ));
    let attribution = cancellation_attribution();
    let capture = cancellation_capture(storage.clone(), attribution.clone());
    let mut admission =
        Box::pin(capture.accept("fixture", RequestType::Chat, json!({"model":"fixture"})));
    let wake = std::sync::Arc::new(AdmissionWake(tokio::sync::Notify::new()));
    let waker = futures::task::waker(wake.clone());
    let mut committed_while_pending = false;
    for _ in 0..64 {
        let progress = admission
            .as_mut()
            .poll(&mut std::task::Context::from_waker(&waker));
        assert!(
            progress.is_pending(),
            "fixture must stop before the parent's final await is repolled"
        );
        // SQLite runs on its worker. Wait for that worker's actual completion
        // wake without polling the parent admission future a second time.
        tokio::time::timeout(Duration::from_secs(2), wake.0.notified())
            .await
            .unwrap();
        let persisted: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM usage_receipts WHERE request_id=?)")
                .bind(attribution.request_id.to_string())
                .fetch_one(&pool)
                .await
                .unwrap();
        if persisted {
            committed_while_pending = true;
            break;
        }
    }
    assert!(
        committed_while_pending,
        "a separate WAL reader must prove COMMIT before cancellation"
    );
    drop(admission);
    drop(capture);
    storage.drain_usage_writers().await.unwrap();
    let state: (Option<String>, i64, i64, i64) = sqlx::query_as(
        "SELECT r.finalized_at,d.request_count,d.pending_requests,d.missing_count
         FROM usage_receipts r JOIN usage_daily d ON d.date=r.date
         AND d.organization_id=r.organization_id AND d.account_id=r.account_id WHERE r.request_id=?",
    ).bind(attribution.request_id.to_string()).fetch_one(&pool).await.unwrap();
    assert!(
        state.0.is_some(),
        "committed acceptance cancelled before parent repoll must settle in this process"
    );
    assert_eq!((state.1, state.2, state.3), (1, 0, 1));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT failed_requests FROM usage_daily")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn cancellation_before_any_durable_receipt_never_creates_a_request() {
    use std::future::Future;
    let directory = tempfile::tempdir().unwrap();
    let pool = cancellation_pool(&directory).await;
    let storage = std::sync::Arc::new(crate::db::request_history::RequestHistoryStorage::new(
        pool.clone(),
    ));
    let blocker = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let capture = cancellation_capture(storage.clone(), cancellation_attribution());
    let mut admission = Box::pin(capture.accept("fixture", RequestType::Chat, json!({})));
    let wake = std::sync::Arc::new(AdmissionWake(tokio::sync::Notify::new()));
    let waker = futures::task::waker(wake);
    assert!(admission
        .as_mut()
        .poll(&mut std::task::Context::from_waker(&waker))
        .is_pending());
    drop(admission);
    blocker.rollback().await.unwrap();
    drop(capture);
    storage.drain_usage_writers().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM usage_receipts")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM usage_daily")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn rejected_admission_drop_does_not_convert_a_storage_error_into_usage() {
    let pool = crate::db::test_utils::test_db_pool().await;
    sqlx::query("CREATE TRIGGER reject_receipt BEFORE INSERT ON usage_receipts BEGIN SELECT RAISE(ABORT,'fixture rejection'); END")
        .execute(&pool).await.unwrap();
    let storage = std::sync::Arc::new(crate::db::request_history::RequestHistoryStorage::new(
        pool.clone(),
    ));
    let capture = cancellation_capture(storage.clone(), cancellation_attribution());
    assert!(capture
        .accept("fixture", RequestType::Chat, json!({}))
        .await
        .is_err());
    drop(capture);
    storage.drain_usage_writers().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM usage_receipts")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM request_history")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn normal_admission_drop_settles_once_and_cannot_replace_an_existing_terminal_result() {
    let pool = crate::db::test_utils::test_db_pool().await;
    let storage = std::sync::Arc::new(crate::db::request_history::RequestHistoryStorage::new(
        pool.clone(),
    ));
    let attribution = cancellation_attribution();
    let capture = cancellation_capture(storage.clone(), attribution.clone());
    capture
        .accept("fixture", RequestType::Chat, json!({}))
        .await
        .unwrap();
    let mut record = capture.state().record.clone().unwrap();
    record.status = crate::common::protocol::RecordStatus::Success;
    storage
        .finish_usage(
            &attribution,
            &record,
            &UsageTokens::provider(Some(2), Some(3), Some(5)),
        )
        .await
        .unwrap();
    drop(capture);
    storage.drain_usage_writers().await.unwrap();
    let state: (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT request_count,pending_requests,total_tokens,provider_count FROM usage_daily",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(state, (1, 0, 5, 1));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT successful_requests FROM usage_daily")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn completed_admission_without_a_response_body_is_missing_once_on_drop() {
    let pool = crate::db::test_utils::test_db_pool().await;
    let storage = std::sync::Arc::new(crate::db::request_history::RequestHistoryStorage::new(
        pool.clone(),
    ));
    let capture = cancellation_capture(storage.clone(), cancellation_attribution());
    capture
        .accept("fixture", RequestType::Chat, json!({}))
        .await
        .unwrap();
    drop(capture);
    storage.drain_usage_writers().await.unwrap();
    let state: (i64, i64, i64) =
        sqlx::query_as("SELECT request_count,pending_requests,missing_count FROM usage_daily")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(state, (1, 0, 1));
}

#[tokio::test]
async fn native_anthropic_openai_and_responses_structured_sse_errors_are_error_outcomes() {
    for event in [
        json!({"type":"error","error":{"type":"api_error","message":"native failure"}}),
        json!({"error":{"message":"openai failure","type":"server_error"}}),
        json!({"type":"error","message":"explicit root error type"}),
        json!({"type":"response.failed","response":{"error":{"code":"server_error","message":"responses failure"}}}),
    ] {
        let (app, state, _) = fixture(false).await;
        let wire = format!("data: {event}\n\ndata: {{\"type\":\"message_stop\"}}\n\n");
        let response = request(&app, json!({"mode":"sse_wire","wire":wire})).await;
        let actual = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(
            actual,
            wire.as_bytes(),
            "metadata observation preserves exact wire bytes"
        );
        state
            .balancer
            .request_history
            .drain_usage_writers()
            .await
            .unwrap();
        let result: (Option<String>, String) =
            sqlx::query_as("SELECT outcome,token_source FROM usage_receipts")
                .fetch_one(&state.db_pool)
                .await
                .unwrap();
        assert_eq!(
            result,
            (Some("error".to_string()), "missing".to_string()),
            "{event}"
        );
    }
}

#[tokio::test]
async fn quoted_and_nested_error_text_never_changes_a_successful_inference_outcome() {
    for event in [
        json!({"choices":[{"delta":{"content":"error response.failed {\"type\":\"error\",\"error\":{}}"}}]}),
        json!({"data":{"error":{"message":"business payload"},"type":"error"}}),
        json!({"type":"response.completed","response":{"output":[{"type":"error","error":{"message":"quoted document"}}]}}),
    ] {
        let (app, state, _) = fixture(false).await;
        let wire = format!("data: {event}\n\ndata: [DONE]\n\n");
        let response = request(&app, json!({"mode":"sse_wire","wire":wire})).await;
        let actual = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(actual, wire.as_bytes());
        state
            .balancer
            .request_history
            .drain_usage_writers()
            .await
            .unwrap();
        let outcome: Option<String> = sqlx::query_scalar("SELECT outcome FROM usage_receipts")
            .fetch_one(&state.db_pool)
            .await
            .unwrap();
        assert_eq!(outcome, Some("success".to_string()), "{event}");
    }
}

#[tokio::test]
async fn structured_sse_error_retains_valid_partial_provider_tokens() {
    let (app, state, _) = fixture(false).await;
    let event =
        json!({"error":{"message":"failed after input processing"},"usage":{"prompt_tokens":2}});
    let response = request(
        &app,
        json!({"mode":"sse_wire","wire":format!("data: {event}\n\n")}),
    )
    .await;
    to_bytes(response.into_body(), usize::MAX).await.unwrap();
    state
        .balancer
        .request_history
        .drain_usage_writers()
        .await
        .unwrap();
    let state:(Option<String>,String,i64,i64,i64) = sqlx::query_as(
        "SELECT r.outcome,r.token_source,d.input_tokens,d.output_missing_count,d.total_missing_count
         FROM usage_receipts r JOIN usage_daily d ON d.date=r.date
         AND d.organization_id=r.organization_id AND d.account_id=r.account_id",
    ).fetch_one(&state.db_pool).await.unwrap();
    assert_eq!(
        state,
        (Some("error".to_string()), "provider".to_string(), 2, 1, 1)
    );
}
