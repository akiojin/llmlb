//! Issue #947: durable request usage survives raw retention and repeated saves.

use super::*;

fn record(timestamp: DateTime<Utc>) -> RequestResponseRecord {
    let mut record = RequestResponseRecord::new(
        Uuid::new_v4(),
        "usage fixture".into(),
        "127.0.0.1".parse().unwrap(),
        "usage-model".into(),
        RequestType::Chat,
        serde_json::json!({"messages": []}),
        axum::http::StatusCode::OK,
        std::time::Duration::from_millis(1),
        None,
        None,
    );
    record.timestamp = timestamp;
    record.input_tokens = Some(11);
    record.output_tokens = Some(7);
    record.total_tokens = Some(18);
    record
}

#[tokio::test]
async fn usage_rollup_survives_cleanup_and_repeated_save() {
    let pool = crate::db::test_utils::test_db_pool().await;
    let storage = RequestHistoryStorage::new(pool.clone());
    let record = record(Utc::now() - Duration::days(10));
    storage.save_record(&record).await.unwrap();
    let totals: (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT SUM(request_count), SUM(input_tokens), SUM(output_tokens), SUM(total_tokens) FROM usage_daily",
    )
    .fetch_one(&pool)
    .await
    .expect("accepted request must create a durable usage rollup");
    assert_eq!(totals, (1, 11, 7, 18));

    storage
        .cleanup_old_records(Duration::days(7))
        .await
        .unwrap();
    assert!(storage.load_records().await.unwrap().is_empty());
    let reopened = RequestHistoryStorage::new(pool.clone());
    reopened.save_record(&record).await.unwrap();
    reopened.save_record(&record).await.unwrap();
    let totals: (i64, i64) =
        sqlx::query_as("SELECT SUM(request_count), SUM(total_tokens) FROM usage_daily")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        totals,
        (1, 18),
        "re-saving the same logical request must not add usage"
    );
}

#[tokio::test]
async fn usage_rollup_uses_utc_receipt_date_and_unattributed_legacy_bucket() {
    let pool = crate::db::test_utils::test_db_pool().await;
    let storage = RequestHistoryStorage::new(pool.clone());
    let timestamp = "2026-10-01T23:59:59Z".parse().unwrap();
    let mut record = record(timestamp);
    record.completed_at = "2026-10-02T00:10:00Z".parse().unwrap();
    storage.save_record(&record).await.unwrap();
    let bucket: (String, String, String) =
        sqlx::query_as("SELECT date, organization_id, account_id FROM usage_daily")
            .fetch_one(&pool)
            .await
            .expect("usage bucket must be persisted independently of request history");
    assert_eq!(bucket, ("2026-10-01".into(), String::new(), String::new()));
}

#[tokio::test]
async fn usage_rollup_late_initial_save_cannot_downgrade_terminal_observation() {
    use crate::common::usage::{TokenSource, UsageAttribution, UsageTokens};
    let pool = crate::db::test_utils::test_db_pool().await;
    let storage = RequestHistoryStorage::new(pool.clone());
    let mut record = record(Utc::now() - Duration::days(10));
    let attribution = UsageAttribution {
        request_id: record.id,
        received_at: record.timestamp,
        organization_id: Some(Uuid::new_v4()),
        account_id: Some(Uuid::new_v4()),
        api_key_id: None,
        auth_source: "jwt".into(),
    };
    let tokens = UsageTokens::from_usage(
        &crate::token::TokenUsage::new(Some(11), Some(7), Some(18)),
        TokenSource::Provider,
    );
    storage
        .save_attributed_record(&record, &attribution, &tokens, true)
        .await
        .unwrap();
    storage
        .cleanup_old_records(Duration::days(7))
        .await
        .unwrap();
    record.endpoint_id = Uuid::nil();
    record.input_tokens = None;
    record.output_tokens = None;
    record.total_tokens = None;
    storage
        .save_attributed_record(&record, &attribution, &UsageTokens::default(), false)
        .await
        .unwrap();
    let row: (i64, i64, i64, String, String) = sqlx::query_as(
        "SELECT request_count, total_tokens, successful_requests, organization_id, account_id FROM usage_daily",
    ).fetch_one(&pool).await.unwrap();
    assert_eq!((row.0, row.1, row.2), (1, 18, 1));
    assert_eq!(row.3, attribution.organization_id.unwrap().to_string());
    assert_eq!(row.4, attribution.account_id.unwrap().to_string());
    assert!(
        storage.get_record_by_id(record.id).await.unwrap().is_none(),
        "late saves must not resurrect expired raw conversations"
    );
}

