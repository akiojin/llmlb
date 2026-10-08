//! Integration Test: ダッシュボード
//!
//! WebSocket接続 → リアルタイム更新 → ノード状態変化の受信
//!
//! NOTE: NodeRegistry廃止（SPEC-e8e9326e）に伴い、EndpointRegistryベースに更新済み。
//! NOTE: AUTH_DISABLED廃止に伴い、JWT認証を使用するよう更新済み。

use axum::Router;
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use llmlb::common::auth::UserRole;
use llmlb::{
    api, auth::jwt::create_jwt, balancer::LoadManager, registry::endpoints::EndpointRegistry,
    AppState,
};
use reqwest::Client;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::http::spawn_lb;
use crate::support::lb::{spawn_test_lb, spawn_test_lb_with_db, test_jwt_secret};

/// アップデート確認のテストで GitHub Releases API のモックに使うリポジトリ
const GITHUB_OWNER: &str = "test-owner";
const GITHUB_REPO: &str = "test-repo";

async fn build_test_app() -> (AppState, Router) {
    build_test_app_with_github_api(None).await
}

/// `github_api_base_url` を渡すと、アップデート確認が呼ぶ GitHub Releases API をその URL へ向ける。
async fn build_test_app_with_github_api(github_api_base_url: Option<String>) -> (AppState, Router) {
    let temp_dir = std::env::temp_dir().join(format!(
        "dashboard-ws-test-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    std::env::set_var("LLMLB_DATA_DIR", &temp_dir);

    std::env::set_var("HOME", &temp_dir);
    std::env::set_var("USERPROFILE", &temp_dir);

    let db_pool = crate::support::lb::create_test_db_pool().await;
    let endpoint_registry = EndpointRegistry::new(db_pool.clone())
        .await
        .expect("Failed to create endpoint registry");
    let load_manager = LoadManager::new(Arc::new(endpoint_registry.clone()));
    llmlb::api::models::clear_registered_models(&db_pool)
        .await
        .expect("clear registered models");
    let request_history = std::sync::Arc::new(
        llmlb::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let jwt_secret = "test-secret".to_string();
    let http_client = reqwest::Client::new();
    let inference_gate = llmlb::inference_gate::InferenceGate::default();
    let shutdown = llmlb::shutdown::ShutdownController::default();
    let update_manager = llmlb::update::UpdateManager::new_with_config(
        http_client.clone(),
        inference_gate.clone(),
        shutdown.clone(),
        GITHUB_OWNER.to_string(),
        GITHUB_REPO.to_string(),
        github_api_base_url,
    )
    .expect("Failed to create update manager");
    let state = AppState {
        balancer: llmlb::BalancerState {
            load_manager,
            endpoint_registry,
            request_history,
        },
        db_pool: db_pool.clone(),
        auth: llmlb::AuthState { jwt_secret },
        http_client,
        event_bus: llmlb::events::create_shared_event_bus(),
        lifecycle: llmlb::LifecycleState {
            inference_gate,
            shutdown,
            update_manager,
        },
        audit: llmlb::AuditState {
            writer: llmlb::audit::writer::AuditLogWriter::new(
                llmlb::db::audit_log::AuditLogStorage::new(db_pool.clone()),
                llmlb::audit::writer::AuditLogWriterConfig::default(),
            ),
            storage: std::sync::Arc::new(llmlb::db::audit_log::AuditLogStorage::new(db_pool)),
            archive_pool: None,
        },
    };

    // bootstrap と同様に、レジストリの状態遷移をダッシュボードイベントバスへ配線する
    state
        .balancer
        .endpoint_registry
        .set_event_bus(state.event_bus.clone());

    let app = api::create_app(state.clone());
    (state, app)
}

type WsRequest = tokio_tungstenite::tungstenite::http::Request<()>;

fn admin_jwt(secret: &str) -> String {
    create_jwt("test-admin", UserRole::Admin, secret, false, 0).expect("create test jwt")
}

/// 認証情報を持たない WebSocket 接続リクエストを生成する。`query` は URL の `?` 以降。
fn ws_request(addr: SocketAddr, query: Option<&str>) -> WsRequest {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let query = query.map(|q| format!("?{q}")).unwrap_or_default();
    format!("ws://{addr}/ws/dashboard{query}")
        .into_client_request()
        .expect("build ws client request")
}

fn ws_request_with_header(addr: SocketAddr, name: &'static str, value: &str) -> WsRequest {
    let mut request = ws_request(addr, None);
    request
        .headers_mut()
        .insert(name, value.parse().expect("valid header value"));
    request
}

/// WebSocket 接続リクエストを Authorization ヘッダー付きで生成する。
///
/// クエリパラメータ経由のトークン受理は廃止されたため、ダッシュボード WS は
/// `Authorization: Bearer` ヘッダー（または Cookie）で認証する。
fn ws_request_with_token(addr: SocketAddr, secret: &str) -> WsRequest {
    ws_request_with_header(
        addr,
        "Authorization",
        &format!("Bearer {}", admin_jwt(secret)),
    )
}

#[tokio::test]
async fn test_dashboard_websocket_connection() {
    // Arrange: Router server startup
    let (state, app) = build_test_app().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give the server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Act: WebSocket connection
    let request = ws_request_with_token(addr, &state.auth.jwt_secret);
    let mut subscriber = DashboardSubscriber::connect_with(request).await;
    assert!(subscriber
        .next_event(Duration::from_millis(50))
        .await
        .is_none());
}

type DashboardWsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// イベント受信の待ち時間上限。高負荷のホストでも届くだけの余裕を持たせる。
const EVENT_TIMEOUT: Duration = Duration::from_secs(10);

/// `/ws/dashboard` に接続した購読者
struct DashboardSubscriber {
    // 送信側を保持して接続を維持する
    _write: SplitSink<DashboardWsStream, Message>,
    read: SplitStream<DashboardWsStream>,
}

impl DashboardSubscriber {
    /// HTTP upgrade で接続し、ping/pong でサーバーの購読準備を確認する
    async fn connect(addr: SocketAddr) -> Self {
        Self::connect_with(ws_request_with_token(addr, &test_jwt_secret())).await
    }

    /// `request` の HTTP upgrade 後、welcome が無いことと購読準備を確認する
    async fn connect_with(request: WsRequest) -> Self {
        let (ws_stream, _) = connect_async(request)
            .await
            .expect("Failed to connect to WebSocket");
        let (write, read) = ws_stream.split();
        let mut subscriber = Self {
            _write: write,
            read,
        };
        let probe = vec![1, 2, 3];
        subscriber
            ._write
            .send(Message::Ping(probe.clone().into()))
            .await
            .unwrap();
        let response = tokio::time::timeout(EVENT_TIMEOUT, subscriber.read.next())
            .await
            .expect("ping/pong readiness timeout")
            .expect("WebSocket closed")
            .expect("WebSocket read");
        assert_eq!(
            response,
            Message::Pong(probe.into()),
            "no welcome text frame is allowed"
        );
        subscriber
    }

    /// 次のイベントを受信する。`timeout` 内に届かなければ `None`
    async fn next_event(&mut self, timeout: Duration) -> Option<Value> {
        loop {
            let msg = tokio::time::timeout(timeout, self.read.next())
                .await
                .ok()?
                .expect("WebSocket closed")
                .expect("Message error");
            if let Message::Text(text) = msg {
                return Some(serde_json::from_str(&text).expect("Invalid JSON"));
            }
        }
    }

    /// `resource` のイベントが届くまで受信し、そのイベントを返す
    async fn expect_event(&mut self, resource: &str) -> Value {
        loop {
            let event = self
                .next_event(EVENT_TIMEOUT)
                .await
                .unwrap_or_else(|| panic!("Timeout waiting for {resource}"));
            if event["changed"] == resource {
                return event;
            }
        }
    }

    /// `is_sentinel` に一致するイベントが届くまで受信し、それより前に届いたイベントを返す。
    ///
    /// イベントバスは発行順を保つため、番兵より前に発行されたイベントは必ず番兵より前に届く。
    /// 「届かないこと」「1 回だけ届くこと」を待ち時間に依存せず検証できる。
    async fn events_before(&mut self, is_sentinel: impl Fn(&Value) -> bool) -> Vec<Value> {
        let mut events = Vec::new();
        loop {
            let event = self
                .next_event(EVENT_TIMEOUT)
                .await
                .expect("Timeout waiting for sentinel event");
            if is_sentinel(&event) {
                return events;
            }
            events.push(event);
        }
    }
}

fn is_event(event: &Value, resource: &str, runtime_id: &str) -> bool {
    event["changed"] == resource && event["id"] == runtime_id
}

/// OpenAI 互換として検出され、`test-model` の chat completions に応答するモック
async fn spawn_openai_compatible_mock() -> MockServer {
    spawn_openai_compatible_mock_with_delay(Duration::ZERO).await
}

/// chat completions の応答を `chat_delay` だけ遅らせる OpenAI 互換モック
async fn spawn_openai_compatible_mock_with_delay(chat_delay: Duration) -> MockServer {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{"id": "test-model", "object": "model"}]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(chat_delay)
                .set_body_json(json!({
                    "id": "chatcmpl-test",
                    "object": "chat.completion",
                    "model": "test-model",
                    "choices": [{
                        "index": 0,
                        "message": {"role": "assistant", "content": "ok"},
                        "finish_reason": "stop"
                    }],
                    "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
                })),
        )
        .mount(&mock)
        .await;
    mock
}

async fn post_endpoint(
    client: &Client,
    lb_addr: SocketAddr,
    name: &str,
    base_url: &str,
) -> reqwest::Response {
    client
        .post(format!("http://{}/api/endpoints", lb_addr))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({ "name": name, "base_url": base_url }))
        .send()
        .await
        .expect("registration request failed")
}

