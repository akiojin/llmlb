//! Issue #950: typed global inference configuration and management boundaries.

#[path = "support/mod.rs"]
mod support;

use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
    Router,
};
use llmlb::{common::auth::UserRole, db::settings::SettingsStorage};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use tower::ServiceExt;

const URI: &str = "/api/dashboard/inference-defaults";
const KEY: &str = "inference.defaults";

async fn user(pool: &SqlitePool, name: &str, role: UserRole, password_change: bool) -> String {
    let user = llmlb::db::users::create(pool, name, "unused", role, password_change)
        .await
        .unwrap();
    llmlb::auth::jwt::create_jwt(
        &user.id.to_string(),
        role,
        &support::lb::test_jwt_secret(),
        password_change,
        0,
    )
    .unwrap()
}

async fn fixture() -> (Router, SqlitePool, String) {
    let pool = support::lb::create_test_db_pool().await;
    let jwt = user(&pool, "admin", UserRole::Admin, false).await;
    for (name, caps) in [
        (
            "text-a",
            vec![llmlb::types::ModelCapability::TextGeneration],
        ),
        (
            "embedding-only",
            vec![llmlb::types::ModelCapability::Embedding],
        ),
        (
            "image-only",
            vec![llmlb::types::ModelCapability::ImageGeneration],
        ),
        (
            "speech-only",
            vec![llmlb::types::ModelCapability::SpeechToText],
        ),
        (
            "tts-only",
            vec![llmlb::types::ModelCapability::TextToSpeech],
        ),
        (
            "openai:cloud-text",
            vec![llmlb::types::ModelCapability::TextGeneration],
        ),
        (
            "openai:cloud-embedding",
            vec![llmlb::types::ModelCapability::Embedding],
        ),
    ] {
        llmlb::db::models::ModelStorage::new(pool.clone())
            .save_model(&llmlb::registry::models::ModelInfo::with_capabilities(
                name.into(),
                0,
                String::new(),
                0,
                vec![],
                caps,
            ))
            .await
            .unwrap();
    }
    let app = support::lb::build_test_router(pool.clone()).await;
    (app, pool, jwt)
}

async fn send(
    app: &Router,
    method: Method,
    uri: &str,
    jwt: Option<&str>,
    value: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(uri);
    if let Some(jwt) = jwt {
        request = request.header("authorization", format!("Bearer {jwt}"));
    }
    let body = if let Some(value) = value {
        request = request.header("content-type", "application/json");
        Body::from(value.to_string())
    } else {
        Body::empty()
    };
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn defaults_roundtrip_persists_across_router_restart_and_clear() {
    let (app, pool, jwt) = fixture().await;
    assert_eq!(
        send(&app, Method::GET, URI, Some(&jwt), None).await,
        (StatusCode::OK, json!({"default_text_model":null}))
    );
    let desired = json!({"default_text_model":"text-a"});
    assert_eq!(
        send(&app, Method::PUT, URI, Some(&jwt), Some(desired.clone())).await,
        (StatusCode::OK, desired.clone())
    );
    let stored = SettingsStorage::new(pool.clone())
        .get_setting(KEY)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&stored).unwrap(),
        json!({"version":1,"default_text_model":"text-a"})
    );
    let restarted = support::lb::build_test_router(pool.clone()).await;
    assert_eq!(
        send(&restarted, Method::GET, URI, Some(&jwt), None).await,
        (StatusCode::OK, desired)
    );
    assert_eq!(
        send(
            &restarted,
            Method::PUT,
            URI,
            Some(&jwt),
            Some(json!({"default_text_model":null}))
        )
        .await,
        (StatusCode::OK, json!({"default_text_model":null}))
    );
}