#[tokio::test]
async fn usage_rollup_raw_metadata_enriches_at_terminal_and_survives_late_initial_save() {
    let pool = crate::db::test_utils::test_db_pool().await;
    let storage = RequestHistoryStorage::new(pool.clone());
    let mut admission = record(Utc::now());
    admission.endpoint_id = Uuid::nil();
    admission.endpoint_name = "N/A".into();
    admission.endpoint_ip = "0.0.0.0".parse().unwrap();
    admission.client_ip = None;
    admission.api_key_id = Some(Uuid::new_v4());
    admission.input_tokens = None;
    admission.output_tokens = None;
    admission.total_tokens = None;
    let attribution = UsageAttribution {
        request_id: admission.id,
        received_at: admission.timestamp,
        organization_id: Some(Uuid::new_v4()),
        account_id: Some(Uuid::new_v4()),
        api_key_id: admission.api_key_id,
        auth_source: "api_key".into(),
    };
    storage
        .save_attributed_record(&admission, &attribution, &UsageTokens::default(), false)
        .await
        .unwrap();

    let mut terminal = admission.clone();
    terminal.endpoint_id = Uuid::new_v4();
    terminal.endpoint_name = "final-target".into();
    terminal.endpoint_ip = "198.51.100.20".parse().unwrap();
    terminal.client_ip = Some("203.0.113.9".parse().unwrap());
    terminal.input_tokens = Some(11);
    terminal.output_tokens = Some(7);
    terminal.total_tokens = Some(18);
    let tokens = UsageTokens::from_usage(
        &crate::token::TokenUsage::new(Some(11), Some(7), Some(18)),
        TokenSource::Provider,
    );
    storage
        .save_attributed_record(&terminal, &attribution, &tokens, true)
        .await
        .unwrap();
    let after_terminal = storage
        .get_record_by_id(admission.id)
        .await
        .unwrap()
        .unwrap();

    // An earlier attempt may finish saving after the terminal transaction.
    // Its target and conflicting known IPs must not replace terminal metadata.
    let mut late_initial = admission.clone();
    late_initial.endpoint_id = Uuid::new_v4();
    late_initial.endpoint_name = "stale-target".into();
    late_initial.endpoint_ip = "198.51.100.30".parse().unwrap();
    late_initial.client_ip = Some("203.0.113.99".parse().unwrap());
    storage
        .save_attributed_record(&late_initial, &attribution, &UsageTokens::default(), false)
        .await
        .unwrap();
    let after_late_initial = storage
        .get_record_by_id(admission.id)
        .await
        .unwrap()
        .unwrap();

    // Metadata enrichment never changes the authenticated admission authority.
    for (table, time_column) in [
        ("request_history", "timestamp"),
        ("usage_receipts", "received_at"),
    ] {
        let identity: (String, Option<String>, Option<String>, Option<String>) = sqlx::query_as(
            &format!(
                "SELECT {time_column}, organization_id, account_id, api_key_id FROM {table} WHERE id = ?"
            ),
        )
        .bind(admission.id.to_string())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            identity,
            (
                attribution.received_at.to_rfc3339(),
                attribution.organization_id.map(|id| id.to_string()),
                attribution.account_id.map(|id| id.to_string()),
                attribution.api_key_id.map(|id| id.to_string()),
            ),
            "raw and receipt must retain the same admission authority: {table}"
        );
    }
    let receipt: (String, bool, bool, Option<i64>, Option<i64>, Option<i64>) = sqlx::query_as(
        "SELECT endpoint_id, finalized, successful, input_tokens, output_tokens, total_tokens FROM usage_receipts WHERE id = ?",
    )
    .bind(admission.id.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        receipt,
        (
            terminal.endpoint_id.to_string(),
            true,
            true,
            Some(11),
            Some(7),
            Some(18)
        )
    );
    let totals: (i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), SUM(request_count), SUM(successful_requests), SUM(failed_requests), SUM(total_tokens) FROM usage_daily",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(totals, (1, 1, 1, 0, 18));
    let bucket: (String, String, String, String) =
        sqlx::query_as("SELECT date, organization_id, account_id, endpoint_id FROM usage_daily")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        bucket,
        (
            attribution.received_at.format("%Y-%m-%d").to_string(),
            attribution.organization_id.unwrap().to_string(),
            attribution.account_id.unwrap().to_string(),
            terminal.endpoint_id.to_string(),
        )
    );

    let metadata = |record: &RequestResponseRecord| {
        (
            record.endpoint_id,
            record.endpoint_name.clone(),
            record.endpoint_ip,
            record.client_ip,
        )
    };
    let expected = metadata(&terminal);
    assert_eq!(
        (metadata(&after_terminal), metadata(&after_late_initial)),
        (expected.clone(), expected),
        "terminal must fill missing IPs and target; late initial saves must retain the first known IPs and finalized target"
    );
}