/// エンドポイントを登録し、ID を返す
async fn register_endpoint(
    client: &Client,
    lb_addr: SocketAddr,
    name: &str,
    base_url: &str,
) -> String {
    let response = post_endpoint(client, lb_addr, name, base_url).await;
    assert_eq!(response.status().as_u16(), 201);
    let body: Value = response.json().await.expect("registration json");
    body["id"].as_str().expect("endpoint id").to_string()
}

async fn delete_endpoint(
    client: &Client,
    lb_addr: SocketAddr,
    endpoint_id: &str,
) -> reqwest::Response {
    client
        .delete(format!("http://{}/api/endpoints/{}", lb_addr, endpoint_id))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("delete request failed")
}

/// 登録済みエンドポイントを Online にし、モデルを同期して推論可能にする
async fn make_endpoint_routable(client: &Client, lb_addr: SocketAddr, endpoint_id: &str) {
    for action in ["test", "sync"] {
        let response = client
            .post(format!(
                "http://{}/api/endpoints/{}/{}",
                lb_addr, endpoint_id, action
            ))
            .header("authorization", "Bearer sk_debug")
            .send()
            .await
            .expect("endpoint action request failed");
        assert_eq!(response.status().as_u16(), 200, "endpoint {action} failed");
    }
}

async fn chat_completion(client: &Client, lb_addr: SocketAddr) -> reqwest::Response {
    client
        .post(format!("http://{}/v1/chat/completions", lb_addr))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "model": "test-model",
            "messages": [{"role": "user", "content": "hello"}]
        }))
        .send()
        .await
        .expect("chat completion request failed")
}

