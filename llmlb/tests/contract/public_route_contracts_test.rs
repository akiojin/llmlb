//! SPEC #585 FR-027 / Issue #806: 契約テストが無かった公開ルート 22 組の契約。
//!
//! 本番 Router への接続、HTTP status、主要な response shape に限定して検証する。
//! 資格情報の無い要求に対する契約（認証境界）は、`api_route_coverage_test.rs` の
//! 契約台帳が全ルートについて検証する。

use crate::api_route_coverage_test::{response_json, send_request, ScopedEnvVar};
use crate::support::http::TestServer;
use axum::http::{Method, StatusCode};
use llmlb::common::auth::UserRole;
use reqwest::Client;
use serde_json::{json, Value};
use serial_test::serial;
use sqlx::SqlitePool;
use std::time::Duration;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const MODEL: &str = "contract-model";
const ENDPOINT_NAME: &str = "route-contract-endpoint";
const ENDPOINT_LOG_PATH: &str = "/var/log/route-contract-endpoint.log";
const CATALOG_REPO: &str = "owner/contract-model-GGUF";

async fn create_user_jwt(db_pool: &SqlitePool, username: &str, role: UserRole) -> String {
    let password_hash = llmlb::auth::password::hash_password("password123").unwrap();
    let user = llmlb::db::users::create(db_pool, username, &password_hash, role, false)
        .await
        .expect("create route contract user");
    llmlb::auth::jwt::create_jwt(
        &user.id.to_string(),
        role,
        &crate::support::lb::test_jwt_secret(),
        false,
        0,
    )
    .expect("create route contract jwt")
}

fn assert_has_keys(value: &Value, keys: &[&str], context: &str) {
    for key in keys {
        assert!(
            value.get(key).is_some(),
            "{context} must include `{key}`; got {value}"
        );
    }
}

/// 上流（OpenAI 互換のモック）を 1 台登録し、モデルを同期済みにした load balancer。
///
/// 推論を転送するハンドラは接続元アドレスを必要とするため、実サーバーとして起動する。
struct LbWithEndpoint {
    server: TestServer,
    _upstream: MockServer,
    client: Client,
    admin_jwt: String,
    viewer_jwt: String,
    endpoint_id: String,
}