#[tokio::test]
async fn usage_rollup_and_raw_save_rollback_together() {
    let pool = crate::db::test_utils::test_db_pool().await;
    let storage = RequestHistoryStorage::new(pool.clone());
    sqlx::query("CREATE TRIGGER reject_usage_fixture BEFORE INSERT ON request_history BEGIN SELECT RAISE(ABORT, 'fixture write failure'); END")
        .execute(&pool).await.unwrap();
    assert!(storage.save_record(&record(Utc::now())).await.is_err());
    for table in ["usage_receipts", "usage_daily", "request_history"] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            count, 0,
            "raw and usage must share one transaction: {table}"
        );
    }
}

#[tokio::test]
async fn usage_rollup_survives_file_database_restart_and_deleted_account() {
    use crate::common::auth::UserRole;
    use crate::common::usage::{TokenSource, UsageAttribution, UsageTokens};
    let directory = tempfile::tempdir().unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(directory.path().join("usage-restart.db"))
        .create_if_missing(true);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options.clone())
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let organization = crate::db::organizations::create(&pool, "restart organization")
        .await
        .unwrap();
    let organization_id = Uuid::parse_str(&organization.id).unwrap();
    let account = crate::db::users::create(
        &pool,
        "deleted-usage@example.com",
        "hash",
        UserRole::Viewer,
        false,
    )
    .await
    .unwrap();
    let storage = RequestHistoryStorage::new(pool.clone());
    let record = record(Utc::now() - Duration::days(10));
    let attribution = UsageAttribution {
        request_id: record.id,
        received_at: record.timestamp,
        organization_id: Some(organization_id),
        account_id: Some(account.id),
        api_key_id: None,
        auth_source: "jwt".into(),
    };
    let tokens = UsageTokens::from_usage(
        &crate::token::TokenUsage::new(Some(11), Some(7), Some(18)),
        TokenSource::Provider,
    );
    storage
        .save_attributed_record(&record, &attribution, &tokens, true)
        .await
        .unwrap();
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(account.id.to_string())
        .execute(&pool)
        .await
        .unwrap();
    crate::db::organizations::archive(&pool, organization_id)
        .await
        .unwrap();
    storage
        .cleanup_old_records(Duration::days(7))
        .await
        .unwrap();
    drop(storage);
    pool.close().await;

    let reopened = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&reopened).await.unwrap();
    let storage = RequestHistoryStorage::new(reopened.clone());
    storage
        .save_attributed_record(&record, &attribution, &tokens, true)
        .await
        .unwrap();
    assert!(storage.load_records().await.unwrap().is_empty());
    let persisted: (String, String, i64, i64) = sqlx::query_as(
        "SELECT organization_id, account_id, request_count, total_tokens FROM usage_daily",
    )
    .fetch_one(&reopened)
    .await
    .unwrap();
    assert_eq!(
        persisted,
        (organization.id.to_string(), account.id.to_string(), 1, 18)
    );
    drop(storage);
    reopened.close().await;
}