/// SPEC #582 FR-046: ログインが発行した JWT cookie だけで接続できる（ブラウザが使う経路）
#[tokio::test]
async fn test_dashboard_websocket_connects_with_login_cookie() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let password_hash = llmlb::auth::password::hash_password("password123").unwrap();
    llmlb::db::users::create(
        &db_pool,
        "ws_cookie_admin",
        &password_hash,
        UserRole::Admin,
        false,
    )
    .await
    .expect("create ws_cookie_admin");

    // Arrange: ログイン応答の Set-Cookie から JWT cookie の `name=value` を取り出す
    let login = Client::new()
        .post(format!("http://{}/api/auth/login", server.addr()))
        .json(&json!({ "username": "ws_cookie_admin", "password": "password123" }))
        .send()
        .await
        .expect("login request failed");
    assert_eq!(login.status().as_u16(), 200);
    let jwt_cookie_prefix = format!("{}=", llmlb::auth::DASHBOARD_JWT_COOKIE);
    let jwt_cookie = login
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|cookie| cookie.starts_with(&jwt_cookie_prefix))
        .and_then(|cookie| cookie.split(';').next())
        .expect("login must set the JWT cookie")
        .to_string();

    // Act / Assert: Authorization ヘッダーなしで HTTP upgrade に成功する
    let request = ws_request_with_header(server.addr(), "Cookie", &jwt_cookie);
    DashboardSubscriber::connect_with(request).await;
}