#[tokio::test]
async fn invalid_wire_or_model_preserves_previous_committed_value() {
    let (app, _pool, jwt) = fixture().await;
    let desired = json!({"default_text_model":"text-a"});
    assert_eq!(
        send(&app, Method::PUT, URI, Some(&jwt), Some(desired.clone()))
            .await
            .0,
        StatusCode::OK
    );
    for invalid in [
        json!({}),
        json!({"default_text_model":1}),
        json!({"default_text_model":false}),
        json!({"default_text_model":[]}),
        json!({"default_text_model":{}}),
        json!({"default_text_model":null,"unknown":true}),
        json!(null),
        json!([]),
        json!({"default_text_model":""}),
        json!({"default_text_model":" text-a"}),
        json!({"default_text_model":"text-a\n"}),
        json!({"default_text_model":"te\u{0000}xt"}),
        json!({"default_text_model":"unknown-local"}),
        json!({"default_text_model":"embedding-only"}),
        json!({"default_text_model":"image-only"}),
        json!({"default_text_model":"speech-only"}),
        json!({"default_text_model":"tts-only"}),
        json!({"default_text_model":"openai:"}),
    ] {
        let (status, body) = send(&app, Method::PUT, URI, Some(&jwt), Some(invalid.clone())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{invalid}: {body}");
        assert_eq!(
            send(&app, Method::GET, URI, Some(&jwt), None).await,
            (StatusCode::OK, desired.clone())
        );
    }
}

#[tokio::test]
async fn global_admin_jwt_and_password_change_are_required_for_both_operations() {
    let (app, pool, admin) = fixture().await;
    let viewer = user(&pool, "viewer", UserRole::Viewer, false).await;
    let must_change = user(&pool, "must-change", UserRole::Admin, true).await;
    for method in [Method::GET, Method::PUT] {
        let value = (method == Method::PUT).then(|| json!({"default_text_model":null}));
        for (jwt, expected) in [
            (None, StatusCode::UNAUTHORIZED),
            (Some("sk_debug"), StatusCode::UNAUTHORIZED),
            (Some(viewer.as_str()), StatusCode::FORBIDDEN),
            (Some(must_change.as_str()), StatusCode::FORBIDDEN),
        ] {
            let (status, body) = send(&app, method.clone(), URI, jwt, value.clone()).await;
            assert_eq!(status, expected, "{method}: {body}");
        }
        assert_eq!(
            send(&app, method, URI, Some(&admin), value).await.0,
            StatusCode::OK
        );
    }
}

#[tokio::test]
async fn cookie_put_requires_csrf_and_generic_settings_cannot_bypass_validation() {
    let (app, pool, admin) = fixture().await;
    let viewer = user(&pool, "viewer", UserRole::Viewer, false).await;
    for csrf in [false, true] {
        let mut request = Request::builder()
            .method(Method::PUT)
            .uri(URI)
            .header("content-type", "application/json")
            .header("host", "localhost")
            .header("origin", "http://localhost")
            .header("cookie", format!("llmlb_jwt={admin}; llmlb_csrf=token"));
        if csrf {
            request = request.header("x-csrf-token", "token");
        }
        let response = app
            .clone()
            .oneshot(
                request
                    .body(Body::from(json!({"default_text_model":null}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            if csrf {
                StatusCode::OK
            } else {
                StatusCode::FORBIDDEN
            }
        );
    }
    let uri = format!("/api/dashboard/settings/{KEY}");
    for jwt in [&admin, &viewer] {
        assert_eq!(
            send(&app, Method::GET, &uri, Some(jwt), None).await.0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            send(
                &app,
                Method::PUT,
                &uri,
                Some(jwt),
                Some(json!({"value":"bypass"}))
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
}

#[tokio::test]
async fn corrupt_storage_and_write_failure_are_safe_errors_without_replacing_old_value() {
    let (app, pool, jwt) = fixture().await;
    let storage = SettingsStorage::new(pool.clone());
    for raw in [
        "not-json",
        "null",
        "{}",
        r#"{"version":2,"default_text_model":"text-a"}"#,
        r#"{"version":1}"#,
        r#"{"version":1,"default_text_model":""}"#,
        r#"{"version":1,"default_text_model":null,"extra":true}"#,
        r#"{"version":1,"default_text_model":123}"#,
    ] {
        storage.set_setting(KEY, raw).await.unwrap();
        let (status, body) = send(&app, Method::GET, URI, Some(&jwt), None).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{raw}: {body}");
        assert!(!body.to_string().contains(raw));
    }
    storage
        .set_setting(KEY, r#"{"version":1,"default_text_model":"text-a"}"#)
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER reject_inference_update BEFORE INSERT ON settings BEGIN SELECT RAISE(ABORT, 'private-db-failure'); END")
        .execute(&pool).await.unwrap();
    let (status, body) = send(
        &app,
        Method::PUT,
        URI,
        Some(&jwt),
        Some(json!({"default_text_model":null})),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert!(!body.to_string().contains("private-db-failure"));
    assert_eq!(
        send(&app, Method::GET, URI, Some(&jwt), None).await,
        (StatusCode::OK, json!({"default_text_model":"text-a"}))
    );
}

#[tokio::test]
async fn offline_synced_text_model_and_its_canonical_alias_can_be_saved_without_probe() {
    use llmlb::types::endpoint::{Endpoint, EndpointModel, EndpointStatus, SupportedAPI};
    let (_app, pool, jwt) = fixture().await;
    let upstream = wiremock::MockServer::start().await;
    let mut endpoint = Endpoint::new(
        "offline".into(),
        upstream.uri(),
        llmlb::types::endpoint::EndpointType::Xllm,
    );
    endpoint.status = EndpointStatus::Offline;
    llmlb::db::endpoints::create_endpoint(&pool, &endpoint)
        .await
        .unwrap();
    let mut model = EndpointModel {
        endpoint_id: endpoint.id,
        model_id: "runtime-text".into(),
        capabilities: Some(vec!["text_generation".into()]),
        max_tokens: None,
        last_checked: None,
        supported_apis: vec![SupportedAPI::ChatCompletions],
        canonical_name: Some("publisher/text-model".into()),
    };
    llmlb::db::endpoints::add_endpoint_model(&pool, &model)
        .await
        .unwrap();
    model.model_id = "runtime-embedding".into();
    model.canonical_name = None;
    model.capabilities = Some(vec!["embedding".into()]);
    model.supported_apis = vec![SupportedAPI::Embeddings];
    llmlb::db::endpoints::add_endpoint_model(&pool, &model)
        .await
        .unwrap();
    let app = support::lb::build_test_router(pool).await;
    for name in ["runtime-text", "publisher/text-model"] {
        let desired = json!({"default_text_model":name});
        assert_eq!(
            send(&app, Method::PUT, URI, Some(&jwt), Some(desired.clone())).await,
            (StatusCode::OK, desired)
        );
    }
    assert_eq!(
        send(
            &app,
            Method::PUT,
            URI,
            Some(&jwt),
            Some(json!({"default_text_model":"runtime-embedding"}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert!(upstream.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
#[serial_test::serial]
async fn cloud_provider_requires_existing_configuration_and_never_probes_network_on_save() {
    struct RestoreEnv(&'static str, Option<std::ffi::OsString>);
    impl Drop for RestoreEnv {
        fn drop(&mut self) {
            if let Some(value) = self.1.as_ref() {
                std::env::set_var(self.0, value);
            } else {
                std::env::remove_var(self.0);
            }
        }
    }
    let _key = RestoreEnv("OPENAI_API_KEY", std::env::var_os("OPENAI_API_KEY"));
    let _base = RestoreEnv("OPENAI_BASE_URL", std::env::var_os("OPENAI_BASE_URL"));
    std::env::remove_var("OPENAI_API_KEY");
    let (app, _pool, jwt) = fixture().await;
    let desired = json!({"default_text_model":"openai:cloud-text"});
    assert_eq!(
        send(&app, Method::PUT, URI, Some(&jwt), Some(desired.clone()))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let upstream = wiremock::MockServer::start().await;
    std::env::set_var("OPENAI_API_KEY", "test-only-cloud-key");
    std::env::set_var("OPENAI_BASE_URL", upstream.uri());
    for invalid in ["openai:unknown-text", "openai:cloud-embedding"] {
        let (status, body) = send(
            &app,
            Method::PUT,
            URI,
            Some(&jwt),
            Some(json!({"default_text_model":invalid})),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{invalid}: {body}");
    }
    assert_eq!(
        send(&app, Method::PUT, URI, Some(&jwt), Some(desired.clone())).await,
        (StatusCode::OK, desired)
    );
    assert!(upstream.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn defaults_survive_closing_and_reopening_the_sqlite_database() {
    let directory = tempfile::tempdir().unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(directory.path().join("inference-defaults.db"))
        .create_if_missing(true);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options.clone())
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let jwt = user(&pool, "admin", UserRole::Admin, false).await;
    llmlb::db::models::ModelStorage::new(pool.clone())
        .save_model(&llmlb::registry::models::ModelInfo::new(
            "persisted-model".into(),
            0,
            String::new(),
            0,
            vec![],
        ))
        .await
        .unwrap();
    let app = support::lb::build_test_router(pool.clone()).await;
    let desired = json!({"default_text_model":"persisted-model"});
    assert_eq!(
        send(&app, Method::PUT, URI, Some(&jwt), Some(desired.clone())).await,
        (StatusCode::OK, desired.clone())
    );
    drop(app);
    pool.close().await;
    let reopened = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    let restarted = support::lb::build_test_router(reopened.clone()).await;
    assert_eq!(
        send(&restarted, Method::GET, URI, Some(&jwt), None).await,
        (StatusCode::OK, desired)
    );
    drop(restarted);
    reopened.close().await;
}

#[tokio::test]
async fn concurrent_puts_only_publish_complete_committed_values() {
    let (app, pool, jwt) = fixture().await;
    llmlb::db::models::ModelStorage::new(pool.clone())
        .save_model(&llmlb::registry::models::ModelInfo::new(
            "text-b".into(),
            0,
            String::new(),
            0,
            vec![],
        ))
        .await
        .unwrap();
    let a = json!({"default_text_model":"text-a"});
    let b = json!({"default_text_model":"text-b"});
    let (first, second) = tokio::join!(
        send(&app, Method::PUT, URI, Some(&jwt), Some(a.clone())),
        send(&app, Method::PUT, URI, Some(&jwt), Some(b.clone())),
    );
    assert_eq!(first, (StatusCode::OK, a.clone()));
    assert_eq!(second, (StatusCode::OK, b.clone()));
    let (status, last) = send(&app, Method::GET, URI, Some(&jwt), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(last == a || last == b, "{last}");
    let raw = SettingsStorage::new(pool)
        .get_setting(KEY)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&raw).unwrap(),
        json!({"version":1,"default_text_model":last["default_text_model"]})
    );
}

#[tokio::test]
async fn settings_read_failure_is_safe_and_does_not_look_unset() {
    let (app, pool, jwt) = fixture().await;
    sqlx::query("DROP TABLE settings")
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = send(&app, Method::GET, URI, Some(&jwt), None).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_ne!(body, json!({"default_text_model":null}));
    assert!(!body.to_string().contains("no such table"));
}

#[tokio::test]
async fn invalid_json_put_is_400_and_preserves_the_saved_value() {
    let (app, _pool, jwt) = fixture().await;
    let desired = json!({"default_text_model":"text-a"});
    assert_eq!(
        send(&app, Method::PUT, URI, Some(&jwt), Some(desired.clone()))
            .await
            .0,
        StatusCode::OK
    );
    for invalid in ["{", "{\"default_text_model\":", "undefined"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri(URI)
                    .header("authorization", format!("Bearer {jwt}"))
                    .header("content-type", "application/json")
                    .body(Body::from(invalid))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            send(&app, Method::GET, URI, Some(&jwt), None).await,
            (StatusCode::OK, desired.clone())
        );
    }
}
