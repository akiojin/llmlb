//! SPEC #585 FR-027 / Issue #696 / Issue #806: 公開 API ルートの契約テスト。
//!
//! 既存の unit / integration / E2E テストと重複しないよう、本番 Router への接続、
//! 認証境界、HTTP status、主要な response shape に限定して検証する。
//!
//! 公開ルートを追加したら、このファイルの `ROUTE_CONTRACTS`（契約台帳）へ行を足すこと。
//! 台帳にも `EXCLUDED_ROUTES` にも無いルートがあると
//! `every_public_route_is_listed_in_the_route_contract_ledger` が失敗する。

use axum::{
    body::{to_bytes, Body},
    extract::MatchedPath,
    http::{header, Method, Request, StatusCode},
    middleware::{self, Next},
    response::Response,
    Router,
};
use llmlb::common::auth::UserRole;
use regex::Regex;
use reqwest::Client;
use serde_json::{json, Value};
use serial_test::serial;
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    net::SocketAddr,
    path::Path,
    time::Duration,
};
use tower::ServiceExt;
use uuid::Uuid;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

pub(super) struct ScopedEnvVar {
    key: &'static str,
    previous: Option<OsString>,
}

impl ScopedEnvVar {
    pub(super) fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let previous = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for ScopedEnvVar {
    fn drop(&mut self) {
        if let Some(value) = self.previous.as_ref() {
            std::env::set_var(self.key, value);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

pub(super) async fn build_app_with_admin_jwt() -> (Router, String) {
    let (app, db_pool) = crate::support::lb::create_test_lb_default_auth().await;
    let password_hash = llmlb::auth::password::hash_password("password123").unwrap();
    let admin = llmlb::db::users::create(
        &db_pool,
        "route-contract-admin",
        &password_hash,
        UserRole::Admin,
        false,
    )
    .await
    .expect("create route contract admin");
    let jwt = llmlb::auth::jwt::create_jwt(
        &admin.id.to_string(),
        UserRole::Admin,
        &crate::support::lb::test_jwt_secret(),
        false,
        0,
    )
    .expect("create route contract admin jwt");

    (app, jwt)
}

pub(super) async fn send_request(
    app: &Router,
    method: Method,
    uri: &str,
    jwt: Option<&str>,
    json_body: Option<Value>,
) -> Response {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(jwt) = jwt {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {jwt}"));
    }
    let body = if let Some(value) = json_body {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
        Body::from(serde_json::to_vec(&value).expect("serialize request body"))
    } else {
        Body::empty()
    };

    app.clone()
        .oneshot(builder.body(body).expect("build request"))
        .await
        .expect("route response")
}

pub(super) async fn response_json(response: Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    serde_json::from_slice(&body).expect("response must be JSON")
}

fn websocket_request(client: &Client, addr: SocketAddr, path: &str) -> reqwest::RequestBuilder {
    client
        .get(format!("http://{addr}{path}"))
        .header(header::CONNECTION, "Upgrade")
        .header(header::UPGRADE, "websocket")
        .header("sec-websocket-version", "13")
        .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
}

// ---------------------------------------------------------------------------
// 公開ルートの網羅ガード（Issue #806 AC-2）
//
// 公開ルート一覧は `llmlb/src/api/router.rs` と `router/*.rs` のルート宣言を走査して得る
// （axum の Router は登録済みルートを列挙する公開 API を持たない）。
// 走査結果は契約台帳 `ROUTE_CONTRACTS` と突き合わせ、台帳の各行は本番 Router へ実際に
// 要求を送って MatchedPath と status を実測する。走査が誤ったパスを拾えば実測で落ちる。
// ---------------------------------------------------------------------------

/// (METHOD, 本番 Router 上のルートテンプレート)
type RouteKey = (String, String);

fn route_key(method: &str, path: &str) -> RouteKey {
    (method.to_string(), path.to_string())
}

/// ルートを宣言する Router 関数と、本番 Router でのマウント先。
///
/// (`llmlb/src/api/` からの相対パス, 関数名, マウント先 prefix)。
/// `router.rs` の `create_app()` が `/api` へ nest するものと、ルート直下へ merge するものがある。
const ROUTER_MOUNTS: &[(&str, &str, &str)] = &[
    ("router/auth.rs", "routes", "/api"),
    ("router/dashboard.rs", "api_routes", "/api"),
    ("router/dashboard.rs", "ui_routes", ""),
    ("router/endpoints.rs", "routes", "/api"),
    ("router/inference.rs", "routes", ""),
    ("router/metrics.rs", "routes", "/api"),
    ("router/models.rs", "routes", "/api"),
    ("router/system.rs", "routes", "/api"),
];

/// 走査器が解釈できる `axum::routing` のメソッド関数。
const ROUTING_METHODS: &[&str] = &["get", "post", "put", "delete", "patch"];

/// ルート宣言の走査結果。
#[derive(Default)]
struct RouterScan {
    /// (ファイル, 関数) ごとの、`.route()` が宣言する (METHOD, path)
    routes: BTreeMap<(String, String), Vec<RouteKey>>,
    /// `.nest(prefix, ..)` の (ファイル, prefix)
    nests: Vec<(String, String)>,
    /// `.merge(module::function(..))` で取り込まれた Router 関数の (ファイル, 関数)
    merged: BTreeSet<(String, String)>,
}

/// 文字列リテラルの内外を追跡する。
#[derive(Default)]
struct StringTracker {
    in_string: bool,
    escaped: bool,
}

impl StringTracker {
    /// `current` が文字列リテラルの外（コード）にあれば true を返す。
    fn is_code(&mut self, current: char) -> bool {
        if self.in_string {
            if self.escaped {
                self.escaped = false;
            } else if current == '\\' {
                self.escaped = true;
            } else if current == '"' {
                self.in_string = false;
            }
            return false;
        }
        if current == '"' {
            self.in_string = true;
            return false;
        }
        true
    }
}

/// 文字列リテラルの外にある `//` 以降を取り除く（コメント中の `.route(` を拾わないため）。
fn strip_line_comments(source: &str) -> String {
    let strip = |line: &str| -> String {
        let mut tracker = StringTracker::default();
        let mut previous_is_slash = false;
        for (index, current) in line.char_indices() {
            let is_slash = tracker.is_code(current) && current == '/';
            if is_slash && previous_is_slash {
                return line[..index - 1].to_string();
            }
            previous_is_slash = is_slash;
        }
        line.to_string()
    };
    source.lines().map(strip).collect::<Vec<_>>().join("\n")
}

/// `open` が指す `(` に対応する `)` までの中身を返す（文字列リテラル内の括弧は数えない）。
fn balanced_arguments(source: &str, open: usize) -> &str {
    let mut tracker = StringTracker::default();
    let mut depth = 0usize;
    for (offset, current) in source[open..].char_indices() {
        if !tracker.is_code(current) {
            continue;
        }
        match current {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return &source[open + 1..open + offset];
                }
            }
            _ => {}
        }
    }
    panic!("ルート宣言の括弧が閉じていません");
}

/// 引数列の先頭にある文字列リテラルと、その後ろの引数を返す。
fn leading_string_literal(arguments: &str) -> Option<(&str, &str)> {
    let (literal, rest) = arguments.trim_start().strip_prefix('"')?.split_once('"')?;
    Some((literal, rest.trim_start().strip_prefix(',')?))
}

/// `get(a).put(b)` のような MethodRouter 式から、宣言された HTTP メソッドを取り出す。
fn declared_methods(method_router: &str, location: &str) -> Vec<String> {
    let mut methods = Vec::new();
    let mut depth = 0usize;
    let mut identifier = String::new();
    for current in method_router.chars() {
        match current {
            '(' => {
                if depth == 0 {
                    assert!(
                        ROUTING_METHODS.contains(&identifier.as_str()),
                        "{location}: `{identifier}(..)` は走査器が未対応のルート宣言です。\
                         api_route_coverage_test.rs の走査器を拡張してください"
                    );
                    methods.push(identifier.to_uppercase());
                }
                depth += 1;
                identifier.clear();
            }
            ')' => depth -= 1,
            _ if current.is_alphanumeric() || current == '_' => identifier.push(current),
            _ => identifier.clear(),
        }
    }
    assert!(
        !methods.is_empty(),
        "{location}: HTTP メソッドを読み取れません"
    );
    methods
}

fn scan_router_source(file: &str, source: &str, scan: &mut RouterScan) {
    let source = strip_line_comments(source);
    let token = Regex::new(
        r"\bfn\s+(\w+)|\.(route|nest|merge|route_service|nest_service|fallback_service)\(",
    )
    .expect("router token pattern");
    let merged_function = Regex::new(r"^(\w+)::(\w+)\(").expect("merged router pattern");
    let mut function = String::new();
    for captures in token.captures_iter(&source) {
        if let Some(name) = captures.get(1) {
            function = name.as_str().to_string();
            continue;
        }
        let call = captures.get(2).expect("router call name");
        let location = format!("{file}::{function}");
        assert!(
            matches!(call.as_str(), "route" | "nest" | "merge"),
            "{location}: `.{}(..)` は走査器が未対応のルート宣言です。\
             api_route_coverage_test.rs の走査器を拡張してください",
            call.as_str()
        );
        let arguments = balanced_arguments(&source, call.end());
        if call.as_str() == "merge" {
            // 同じ関数内で組み立てた Router（ローカル変数）の merge は、その `.route()` を
            // 走査済み。別モジュールの Router 関数を取り込む merge だけを記録する。
            // 別モジュールの Router をローカル変数へ束縛してから merge する書き方は検出できない。
            let argument = arguments.trim();
            if let Some(captures) = merged_function.captures(argument) {
                scan.merged.insert((
                    format!("router/{}.rs", &captures[1]),
                    captures[2].to_string(),
                ));
            } else {
                assert!(
                    argument.chars().all(|c| c.is_alphanumeric() || c == '_'),
                    "{location}: `.merge({argument})` は走査器が未対応の Router 取り込みです。\
                     api_route_coverage_test.rs の走査器を拡張してください"
                );
            }
            continue;
        }
        let (literal, rest) = leading_string_literal(arguments)
            .unwrap_or_else(|| panic!("{location}: パスが文字列リテラルではありません"));
        if call.as_str() == "nest" {
            scan.nests.push((file.to_string(), literal.to_string()));
            continue;
        }
        let location = format!("{location} {literal}");
        scan.routes
            .entry((file.to_string(), function.clone()))
            .or_default()
            .extend(
                declared_methods(rest, &location)
                    .into_iter()
                    .map(|method| (method, literal.to_string())),
            );
    }
}

/// 本番 Router を構成するソース（`router.rs` と `router/*.rs`）を走査する。
fn scan_production_routers() -> RouterScan {
    let api_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/api");
    let mut files = vec!["router.rs".to_string()];
    for entry in std::fs::read_dir(api_dir.join("router")).expect("read llmlb/src/api/router") {
        let entry = entry.expect("read router directory entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        assert!(
            entry.path().is_file(),
            "llmlb/src/api/router/{name} はディレクトリです。走査器はサブディレクトリを辿りません。\
             api_route_coverage_test.rs の走査器を拡張してください"
        );
        // tests.rs は `#[cfg(test)]` の単体テストで、本番 Router には入らない。
        if name.ends_with(".rs") && name != "tests.rs" {
            files.push(format!("router/{name}"));
        }
    }
    files.sort();

    let mut scan = RouterScan::default();
    for file in &files {
        let source = std::fs::read_to_string(api_dir.join(file))
            .unwrap_or_else(|error| panic!("read llmlb/src/api/{file}: {error}"));
        scan_router_source(file, &source, &mut scan);
    }
    scan
}

/// 走査結果へマウント先を適用し、公開ルート一覧 (METHOD, path) を得る。
fn public_routes(scan: &RouterScan) -> BTreeSet<RouteKey> {
    assert_eq!(
        scan.nests,
        [("router.rs".to_string(), "/api".to_string())],
        "`.nest()` の構成が変わりました。ROUTER_MOUNTS のマウント先を見直してください"
    );

    let mut routes = BTreeSet::new();
    for ((file, function), declared) in &scan.routes {
        let (_, _, prefix) = ROUTER_MOUNTS
            .iter()
            .find(|(mount_file, mount_function, _)| {
                mount_file == file && mount_function == function
            })
            .unwrap_or_else(|| {
                panic!(
                    "{file}::{function} がルートを宣言していますが、マウント先が \
                     ROUTER_MOUNTS にありません"
                )
            });
        for (method, path) in declared {
            assert!(
                routes.insert((method.clone(), format!("{prefix}{path}"))),
                "{method} {prefix}{path} が二重に宣言されています"
            );
        }
    }
    let mounted: BTreeSet<(String, String)> = ROUTER_MOUNTS
        .iter()
        .map(|(file, function, _)| (file.to_string(), function.to_string()))
        .collect();
    for mount in &mounted {
        assert!(
            scan.routes.contains_key(mount),
            "ROUTER_MOUNTS の {}::{} はルートを宣言していません",
            mount.0,
            mount.1
        );
    }
    // ルートを宣言しない関数経由でも、走査対象外の Router が本番 Router へ入らないこと。
    assert_eq!(
        scan.merged, mounted,
        "`.merge()` で取り込む Router 関数が ROUTER_MOUNTS と一致しません"
    );
    routes
}

/// 資格情報の無い要求に対する契約。
#[derive(Clone, Copy)]
enum Boundary {
    /// 認証必須。資格情報の無い要求は 401 で拒否される。
    Protected,
    /// 認証ミドルウェアを持たない。資格情報の無い要求にこの status を返す。
    Open(StatusCode),
}

/// 契約台帳の 1 行。公開ルートと、その認証境界。
struct RouteContract {
    method: &'static str,
    path: &'static str,
    boundary: Boundary,
}

const fn protected(method: &'static str, path: &'static str) -> RouteContract {
    RouteContract {
        method,
        path,
        boundary: Boundary::Protected,
    }
}

const fn open(method: &'static str, path: &'static str, status: StatusCode) -> RouteContract {
    RouteContract {
        method,
        path,
        boundary: Boundary::Open(status),
    }
}

/// 契約台帳。公開ルートはすべて、ここか `EXCLUDED_ROUTES` のどちらかに載る。
const ROUTE_CONTRACTS: &[RouteContract] = &[
    // router/system.rs
    open("GET", "/api/version", StatusCode::OK),
    open("GET", "/api/system", StatusCode::OK),
    protected("POST", "/api/system/update/check"),
    protected("POST", "/api/system/update/apply"),
    protected("POST", "/api/system/update/apply/force"),
    protected("POST", "/api/system/update/schedule"),
    protected("GET", "/api/system/update/schedule"),
    protected("DELETE", "/api/system/update/schedule"),
    protected("POST", "/api/system/update/rollback"),
    // router/auth.rs
    open("POST", "/api/auth/login", StatusCode::UNPROCESSABLE_ENTITY),
    open(
        "POST",
        "/api/auth/register",
        StatusCode::UNPROCESSABLE_ENTITY,
    ),
    open(
        "POST",
        "/api/auth/forgot-password",
        StatusCode::UNPROCESSABLE_ENTITY,
    ),
    open(
        "POST",
        "/api/auth/reset-password",
        StatusCode::UNPROCESSABLE_ENTITY,
    ),
    protected("GET", "/api/auth/me"),
    protected("POST", "/api/auth/logout"),
    protected("PUT", "/api/auth/change-password"),
    protected("GET", "/api/users"),
    protected("POST", "/api/users"),
    protected("PUT", "/api/users/{id}"),
    protected("DELETE", "/api/users/{id}"),
    protected("GET", "/api/me/api-keys"),
    protected("POST", "/api/me/api-keys"),
    protected("PUT", "/api/me/api-keys/{id}"),
    protected("DELETE", "/api/me/api-keys/{id}"),
    protected("GET", "/api/invitations"),
    protected("POST", "/api/invitations"),
    protected("DELETE", "/api/invitations/{id}"),
    // router/dashboard.rs（api_routes）
    protected("GET", "/api/dashboard/endpoints"),
    protected("GET", "/api/dashboard/models"),
    protected("GET", "/api/dashboard/playground/models"),
    protected("GET", "/api/dashboard/stats"),
    protected("GET", "/api/dashboard/request-history"),
    protected("GET", "/api/dashboard/overview"),
    protected("GET", "/api/dashboard/metrics/{endpoint_id}"),
    protected("GET", "/api/dashboard/request-responses"),
    protected("GET", "/api/dashboard/request-responses/{id}"),
    protected("GET", "/api/dashboard/request-responses/export"),
    protected("GET", "/api/dashboard/stats/tokens"),
    protected("GET", "/api/dashboard/stats/tokens/daily"),
    protected("GET", "/api/dashboard/stats/tokens/monthly"),
    protected("GET", "/api/dashboard/logs/lb"),
    protected("GET", "/api/dashboard/model-stats"),
    protected("POST", "/api/benchmarks/tps"),
    protected("GET", "/api/benchmarks/tps/{run_id}"),
    protected("GET", "/api/dashboard/clients"),
    protected("GET", "/api/dashboard/clients/timeline"),
    protected("GET", "/api/dashboard/clients/models"),
    protected("GET", "/api/dashboard/clients/heatmap"),
    protected("GET", "/api/dashboard/clients/{ip}/detail"),
    protected("GET", "/api/dashboard/clients/{ip}/api-keys"),
    protected("GET", "/api/dashboard/settings/{key}"),
    protected("PUT", "/api/dashboard/settings/{key}"),
    protected("GET", "/api/catalog/search"),
    protected("GET", "/api/catalog/recommend-endpoints/{*repo_id}"),
    protected("GET", "/api/catalog/{*repo_id}"),
    protected("POST", "/api/dashboard/playground/chat/completions"),
    protected(
        "POST",
        "/api/dashboard/playground/load-test/chat/completions",
    ),
    protected("GET", "/api/dashboard/audit-logs"),
    protected("GET", "/api/dashboard/audit-logs/stats"),
    protected("POST", "/api/dashboard/audit-logs/verify"),
    protected("GET", "/api/dashboard/notifications"),
    protected("PUT", "/api/dashboard/notifications"),
    // router/dashboard.rs（ui_routes）
    open("GET", "/dashboard", StatusCode::OK),
    // upgrade ヘッダーの無い要求は、JWT の検査より先に WebSocketUpgrade extractor が拒否する。
    // JWT の境界（401）は実サーバーを使う `websocket_contract_*` テストが検証する。
    open("GET", "/ws/dashboard", StatusCode::BAD_REQUEST),
    // router/endpoints.rs
    protected("GET", "/api/endpoints/{id}/logs"),
    protected("GET", "/api/endpoints"),
    protected("GET", "/api/endpoints/{id}"),
    protected("GET", "/api/endpoints/{id}/models"),
    protected("GET", "/api/endpoints/{id}/download/progress"),
    protected("GET", "/api/endpoints/{id}/models/{model}/info"),
    protected("GET", "/api/endpoints/{id}/today-stats"),
    protected("GET", "/api/endpoints/{id}/daily-stats"),
    protected("GET", "/api/endpoints/{id}/model-stats"),
    protected("GET", "/api/endpoints/{id}/model-tps"),
    protected("POST", "/api/endpoints"),
    protected("PUT", "/api/endpoints/{id}"),
    protected("DELETE", "/api/endpoints/{id}"),
    protected("POST", "/api/endpoints/{id}/test"),
    protected("POST", "/api/endpoints/{id}/sync"),
    protected("POST", "/api/endpoints/{id}/download"),
    protected("POST", "/api/endpoints/{id}/models/delete"),
    protected("POST", "/api/endpoints/{id}/chat/completions"),
    // router/models.rs
    protected("POST", "/api/models/register"),
    protected("DELETE", "/api/models/{*model_name}"),
    protected("GET", "/api/models/registry/{model_name}/manifest.json"),
    protected("GET", "/api/models"),
    protected("GET", "/api/models/hub"),
    // router/metrics.rs
    protected("GET", "/api/metrics/cloud"),
    protected("GET", "/api/metrics/cloud/export"),
    // router/inference.rs
    protected("POST", "/v1/chat/completions"),
    protected("POST", "/v1/completions"),
    protected("POST", "/v1/embeddings"),
    protected("POST", "/v1/responses"),
    protected("POST", "/v1/audio/transcriptions"),
    protected("POST", "/v1/audio/speech"),
    protected("POST", "/v1/images/generations"),
    protected("POST", "/v1/images/edits"),
    protected("POST", "/v1/images/variations"),
    protected("POST", "/v1/messages"),
    protected("GET", "/v1/models"),
    protected("GET", "/v1/models/{model_id}"),
];

/// 契約台帳の対象外。JSON の request / response という API 契約を持たないルートだけを置く。
const EXCLUDED_ROUTES: &[(&str, &str)] = &[
    // 埋め込み静的アセットの配信（`serve_dashboard_index`）。`GET /dashboard` と同じ
    // index.html を返す別名で、API 契約を持たない。
    ("GET", "/dashboard/"),
    // 埋め込み静的アセットの配信（`serve_dashboard_asset`）。応答はダッシュボードの
    // ビルド成果物そのもので、API 契約を持たない。
    ("GET", "/dashboard/{*path}"),
];

fn describe_routes(routes: &[&RouteKey]) -> String {
    routes
        .iter()
        .map(|(method, path)| format!("  {method} {path}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// ルートテンプレートのパラメータを具体値に置き換え、要求 URI にする。
fn concrete_uri(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            if segment.starts_with("{*") {
                "contract/probe".to_string()
            } else if segment.starts_with('{') {
                Uuid::nil().to_string()
            } else {
                segment.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// 本番 Router が要求を振り分けたルートテンプレートを、応答の extension へ写す。
async fn expose_matched_path(request: Request<Body>, next: Next) -> Response {
    let matched_path = request.extensions().get::<MatchedPath>().cloned();
    let mut response = next.run(request).await;
    if let Some(matched_path) = matched_path {
        response.extensions_mut().insert(matched_path);
    }
    response
}

#[test]
fn route_scanner_reads_method_chains_and_skips_comments() {
    let source = r#"
        pub(super) fn routes(state: &AppState) -> Router<AppState> {
            // .route("/commented-out", get(handler))
            Router::new()
                .route("/items", get(list).post(create)) // "(trailing comment
                .route("/odd)//path", get(odd))
                .route(
                    "/items/{id}",
                    axum::routing::put(update).delete(remove),
                )
                .layer(middleware::from_fn(guard))
                .merge(local_routes)
                .merge(orders::routes(&state))
        }
    "#;
    let mut scan = RouterScan::default();
    scan_router_source("router/items.rs", source, &mut scan);

    assert!(scan.nests.is_empty());
    assert_eq!(
        scan.merged,
        BTreeSet::from([("router/orders.rs".to_string(), "routes".to_string())])
    );
    assert_eq!(
        scan.routes
            .get(&("router/items.rs".to_string(), "routes".to_string())),
        Some(&vec![
            route_key("GET", "/items"),
            route_key("POST", "/items"),
            route_key("GET", "/odd)//path"),
            route_key("PUT", "/items/{id}"),
            route_key("DELETE", "/items/{id}"),
        ])
    );
}

#[test]
#[should_panic(expected = "走査器が未対応のルート宣言")]
fn route_scanner_rejects_declarations_it_cannot_read() {
    let source = r#"fn routes() { Router::new().route_service("/files", ServeDir::new("x")) }"#;
    scan_router_source("router/files.rs", source, &mut RouterScan::default());
}

#[test]
fn every_public_route_is_listed_in_the_route_contract_ledger() {
    let public = public_routes(&scan_production_routers());
    let excluded: BTreeSet<RouteKey> = EXCLUDED_ROUTES
        .iter()
        .map(|(method, path)| route_key(method, path))
        .collect();
    let ledger: BTreeSet<RouteKey> = ROUTE_CONTRACTS
        .iter()
        .map(|contract| route_key(contract.method, contract.path))
        .collect();
    assert_eq!(
        ledger.len(),
        ROUTE_CONTRACTS.len(),
        "ROUTE_CONTRACTS に同じルートの行が複数あります"
    );

    let unlisted: Vec<_> = public
        .iter()
        .filter(|route| !ledger.contains(*route) && !excluded.contains(*route))
        .collect();
    assert!(
        unlisted.is_empty(),
        "契約台帳に無い公開ルートが {} 組あります。\n{}\n\
         契約テストを llmlb/tests/contract/ に追加し、ROUTE_CONTRACTS へ行を足してください。\n\
         API 契約を持たないルートだけは、理由のコメントを付けて EXCLUDED_ROUTES に置けます。",
        unlisted.len(),
        describe_routes(&unlisted)
    );

    let stale: Vec<_> = ledger
        .iter()
        .chain(excluded.iter())
        .filter(|route| !public.contains(*route))
        .collect();
    assert!(
        stale.is_empty(),
        "本番 Router に無いルートが ROUTE_CONTRACTS / EXCLUDED_ROUTES に残っています。\n{}",
        describe_routes(&stale)
    );

    let listed_twice: Vec<_> = ledger.intersection(&excluded).collect();
    assert!(
        listed_twice.is_empty(),
        "ROUTE_CONTRACTS と EXCLUDED_ROUTES の両方に載っているルートがあります。\n{}",
        describe_routes(&listed_twice)
    );
}

#[tokio::test]
#[serial]
async fn route_contract_ledger_matches_the_production_router() {
    let (app, _db_pool) = crate::support::lb::create_test_lb_default_auth().await;
    let app = app.layer(middleware::from_fn(expose_matched_path));

    let mut violations = Vec::new();
    for contract in ROUTE_CONTRACTS {
        let method = Method::from_bytes(contract.method.as_bytes()).expect("ledger method");
        let uri = concrete_uri(contract.path);
        let body = if matches!(method, Method::POST | Method::PUT | Method::PATCH) {
            Body::from("{}")
        } else {
            Body::empty()
        };
        // `/v1/messages` は Anthropic 互換のため、認証より先に anthropic-version ヘッダーを
        // 要求する。認証境界まで到達させるため、どの行にも付けて送る。
        let request = Request::builder()
            .method(method)
            .uri(&uri)
            .header(header::CONTENT_TYPE, "application/json")
            .header("anthropic-version", "2023-06-01")
            .body(body)
            .expect("build ledger request");
        let response = app
            .clone()
            .oneshot(request)
            .await
            .expect("ledger route response");

        let matched_path = response
            .extensions()
            .get::<MatchedPath>()
            .map(|matched| matched.as_str().to_string());
        if matched_path.as_deref() != Some(contract.path) {
            violations.push(format!(
                "{} {uri}: 本番 Router は {} へ振り分けるはずが {matched_path:?} でした",
                contract.method, contract.path
            ));
            continue;
        }

        let expected = match contract.boundary {
            Boundary::Protected => StatusCode::UNAUTHORIZED,
            Boundary::Open(status) => status,
        };
        if response.status() != expected {
            violations.push(format!(
                "{} {}: 資格情報の無い要求に {expected} を返すはずが {} でした",
                contract.method,
                contract.path,
                response.status()
            ));
        }
    }
    assert!(
        violations.is_empty(),
        "契約台帳が本番 Router と一致しません。\n{}",
        violations.join("\n")
    );
}

#[tokio::test]
#[serial]
async fn system_update_routes_expose_only_documented_methods() {
    let (app, jwt) = build_app_with_admin_jwt().await;
    let cases: [(&str, &[&str]); 5] = [
        ("/api/system/update/check", &["POST"]),
        ("/api/system/update/apply", &["POST"]),
        ("/api/system/update/apply/force", &["POST"]),
        (
            "/api/system/update/schedule",
            &["GET", "HEAD", "POST", "DELETE"],
        ),
        ("/api/system/update/rollback", &["POST"]),
    ];

    for (uri, expected_methods) in cases {
        let response = send_request(&app, Method::OPTIONS, uri, Some(&jwt), None).await;
        assert_eq!(
            response.status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "OPTIONS {uri} must expose the registered method contract"
        );
        let allow = response
            .headers()
            .get(header::ALLOW)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_else(|| panic!("OPTIONS {uri} must include an Allow header"));
        let allowed_methods: Vec<_> = allow.split(',').map(str::trim).collect();
        assert_eq!(
            allowed_methods.len(),
            expected_methods.len(),
            "OPTIONS {uri} returned an unexpected Allow set: {allow}"
        );
        for method in expected_methods {
            assert!(
                allowed_methods.contains(method),
                "OPTIONS {uri} must allow {method}; got {allow}"
            );
        }
    }
}

#[tokio::test]
#[serial]
async fn audit_log_routes_return_documented_response_shapes() {
    let (app, jwt) = build_app_with_admin_jwt().await;

    let response = send_request(
        &app,
        Method::GET,
        "/api/dashboard/audit-logs",
        Some(&jwt),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert!(body["items"].is_array());
    assert!(body["total"].is_i64());
    assert_eq!(body["page"], 1);
    assert_eq!(body["per_page"], 50);

    let response = send_request(
        &app,
        Method::GET,
        "/api/dashboard/audit-logs/stats",
        Some(&jwt),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert!(body["total_entries"].is_i64());
    assert!(body["by_method"].is_array());
    assert!(body["by_actor_type"].is_array());
    assert!(body["last_24h"].is_i64());

    let response = send_request(
        &app,
        Method::POST,
        "/api/dashboard/audit-logs/verify",
        Some(&jwt),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert!(body["valid"].is_boolean());
    assert!(body["batches_checked"].is_i64());
    assert!(body.get("tampered_batch").is_some());
    assert!(body.get("message").is_some());
}

#[tokio::test]
#[serial]
async fn benchmark_routes_return_acceptance_and_lookup_contracts() {
    let (app, jwt) = build_app_with_admin_jwt().await;
    let response = send_request(
        &app,
        Method::POST,
        "/api/benchmarks/tps",
        Some(&jwt),
        Some(json!({
            "model": "contract-model",
            "total_requests": 1,
            "concurrency": 1,
            "max_tokens": 1
        })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let accepted = response_json(response).await;
    assert_eq!(accepted["status"], "running");
    let run_id = accepted["run_id"]
        .as_str()
        .expect("accepted response run_id");

    let response = send_request(
        &app,
        Method::GET,
        &format!("/api/benchmarks/tps/{run_id}"),
        Some(&jwt),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let run = response_json(response).await;
    assert_eq!(run["run_id"], run_id);
    assert_eq!(run["request"]["model"], "contract-model");
    assert!(matches!(
        run["status"].as_str(),
        Some("running" | "failed" | "completed")
    ));
}

#[tokio::test]
#[serial]
async fn catalog_search_route_returns_models_contract() {
    let mock = MockServer::start().await;
    let query = format!("issue-696-{}", Uuid::new_v4());
    Mock::given(method("GET"))
        .and(path("/api/models"))
        .and(query_param("search", query.as_str()))
        .and(query_param("limit", "1"))
        .and(query_param("filter", "gguf"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "id": "owner/contract-model-GGUF",
            "tags": ["gguf"],
            "downloads": 7,
            "siblings": [{ "rfilename": "model.Q4_K_M.gguf" }]
        }])))
        .mount(&mock)
        .await;
    let hf_base_url = ScopedEnvVar::set("HF_BASE_URL", mock.uri());

    let (app, jwt) = build_app_with_admin_jwt().await;
    let response = send_request(
        &app,
        Method::GET,
        &format!("/api/catalog/search?q={query}&limit=1"),
        Some(&jwt),
        None,
    )
    .await;
    drop(hf_base_url);

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert!(body["models"].is_array());
    assert_eq!(body["models"][0]["repo_id"], "owner/contract-model-GGUF");
    assert_eq!(body["models"][0]["downloads"], 7);
}

#[tokio::test]
#[serial]
async fn safe_read_routes_return_documented_contracts() {
    let (app, jwt) = build_app_with_admin_jwt().await;

    let response = send_request(
        &app,
        Method::GET,
        "/api/system/update/schedule",
        Some(&jwt),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert!(body.get("schedule").is_some());
    assert!(body["schedule"].is_null());

    // Prometheus は未観測の metric family を gather 結果へ含めないため、
    // export 契約を検証する前に1サンプル記録する。
    llmlb::cloud_metrics::record("contract", 200, 1);
    let response = send_request(&app, Method::GET, "/api/metrics/cloud", Some(&jwt), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/plain")));
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read metrics response");
    let body = String::from_utf8(body.to_vec()).expect("metrics body must be UTF-8");
    assert!(body.contains("cloud_requests_total"));

    let response = send_request(&app, Method::GET, "/api/models/hub", Some(&jwt), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert!(body.is_array());
    assert!(body.as_array().is_some_and(Vec::is_empty));
}

#[tokio::test]
#[serial]
async fn websocket_contract_uses_dashboard_route_and_keeps_chat_route_removed() {
    // WebSocketUpgrade extractor は実サーバーが付与する OnUpgrade extension を必要とする。
    let server = crate::support::lb::spawn_test_lb().await;
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("build WebSocket contract client");

    let response = websocket_request(&client, server.addr(), "/ws/dashboard")
        .send()
        .await
        .expect("dashboard WebSocket response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let response = websocket_request(&client, server.addr(), "/ws/chat")
        .send()
        .await
        .expect("removed chat WebSocket response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