/// SPEC #582 FR-046: query string のトークンでは認証しない。
///
/// URL はアクセスログ・Referer・ブラウザ履歴に残るため、`?token=` の経路は廃止済み。
/// 同じトークンが Authorization ヘッダーでは通ることも確認し、拒否の理由が経路であることを固定する。
#[tokio::test]
async fn test_dashboard_websocket_rejects_query_token() {
    use tokio_tungstenite::tungstenite::Error;

    let server = spawn_test_lb().await;
    let token = admin_jwt(&test_jwt_secret());

    let request = ws_request(server.addr(), Some(&format!("token={token}")));
    let error = connect_async(request)
        .await
        .expect_err("a query string token must not authenticate");
    match error {
        Error::Http(response) => assert_eq!(response.status().as_u16(), 401),
        other => panic!("Expected HTTP 401, got {other:?}"),
    }

    let request =
        ws_request_with_header(server.addr(), "Authorization", &format!("Bearer {token}"));
    DashboardSubscriber::connect_with(request).await;
}

/// SPEC #582 FR-048a: 登録が確定すると、購読者は `NodeRegistered` をちょうど 1 回受信する
#[tokio::test]
async fn test_dashboard_receives_node_registered_once_on_registration() {
    let mock = spawn_openai_compatible_mock().await;
    let sentinel_mock = spawn_openai_compatible_mock().await;
    let server = spawn_test_lb().await;
    let mut subscriber = DashboardSubscriber::connect(server.addr()).await;
    let client = Client::new();

    // Act: 管理 API 経由で登録し、番兵として別のエンドポイントを登録する
    let endpoint_id =
        register_endpoint(&client, server.addr(), "registered-node", &mock.uri()).await;
    let sentinel_id = register_endpoint(
        &client,
        server.addr(),
        "sentinel-node",
        &sentinel_mock.uri(),
    )
    .await;

    // Assert
    let events = subscriber
        .events_before(|event| is_event(event, "endpoints", &sentinel_id))
        .await;
    let registered: Vec<&Value> = events
        .iter()
        .filter(|event| event["changed"] == "endpoints")
        .collect();
    assert_eq!(registered.len(), 1, "events: {events:?}");
    assert_eq!(
        registered[0],
        &json!({"changed":"endpoints", "id":endpoint_id})
    );
}

/// SPEC #582 FR-048a: 失敗した登録では `NodeRegistered` を配信しない
#[tokio::test]
async fn test_dashboard_receives_no_node_registered_on_failed_registration() {
    let mock = spawn_openai_compatible_mock().await;
    let server = spawn_test_lb().await;
    let client = Client::new();
    let existing_id = register_endpoint(&client, server.addr(), "existing-node", &mock.uri()).await;
    let mut subscriber = DashboardSubscriber::connect(server.addr()).await;

    // Act: 名前重複・URL 重複（保存失敗）・到達不能・入力不正の登録はいずれも失敗する
    let other_mock = spawn_openai_compatible_mock().await;
    let duplicate_name =
        post_endpoint(&client, server.addr(), "existing-node", &other_mock.uri()).await;
    assert_eq!(duplicate_name.status().as_u16(), 400);
    let duplicate_url = post_endpoint(&client, server.addr(), "another-node", &mock.uri()).await;
    assert!(!duplicate_url.status().is_success());
    let unreachable = post_endpoint(
        &client,
        server.addr(),
        "unreachable-node",
        "http://127.0.0.1:9",
    )
    .await;
    assert!(!unreachable.status().is_success());
    let invalid = post_endpoint(&client, server.addr(), "invalid-node", "not a url").await;
    assert_eq!(invalid.status().as_u16(), 400);

    // 番兵: 既存エンドポイントの削除
    let deleted = delete_endpoint(&client, server.addr(), &existing_id).await;
    assert_eq!(deleted.status().as_u16(), 204);

    // Assert
    let events = subscriber
        .events_before(|event| is_event(event, "endpoints", &existing_id))
        .await;
    assert!(
        events.iter().all(|event| event["changed"] != "endpoints"),
        "events: {events:?}"
    );
}