impl LbWithEndpoint {
    async fn spawn() -> Self {
        let upstream = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "object": "list",
                "data": [{ "id": MODEL, "object": "model" }]
            })))
            .mount(&upstream)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "chatcmpl-route-contract",
                "object": "chat.completion",
                "model": MODEL,
                "choices": [{
                    "index": 0,
                    "message": { "role": "assistant", "content": "pong" },
                    "finish_reason": "stop"
                }],
                "usage": { "prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2 }
            })))
            .mount(&upstream)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/logs"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "entries": [],
                "path": ENDPOINT_LOG_PATH
            })))
            .mount(&upstream)
            .await;

        let (server, db_pool) = crate::support::lb::spawn_test_lb_with_db().await;
        let admin_jwt = create_user_jwt(&db_pool, "route-contract-admin", UserRole::Admin).await;
        let viewer_jwt = create_user_jwt(&db_pool, "route-contract-viewer", UserRole::Viewer).await;
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("build route contract client");

        let registered = client
            .post(format!("http://{}/api/endpoints", server.addr()))
            .bearer_auth(&admin_jwt)
            .json(&json!({ "name": ENDPOINT_NAME, "base_url": upstream.uri() }))
            .send()
            .await
            .expect("register endpoint");
        assert_eq!(registered.status(), StatusCode::CREATED);
        let registered: Value = registered.json().await.expect("registered endpoint json");
        let endpoint_id = registered["id"]
            .as_str()
            .expect("registered endpoint id")
            .to_string();

        // ヘルスチェックで online にし（ルーティング対象は online のみ）、モデルを同期する。
        for action in ["test", "sync"] {
            let response = client
                .post(format!(
                    "http://{}/api/endpoints/{endpoint_id}/{action}",
                    server.addr()
                ))
                .bearer_auth(&admin_jwt)
                .send()
                .await
                .expect("prepare endpoint");
            assert_eq!(response.status(), StatusCode::OK, "endpoint {action}");
        }

        Self {
            server,
            _upstream: upstream,
            client,
            admin_jwt,
            viewer_jwt,
            endpoint_id,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.server.addr())
    }

    async fn get(&self, path: &str) -> reqwest::Response {
        self.client
            .get(self.url(path))
            .bearer_auth(&self.admin_jwt)
            .send()
            .await
            .unwrap_or_else(|error| panic!("GET {path}: {error}"))
    }

    /// admin として GET し、200 の JSON を返す。
    async fn get_json(&self, path: &str) -> Value {
        let response = self.get(path).await;
        assert_eq!(response.status(), StatusCode::OK, "GET {path}");
        response
            .json()
            .await
            .unwrap_or_else(|error| panic!("GET {path} must return JSON: {error}"))
    }

    /// 統計は応答の後に記録されるため、`ready` を満たすまで取得を繰り返す。
    async fn wait_for_json(&self, path: &str, ready: impl Fn(&Value) -> bool) -> Value {
        let mut last = Value::Null;
        for _ in 0..50 {
            last = self.get_json(path).await;
            if ready(&last) {
                return last;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("GET {path} did not reach the expected state; last response: {last}");
    }

    async fn chat(&self, path: &str, jwt: &str) -> reqwest::Response {
        self.client
            .post(self.url(path))
            .bearer_auth(jwt)
            .json(&json!({
                "model": MODEL,
                "messages": [{ "role": "user", "content": "ping" }]
            }))
            .send()
            .await
            .unwrap_or_else(|error| panic!("POST {path}: {error}"))
    }
}

fn is_non_empty_array(value: &Value) -> bool {
    value.as_array().is_some_and(|items| !items.is_empty())
}

const PLAYGROUND_CHAT: &str = "/api/dashboard/playground/chat/completions";
const PLAYGROUND_LOAD_TEST_CHAT: &str = "/api/dashboard/playground/load-test/chat/completions";

#[tokio::test]
#[serial]
async fn registration_route_creates_a_viewer_from_an_invitation_code() {
    let (app, db_pool) = crate::support::lb::create_test_lb_default_auth().await;
    let admin_jwt = create_user_jwt(&db_pool, "route-contract-admin", UserRole::Admin).await;

    let response = send_request(
        &app,
        Method::POST,
        "/api/invitations",
        Some(&admin_jwt),
        Some(json!({})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let invitation = response_json(response).await;

    let register = |username: &'static str, invitation_code: Value| {
        send_request(
            &app,
            Method::POST,
            "/api/auth/register",
            None,
            Some(json!({
                "invitation_code": invitation_code,
                "username": username,
                "password": "Password123"
            })),
        )
    };

    let response = register("route-contract-invitee", invitation["code"].clone()).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let user = response_json(response).await;
    assert!(user["id"]
        .as_str()
        .is_some_and(|id| Uuid::parse_str(id).is_ok()));
    assert_eq!(user["username"], "route-contract-invitee");
    assert_eq!(user["role"], "viewer");
    assert!(user["created_at"].is_string());

    // 招待コードは 1 回しか使えない。
    let response = register("route-contract-second-invitee", invitation["code"].clone()).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(response_json(response).await["error"].is_string());

    let response = register("route-contract-uninvited", json!("inv_unknown")).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(response_json(response).await["error"].is_string());
}

#[tokio::test]
#[serial]
async fn notification_settings_routes_serve_admins_only() {
    let (app, db_pool) = crate::support::lb::create_test_lb_default_auth().await;
    let admin_jwt = create_user_jwt(&db_pool, "route-contract-admin", UserRole::Admin).await;
    let viewer_jwt = create_user_jwt(&db_pool, "route-contract-viewer", UserRole::Viewer).await;
    let uri = "/api/dashboard/notifications";

    let response = send_request(&app, Method::GET, uri, Some(&admin_jwt), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let current = response_json(response).await;
    assert_has_keys(
        &current["settings"],
        &[
            "enabled",
            "smtp_host",
            "smtp_port",
            "smtp_from",
            "daily_digest_time",
            "language",
        ],
        "notification settings",
    );
    assert!(current["status"]["state"].is_string());
    assert!(current["credentials_configured"].is_boolean());
    assert!(current["recipients"].is_array());
    assert_has_keys(
        &current,
        &["last_digest_sent_date", "last_digest_error"],
        "notification settings response",
    );

    let mut settings = current["settings"].clone();
    settings["daily_digest_time"] = json!("08:30");
    let response = send_request(&app, Method::PUT, uri, Some(&admin_jwt), Some(settings)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let updated = response_json(response).await;
    assert_eq!(updated["settings"]["daily_digest_time"], "08:30");
    assert!(updated["status"]["state"].is_string());

    let response = send_request(&app, Method::GET, uri, Some(&viewer_jwt), None).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let response = send_request(
        &app,
        Method::PUT,
        uri,
        Some(&viewer_jwt),
        Some(current["settings"].clone()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[serial]
async fn playground_and_endpoint_proxy_routes_forward_chat_completions() {
    let lb = LbWithEndpoint::spawn().await;
    let endpoint_chat = format!("/api/endpoints/{}/chat/completions", lb.endpoint_id);

    for path in [
        PLAYGROUND_CHAT,
        PLAYGROUND_LOAD_TEST_CHAT,
        endpoint_chat.as_str(),
    ] {
        let response = lb.chat(path, &lb.admin_jwt).await;
        assert_eq!(response.status(), StatusCode::OK, "POST {path}");
        let completion: Value = response.json().await.expect("chat completion json");
        assert_eq!(completion["object"], "chat.completion", "POST {path}");
        assert_eq!(
            completion["choices"][0]["message"]["content"], "pong",
            "POST {path} must return the upstream completion"
        );
    }

    // Chat は viewer も使えるが、Load Test は admin ロール限定。
    let response = lb.chat(PLAYGROUND_CHAT, &lb.viewer_jwt).await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = lb.chat(PLAYGROUND_LOAD_TEST_CHAT, &lb.viewer_jwt).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let unknown_endpoint = format!("/api/endpoints/{}/chat/completions", Uuid::nil());
    let response = lb.chat(&unknown_endpoint, &lb.admin_jwt).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body: Value = response.json().await.expect("unknown endpoint error json");
    assert!(body["error"].is_string());
}

#[tokio::test]
#[serial]
async fn dashboard_summary_routes_report_endpoints_models_and_traffic() {
    let lb = LbWithEndpoint::spawn().await;
    // 推論を 1 回通し、統計を返すルートに要素を持たせる。
    let response = lb.chat(PLAYGROUND_CHAT, &lb.admin_jwt).await;
    assert_eq!(response.status(), StatusCode::OK);

    let endpoints = lb.get_json("/api/dashboard/endpoints").await;
    assert_eq!(endpoints.as_array().map(Vec::len), Some(1));
    let endpoint = &endpoints[0];
    assert_eq!(endpoint["id"], lb.endpoint_id.as_str());
    assert_eq!(endpoint["name"], ENDPOINT_NAME);
    assert_eq!(endpoint["status"], "online");
    assert_eq!(endpoint["model_count"], 1);
    assert_has_keys(
        endpoint,
        &[
            "base_url",
            "endpoint_type",
            "latency_ms",
            "total_requests",
            "successful_requests",
            "failed_requests",
        ],
        "dashboard endpoint",
    );

    let overview = lb.get_json("/api/dashboard/overview").await;
    assert_has_keys(
        &overview,
        &[
            "operations",
            "capacity",
            "endpoint_tps",
            "generated_at",
            "generation_time_ms",
        ],
        "dashboard overview",
    );
    assert_eq!(overview["endpoints"][0]["id"], lb.endpoint_id.as_str());
    assert_eq!(overview["operations"]["total_endpoints"], 1);
    assert!(overview["operations"]["health"].is_string());
    assert!(overview["action_items"].is_array());
    assert!(overview["history"].is_array());

    let history = lb.get_json("/api/dashboard/request-history").await;
    assert!(is_non_empty_array(&history));
    assert!(history[0]["minute"].is_string());
    assert!(history[0]["success"].is_u64());
    assert!(history[0]["error"].is_u64());

    let metrics = lb
        .get_json(&format!("/api/dashboard/metrics/{}", lb.endpoint_id))
        .await;
    assert!(metrics.is_array());
    let response = lb
        .get(&format!("/api/dashboard/metrics/{}", Uuid::nil()))
        .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body: Value = response.json().await.expect("unknown endpoint error json");
    assert!(body["error"].is_string());

    let models = lb.get_json("/api/dashboard/playground/models").await;
    assert_eq!(models["object"], "list");
    let model = models["data"]
        .as_array()
        .and_then(|data| data.iter().find(|model| model["id"] == MODEL))
        .expect("playground models must list the synced model");
    assert_eq!(model["object"], "model");
    assert!(model["supported_apis"].is_array());

    let token_keys = ["total_input_tokens", "total_output_tokens", "total_tokens"];
    let totals = lb
        .wait_for_json("/api/dashboard/stats/tokens", |totals| {
            totals["total_tokens"]
                .as_u64()
                .is_some_and(|total| total > 0)
        })
        .await;
    for key in token_keys {
        assert!(totals[key].is_u64(), "token totals `{key}`: {totals}");
    }

    let daily = lb
        .wait_for_json("/api/dashboard/stats/tokens/daily", is_non_empty_array)
        .await;
    assert!(daily[0]["date"].is_string());
    let monthly = lb
        .wait_for_json("/api/dashboard/stats/tokens/monthly", is_non_empty_array)
        .await;
    assert!(monthly[0]["month"].is_string());
    for bucket in [&daily[0], &monthly[0]] {
        assert!(bucket["request_count"].is_u64(), "token bucket: {bucket}");
        for key in token_keys {
            assert!(bucket[key].is_u64(), "token bucket `{key}`: {bucket}");
        }
    }

    let logs = lb.get_json("/api/dashboard/logs/lb?limit=5").await;
    assert_eq!(logs["source"], "load balancer");
    assert!(logs["entries"].is_array());
    assert!(logs["path"].is_string());
}

#[tokio::test]
#[serial]
async fn endpoint_scoped_routes_return_logs_and_request_stats() {
    let lb = LbWithEndpoint::spawn().await;
    let endpoint = format!("/api/endpoints/{}", lb.endpoint_id);

    let logs = lb.get_json(&format!("{endpoint}/logs")).await;
    assert_eq!(logs["source"], format!("endpoint:{ENDPOINT_NAME}"));
    assert!(logs["entries"].is_array());
    assert_eq!(logs["path"], ENDPOINT_LOG_PATH);

    let response = lb
        .get(&format!("/api/endpoints/{}/logs", Uuid::nil()))
        .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body: Value = response.json().await.expect("unknown endpoint error json");
    assert!(body["error"].is_string());

    let response = lb.chat(PLAYGROUND_CHAT, &lb.admin_jwt).await;
    assert_eq!(response.status(), StatusCode::OK);

    let today = lb
        .wait_for_json(&format!("{endpoint}/today-stats"), |today| {
            today["total_requests"]
                .as_u64()
                .is_some_and(|total| total > 0)
        })
        .await;
    assert!(today["date"].is_string());
    assert!(today["successful_requests"].is_u64());
    assert!(today["failed_requests"].is_u64());

    let model_stats = lb
        .wait_for_json(&format!("{endpoint}/model-stats"), is_non_empty_array)
        .await;
    assert_eq!(model_stats[0]["model_id"], MODEL);
    assert_has_keys(
        &model_stats[0],
        &[
            "total_requests",
            "successful_requests",
            "failed_requests",
            "total_output_tokens",
            "total_duration_ms",
        ],
        "endpoint model stats",
    );

    let model_tps = lb
        .wait_for_json(&format!("{endpoint}/model-tps"), is_non_empty_array)
        .await;
    assert_eq!(model_tps[0]["model_id"], MODEL);
    assert_eq!(model_tps[0]["api_kind"], "chat_completions");
    assert!(model_tps[0]["tps"].is_number());
    assert_has_keys(
        &model_tps[0],
        &[
            "source",
            "request_count",
            "total_output_tokens",
            "average_duration_ms",
        ],
        "endpoint model tps",
    );
}

#[tokio::test]
#[serial]
async fn catalog_model_routes_return_details_and_endpoint_recommendations() {
    let hugging_face = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/models/{CATALOG_REPO}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": CATALOG_REPO,
            "tags": ["gguf"],
            "downloads": 7,
            "siblings": [{ "rfilename": "model.Q4_K_M.gguf" }]
        })))
        .mount(&hugging_face)
        .await;
    let hf_base_url = ScopedEnvVar::set("HF_BASE_URL", hugging_face.uri());

    let lb = LbWithEndpoint::spawn().await;
    let model = lb.get_json(&format!("/api/catalog/{CATALOG_REPO}")).await;
    let recommended = lb
        .get_json(&format!("/api/catalog/recommend-endpoints/{CATALOG_REPO}"))
        .await;
    drop(hf_base_url);

    assert_eq!(model["repo_id"], CATALOG_REPO);
    assert_eq!(model["downloads"], 7);
    assert_eq!(model["tags"], json!(["gguf"]));
    assert_eq!(model["siblings"][0]["rfilename"], "model.Q4_K_M.gguf");
    assert!(model["engine_names"].is_object());
    assert!(model["supports_download"].is_array());

    assert_eq!(recommended["endpoints"].as_array().map(Vec::len), Some(1));
    let endpoint = &recommended["endpoints"][0];
    assert_eq!(endpoint["id"], lb.endpoint_id.as_str());
    assert_eq!(endpoint["name"], ENDPOINT_NAME);
    assert!(endpoint["endpoint_type"].is_string());
    assert!(endpoint["can_download"].is_boolean());
    assert!(endpoint["has_model"].is_boolean());
}

#[tokio::test]
#[serial]
async fn model_lookup_route_returns_the_model_or_an_openai_error() {
    let lb = LbWithEndpoint::spawn().await;
    let lookup = |model_id: &'static str| {
        lb.client
            .get(lb.url(&format!("/v1/models/{model_id}")))
            .header("x-api-key", "sk_debug")
            .send()
    };

    let response = lookup(MODEL).await.expect("model lookup");
    assert_eq!(response.status(), StatusCode::OK);
    let model: Value = response.json().await.expect("model json");
    assert_eq!(model["id"], MODEL);
    assert_eq!(model["object"], "model");
    assert!(model["created"].is_i64());
    assert!(model["owned_by"].is_string());
    assert!(model["ready"].is_boolean());
    assert!(model["supported_apis"].is_array());

    let response = lookup("route-contract-unknown-model")
        .await
        .expect("unknown model lookup");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body: Value = response.json().await.expect("unknown model error json");
    assert_eq!(body["error"]["code"], "model_not_found");
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert!(body["error"]["message"].is_string());
}