#[tokio::test]
async fn usage_restart_recovers_abandoned_receipts_once() {
    use crate::common::usage::{TokenSource, UsageAttribution, UsageTokens};
    let directory = tempfile::tempdir().unwrap();
    for remove_raw in [false, true] {
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(directory.path().join(format!("abandoned-{remove_raw}.db")))
            .create_if_missing(true);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone())
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let storage = RequestHistoryStorage::new(pool.clone());
        let record = record(Utc::now() - Duration::days(10));
        let attribution = UsageAttribution {
            request_id: record.id,
            received_at: record.timestamp,
            organization_id: Some(Uuid::new_v4()),
            account_id: Some(Uuid::new_v4()),
            api_key_id: None,
            auth_source: "jwt".into(),
        };
        let observed = UsageTokens::from_usage(
            &crate::token::TokenUsage::new(Some(11), None, None),
            TokenSource::Provider,
        );
        storage
            .save_attributed_record(&record, &attribution, &observed, false)
            .await
            .unwrap();
        if remove_raw {
            storage
                .cleanup_old_records(Duration::days(7))
                .await
                .unwrap();
        }
        drop(storage);
        pool.close().await;

        let reopened = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let storage = RequestHistoryStorage::new(reopened.clone());
        assert_eq!(storage.recover_interrupted_usage().await.unwrap(), 1);
        assert_eq!(storage.recover_interrupted_usage().await.unwrap(), 0);
        let pending: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM usage_receipts WHERE finalized = 0")
                .fetch_one(&reopened)
                .await
                .unwrap();
        assert_eq!(
            pending, 0,
            "a restarted process must not report abandoned admissions as permanently active"
        );
        let totals: (i64, i64, i64, i64, i64) = sqlx::query_as("SELECT request_count, failed_requests, input_tokens, total_tokens, missing_count FROM usage_daily")
            .fetch_one(&reopened).await.unwrap();
        assert_eq!(
            totals,
            (1, 1, 11, 0, 1),
            "restart preserves observed fields without inventing missing terminal counts"
        );
        let raw = storage.get_record_by_id(record.id).await.unwrap();
        if remove_raw {
            assert!(raw.is_none());
        } else {
            assert!(matches!(raw.unwrap().status, RecordStatus::Error { .. }));
        }
        drop(storage);
        reopened.close().await;
    }
}

#[tokio::test]
async fn usage_rollup_http_responses_and_input_rejection() {
    use crate::types::endpoint::{Endpoint, EndpointModel, EndpointStatus, EndpointType};
    use axum::{body::to_bytes, body::Body, http::Request};
    use tower::ServiceExt;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let state = crate::db::test_utils::TestAppStateBuilder::new()
        .await
        .build()
        .await;
    let pool = state.db_pool.clone();
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "fixture-response", "object": "response", "status": "completed", "error": null,
            "usage": {"input_tokens": 12, "output_tokens": 8, "total_tokens": 20}
        })))
        .mount(&server)
        .await;
    let mut endpoint = Endpoint::new("usage-responses".into(), server.uri(), EndpointType::Vllm);
    endpoint.status = EndpointStatus::Online;
    let endpoint_id = endpoint.id;
    state
        .balancer
        .endpoint_registry
        .add(endpoint)
        .await
        .unwrap();
    state
        .balancer
        .endpoint_registry
        .add_model(&EndpointModel {
            endpoint_id,
            model_id: "usage-responses-model".into(),
            capabilities: None,
            max_tokens: None,
            last_checked: None,
            supported_apis: vec![],
            canonical_name: None,
        })
        .await
        .unwrap();
    let app = crate::api::create_app(state);
    for (payload, expected_status) in [
        (
            serde_json::json!({"model": "usage-responses-model", "input": "hello"}),
            axum::http::StatusCode::OK,
        ),
        (
            serde_json::json!({"input": "model is missing"}),
            axum::http::StatusCode::BAD_REQUEST,
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/responses")
                    .header("authorization", "Bearer sk_debug")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected_status);
        to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    }
    let totals = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let totals: (i64, i64) = sqlx::query_as(
                "SELECT COALESCE(SUM(request_count),0), COALESCE(SUM(total_tokens),0) FROM usage_daily",
            ).fetch_one(&pool).await.expect("Responses route must persist durable usage");
            if totals.1 == 20 { break totals; }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.expect("Responses usage must settle after body consumption");
    assert_eq!(
        totals,
        (1, 20),
        "input validation rejections must not count as inference"
    );
    let outcomes: (i64, i64) =
        sqlx::query_as("SELECT SUM(successful_requests), SUM(failed_requests) FROM usage_daily")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        outcomes,
        (1, 0),
        "Responses error:null is a successful result"
    );
    let successful: bool = sqlx::query_scalar("SELECT successful FROM usage_receipts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(successful);
    let raw = RequestHistoryStorage::new(pool)
        .load_records()
        .await
        .unwrap();
    assert_eq!(raw.len(), 1);
    assert!(matches!(raw[0].status, RecordStatus::Success));
}