/// SPEC #582 FR-048b: 削除が確定すると、購読者は `NodeRemoved` をちょうど 1 回受信する
#[tokio::test]
async fn test_dashboard_receives_node_removed_once_on_deletion() {
    let mock = spawn_openai_compatible_mock().await;
    let sentinel_mock = spawn_openai_compatible_mock().await;
    let server = spawn_test_lb().await;
    let client = Client::new();
    let endpoint_id = register_endpoint(&client, server.addr(), "removed-node", &mock.uri()).await;
    let mut subscriber = DashboardSubscriber::connect(server.addr()).await;

    // Act: 削除し、番兵として別のエンドポイントを登録する
    let deleted = delete_endpoint(&client, server.addr(), &endpoint_id).await;
    assert_eq!(deleted.status().as_u16(), 204);
    let sentinel_id = register_endpoint(
        &client,
        server.addr(),
        "sentinel-node",
        &sentinel_mock.uri(),
    )
    .await;

    // Assert
    let events = subscriber
        .events_before(|event| is_event(event, "endpoints", &sentinel_id))
        .await;
    let removed: Vec<&Value> = events
        .iter()
        .filter(|event| event["changed"] == "endpoints")
        .collect();
    assert_eq!(removed.len(), 1, "events: {events:?}");
    assert_eq!(
        removed[0],
        &json!({"changed":"endpoints", "id":endpoint_id})
    );
}

/// SPEC #582 FR-048b: 失敗した削除（対象なし・削除済み）では `NodeRemoved` を配信しない
#[tokio::test]
async fn test_dashboard_receives_no_node_removed_on_failed_deletion() {
    let mock = spawn_openai_compatible_mock().await;
    let sentinel_mock = spawn_openai_compatible_mock().await;
    let server = spawn_test_lb().await;
    let client = Client::new();
    let endpoint_id = register_endpoint(&client, server.addr(), "removed-node", &mock.uri()).await;
    let deleted = delete_endpoint(&client, server.addr(), &endpoint_id).await;
    assert_eq!(deleted.status().as_u16(), 204);
    let mut subscriber = DashboardSubscriber::connect(server.addr()).await;

    // Act: 存在しない ID と、削除済みの ID を削除する
    let unknown = delete_endpoint(&client, server.addr(), &uuid::Uuid::new_v4().to_string()).await;
    assert_eq!(unknown.status().as_u16(), 404);
    let already_deleted = delete_endpoint(&client, server.addr(), &endpoint_id).await;
    assert_eq!(already_deleted.status().as_u16(), 404);

    // 番兵: 別のエンドポイントの登録
    let sentinel_id = register_endpoint(
        &client,
        server.addr(),
        "sentinel-node",
        &sentinel_mock.uri(),
    )
    .await;

    // Assert
    let events = subscriber
        .events_before(|event| is_event(event, "endpoints", &sentinel_id))
        .await;
    assert!(
        events.iter().all(|event| event["changed"] != "endpoints"),
        "events: {events:?}"
    );
}

/// SPEC #582 FR-048c: 推論リクエストが完了すると、購読者は `MetricsUpdated` を受信する
#[tokio::test]
async fn test_dashboard_receives_metrics_updated_after_proxied_request() {
    let mock = spawn_openai_compatible_mock().await;
    let server = spawn_test_lb().await;
    let client = Client::new();
    let endpoint_id = register_endpoint(&client, server.addr(), "metrics-node", &mock.uri()).await;
    make_endpoint_routable(&client, server.addr(), &endpoint_id).await;
    let mut subscriber = DashboardSubscriber::connect(server.addr()).await;

    // Act
    let response = chat_completion(&client, server.addr()).await;
    assert_eq!(response.status().as_u16(), 200);

    // Assert
    let event = subscriber.expect_event("metrics").await;
    assert_eq!(event, json!({"changed":"metrics", "id":endpoint_id}));
}

/// SPEC #582 FR-048d: 高頻度のリクエストでも `MetricsUpdated` はエンドポイントごとに
/// 1 秒あたり高々 1 回に集約され、リクエスト数には比例しない
#[tokio::test]
async fn test_dashboard_metrics_updated_is_coalesced_under_request_burst() {
    const REQUESTS: usize = 40;
    // 最後の集約窓（1 秒）が閉じたとみなすまでの無通信時間
    const QUIET_PERIOD: Duration = Duration::from_secs(3);

    let mock = spawn_openai_compatible_mock().await;
    let server = spawn_test_lb().await;
    let client = Client::new();
    let endpoint_id = register_endpoint(&client, server.addr(), "burst-node", &mock.uri()).await;
    make_endpoint_routable(&client, server.addr(), &endpoint_id).await;
    let mut subscriber = DashboardSubscriber::connect(server.addr()).await;

    // Act: 同じエンドポイントへ並行にリクエストを送る
    let burst_started_at = Instant::now();
    let responses =
        futures::future::join_all((0..REQUESTS).map(|_| chat_completion(&client, server.addr())))
            .await;
    for response in responses {
        assert_eq!(response.status().as_u16(), 200);
    }

    // Assert: 無通信になるまで受信し、MetricsUpdated の件数と最後の受信時刻を記録する
    let mut metrics_updated = 0usize;
    let mut last_received_at = burst_started_at;
    let mut timeout = EVENT_TIMEOUT;
    while let Some(event) = subscriber.next_event(timeout).await {
        if is_event(&event, "metrics", &endpoint_id) {
            metrics_updated += 1;
            last_received_at = Instant::now();
            timeout = QUIET_PERIOD;
        }
    }

    assert!(metrics_updated >= 1, "MetricsUpdated was never received");
    // 集約窓は 1 秒で互いに重ならないため、n 件目の配信は開始から n 秒以降になる。
    // 受信の遅延は経過時間を伸ばす方向にしか働かないので、負荷に依存せず成り立つ。
    let elapsed_secs = last_received_at
        .duration_since(burst_started_at)
        .as_secs_f64();
    assert!(
        metrics_updated as f64 <= elapsed_secs,
        "{metrics_updated} MetricsUpdated events within {elapsed_secs:.2}s for {REQUESTS} requests"
    );
}

/// SPEC #582 FR-048: 推論リクエストが成功して TPS が計測されると、購読者は `TpsUpdated` を受信する
#[tokio::test]
async fn test_dashboard_receives_tps_updated_after_proxied_request() {
    // TPS は処理時間が 1 ms 以上のリクエストだけが計測対象になるため、応答を遅らせる
    let mock = spawn_openai_compatible_mock_with_delay(Duration::from_millis(50)).await;
    let server = spawn_test_lb().await;
    let client = Client::new();
    let endpoint_id = register_endpoint(&client, server.addr(), "tps-node", &mock.uri()).await;
    make_endpoint_routable(&client, server.addr(), &endpoint_id).await;
    let mut subscriber = DashboardSubscriber::connect(server.addr()).await;

    // Act
    let response = chat_completion(&client, server.addr()).await;
    assert_eq!(response.status().as_u16(), 200);

    // Assert
    let event = subscriber.expect_event("tps").await;
    assert_eq!(event, json!({"changed":"tps", "id":endpoint_id}));
}

/// SPEC #582 FR-048: アップデート確認が完了すると、購読者は `UpdateStateChanged` を受信する
#[tokio::test]
async fn test_dashboard_receives_update_state_changed_after_update_check() {
    // 現行より新しくないバージョンを返すので、ダウンロードは始まらない
    let github = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/repos/{GITHUB_OWNER}/{GITHUB_REPO}/releases/latest"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tag_name": "v0.0.0",
            "html_url": "https://example.invalid/releases/tag/v0.0.0",
            "assets": []
        })))
        .mount(&github)
        .await;
    let (state, app) = build_test_app_with_github_api(Some(github.uri())).await;
    let server = spawn_lb(app).await;
    let secret = &state.auth.jwt_secret;
    let mut subscriber =
        DashboardSubscriber::connect_with(ws_request_with_token(server.addr(), secret)).await;

    // Act
    let response = Client::new()
        .post(format!("http://{}/api/system/update/check", server.addr()))
        .bearer_auth(admin_jwt(secret))
        .send()
        .await
        .expect("update check request failed");
    assert_eq!(response.status().as_u16(), 200);

    // Assert: 再取得を促すだけの通知で、ペイロードを持たない
    let event = subscriber.expect_event("system").await;
    assert_eq!(event, json!({"changed":"system"}));
}

#[tokio::test]
async fn test_dashboard_receives_node_status_change() {
    // Arrange: Router server startup
    let (state, app) = build_test_app().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give the server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let mut subscriber =
        DashboardSubscriber::connect_with(ws_request_with_token(addr, &state.auth.jwt_secret))
            .await;

    // Arrange: register an endpoint (Pending)
    let endpoint = llmlb::types::endpoint::Endpoint::new(
        "status-change-test".to_string(),
        "http://127.0.0.1:9".to_string(),
        llmlb::types::endpoint::EndpointType::OpenaiCompatible,
    );
    let endpoint_id = endpoint.id;
    state
        .balancer
        .endpoint_registry
        .add(endpoint)
        .await
        .unwrap();

    // Act: change status through the production update path (health checker / connection test)
    state
        .balancer
        .endpoint_registry
        .update_status(
            endpoint_id,
            llmlb::types::endpoint::EndpointStatus::Offline,
            None,
            Some("connection refused"),
        )
        .await
        .unwrap();

    let event = subscriber.expect_event("endpoints").await;
    assert_eq!(event, json!({"changed":"endpoints", "id":endpoint_id}));
}

/// SPEC #821: every internal variant projects to only changed/id at the WS boundary.
#[tokio::test]
async fn test_dashboard_wire_projects_all_internal_variants() {
    use llmlb::events::DashboardEvent;
    use llmlb::types::endpoint::EndpointStatus;
    let (state, app) = build_test_app().await;
    let server = spawn_lb(app).await;
    let mut subscriber = DashboardSubscriber::connect_with(ws_request_with_token(
        server.addr(),
        &state.auth.jwt_secret,
    ))
    .await;
    let id = uuid::Uuid::new_v4();
    let cases = [
        (
            DashboardEvent::NodeRegistered {
                runtime_id: id,
                machine_name: "private-name".into(),
                ip_address: "192.0.2.1".into(),
                status: EndpointStatus::Pending,
            },
            json!({"changed":"endpoints", "id":id}),
        ),
        (
            DashboardEvent::NodeRemoved { runtime_id: id },
            json!({"changed":"endpoints", "id":id}),
        ),
        (
            DashboardEvent::EndpointStatusChanged {
                runtime_id: id,
                old_status: EndpointStatus::Online,
                new_status: EndpointStatus::Offline,
            },
            json!({"changed":"endpoints", "id":id}),
        ),
        (
            DashboardEvent::MetricsUpdated {
                runtime_id: id,
                cpu_usage: Some(12.0),
                memory_usage: None,
                gpu_usage: None,
            },
            json!({"changed":"metrics", "id":id}),
        ),
        (
            DashboardEvent::TpsUpdated {
                endpoint_id: id,
                model_id: "private-model".into(),
                tps: 10.0,
                output_tokens: 1,
                duration_ms: 100,
            },
            json!({"changed":"tps", "id":id}),
        ),
        (
            DashboardEvent::UpdateStateChanged,
            json!({"changed":"system"}),
        ),
    ];
    for (event, expected) in cases {
        state.event_bus.publish(event);
        assert_eq!(
            subscriber.next_event(EVENT_TIMEOUT).await.unwrap(),
            expected
        );
    }
}
