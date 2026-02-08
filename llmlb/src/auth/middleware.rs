// T047-T049: 認証ミドルウェア実装

use crate::common::auth::{ApiKeyPermission, Claims, UserRole};
use crate::AppState;
use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use jsonwebtoken::decode_header;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[cfg(debug_assertions)]
const DEBUG_API_KEY_ALL: &str = "sk_debug";
#[cfg(debug_assertions)]
const DEBUG_API_KEY_RUNTIME: &str = "sk_debug_runtime";
#[cfg(debug_assertions)]
const DEBUG_API_KEY_API: &str = "sk_debug_api";
#[cfg(debug_assertions)]
const DEBUG_API_KEY_ADMIN: &str = "sk_debug_admin";

#[cfg(debug_assertions)]
fn debug_api_key_permissions(
    request_key: &str,
) -> Option<Vec<crate::common::auth::ApiKeyPermission>> {
    match request_key {
        DEBUG_API_KEY_ALL => Some(crate::common::auth::ApiKeyPermission::all()),
        DEBUG_API_KEY_RUNTIME => Some(vec![crate::common::auth::ApiKeyPermission::RegistryRead]),
        DEBUG_API_KEY_API => Some(vec![
            crate::common::auth::ApiKeyPermission::OpenaiInference,
            crate::common::auth::ApiKeyPermission::OpenaiModelsRead,
        ]),
        DEBUG_API_KEY_ADMIN => Some(crate::common::auth::ApiKeyPermission::all()),
        _ => None,
    }
}

#[cfg(not(debug_assertions))]
fn debug_api_key_permissions(
    _request_key: &str,
) -> Option<Vec<crate::common::auth::ApiKeyPermission>> {
    None
}

/// APIキー認証済みのコンテキスト
#[derive(Debug, Clone)]
pub struct ApiKeyAuthContext {
    /// APIキーID
    pub id: Uuid,
    /// APIキー発行者のユーザーID
    pub created_by: Uuid,
    /// APIキーの権限一覧
    pub permissions: Vec<crate::common::auth::ApiKeyPermission>,
    /// APIキーの有効期限
    pub expires_at: Option<DateTime<Utc>>,
}

fn has_permission(
    permissions: &[crate::common::auth::ApiKeyPermission],
    required: crate::common::auth::ApiKeyPermission,
) -> bool {
    permissions.contains(&required)
}

fn token_looks_like_jwt(token: &str) -> bool {
    let mut parts = token.split('.');
    let (first, second, third, extra) = (parts.next(), parts.next(), parts.next(), parts.next());
    if extra.is_some() {
        return false;
    }
    if matches!((first, second, third), (Some(a), Some(b), Some(c)) if !a.is_empty() && !b.is_empty() && !c.is_empty())
    {
        return decode_header(token).is_ok();
    }
    false
}

fn extract_jwt_from_headers(headers: &HeaderMap) -> Option<String> {
    if let Some(auth_header) = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
    {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            if token_looks_like_jwt(token) {
                return Some(token.to_string());
            }
        }
    }
    extract_jwt_cookie(headers)
}

pub(crate) fn extract_jwt_cookie(headers: &HeaderMap) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in cookie_header.split(';') {
        let trimmed = part.trim();
        if let Some(value) =
            trimmed.strip_prefix(&format!("{}=", crate::auth::DASHBOARD_JWT_COOKIE))
        {
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

pub(crate) fn extract_csrf_cookie(headers: &HeaderMap) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in cookie_header.split(';') {
        let trimmed = part.trim();
        if let Some(value) =
            trimmed.strip_prefix(&format!("{}=", crate::auth::DASHBOARD_CSRF_COOKIE))
        {
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn method_requires_csrf(method: &axum::http::Method) -> bool {
    matches!(
        *method,
        axum::http::Method::POST
            | axum::http::Method::PUT
            | axum::http::Method::PATCH
            | axum::http::Method::DELETE
    )
}

fn expected_origin(headers: &HeaderMap) -> Option<String> {
    let host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get(header::HOST))
        .and_then(|value| value.to_str().ok())?;
    let proto = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .unwrap_or("http");
    Some(format!("{}://{}", proto, host))
}

fn origin_or_referer(headers: &HeaderMap) -> Option<String> {
    if let Some(origin) = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
    {
        return Some(origin.to_string());
    }
    let referer = headers
        .get(header::REFERER)
        .and_then(|value| value.to_str().ok())?;
    if let Some((scheme, rest)) = referer.split_once("://") {
        let host = rest.split('/').next().unwrap_or_default();
        if !host.is_empty() {
            return Some(format!("{}://{}", scheme, host));
        }
    }
    None
}

fn origin_matches(headers: &HeaderMap) -> bool {
    let expected = match expected_origin(headers) {
        Some(value) => value,
        None => return false,
    };
    let provided = match origin_or_referer(headers) {
        Some(value) => value,
        None => return false,
    };
    provided.eq_ignore_ascii_case(&expected)
}

fn request_is_secure(headers: &HeaderMap) -> bool {
    if let Some(proto) = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
    {
        if proto.eq_ignore_ascii_case("https") {
            return true;
        }
    }
    if let Some(forwarded) = headers
        .get("forwarded")
        .and_then(|value| value.to_str().ok())
    {
        let lowered = forwarded.to_ascii_lowercase();
        if lowered.contains("proto=https") {
            return true;
        }
    }
    false
}

fn response_sets_csrf_cookie(response: &Response) -> bool {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|value| value.starts_with(crate::auth::DASHBOARD_CSRF_COOKIE))
}

async fn authenticate_api_key(
    pool: &sqlx::SqlitePool,
    api_key: &str,
) -> Result<ApiKeyAuthContext, Response> {
    if let Some(permissions) = debug_api_key_permissions(api_key) {
        tracing::warn!("Authenticated via debug API key (debug build only)");
        return Ok(ApiKeyAuthContext {
            id: Uuid::nil(),
            created_by: Uuid::nil(),
            permissions,
            expires_at: None,
        });
    }

    let key_hash = hash_with_sha256(api_key);
    let api_key_record = crate::db::api_keys::find_by_hash(pool, &key_hash)
        .await
        .map_err(|e| {
            tracing::warn!("API key verification failed: {}", e);
            (StatusCode::UNAUTHORIZED, "Invalid API key".to_string()).into_response()
        })?
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, "Invalid API key".to_string()).into_response())?;

    if let Some(expires_at) = api_key_record.expires_at {
        if expires_at < chrono::Utc::now() {
            return Err((StatusCode::UNAUTHORIZED, "API key expired".to_string()).into_response());
        }
    }

    Ok(ApiKeyAuthContext {
        id: api_key_record.id,
        created_by: api_key_record.created_by,
        permissions: api_key_record.permissions,
        expires_at: api_key_record.expires_at,
    })
}

#[allow(clippy::result_large_err)]
fn extract_api_key(request: &Request) -> Result<String, Response> {
    if let Some(api_key) = request
        .headers()
        .get("X-API-Key")
        .and_then(|h| h.to_str().ok())
    {
        return Ok(api_key.to_string());
    }

    if let Some(auth_header) = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
    {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            return Ok(token.to_string());
        }
        return Err((
            StatusCode::UNAUTHORIZED,
            "Invalid Authorization header format. Expected 'Bearer <token>'".to_string(),
        )
            .into_response());
    }

    Err((
        StatusCode::UNAUTHORIZED,
        "Missing X-API-Key header or Authorization header".to_string(),
    )
        .into_response())
}

/// JWT認証ミドルウェア
///
/// Authorizationヘッダーから "Bearer {token}" を抽出してJWT検証を行う
///
/// # Arguments
/// * `State(jwt_secret)` - JWT署名検証用のシークレットキー
/// * `request` - HTTPリクエスト
/// * `next` - 次のミドルウェア/ハンドラー
///
/// # Returns
/// * `Ok(Response)` - 認証成功、requestにClaimsを追加
/// * `Err(Response)` - 認証失敗、401 Unauthorized
pub async fn jwt_auth_middleware(
    State(jwt_secret): State<String>,
    mut request: Request,
    next: Next,
) -> Result<Response, Response> {
    // AuthorizationヘッダーまたはCookieからトークンを取得
    let token = if let Some(auth_header) = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
    {
        auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| {
                (
                    StatusCode::UNAUTHORIZED,
                    "Invalid Authorization header format".to_string(),
                )
                    .into_response()
            })?
            .to_string()
    } else if let Some(cookie_token) = extract_jwt_cookie(request.headers()) {
        cookie_token
    } else {
        return Err((
            StatusCode::UNAUTHORIZED,
            "Missing Authorization header or JWT cookie".to_string(),
        )
            .into_response());
    };

    // JWTを検証
    let claims = crate::auth::jwt::verify_jwt(&token, &jwt_secret).map_err(|e| {
        tracing::warn!("JWT verification failed: {}", e);
        (StatusCode::UNAUTHORIZED, format!("Invalid token: {}", e)).into_response()
    })?;

    // 検証済みのClaimsをrequestの拡張データに格納
    request.extensions_mut().insert(claims);

    // 次のミドルウェア/ハンドラーに進む
    Ok(next.run(request).await)
}

/// CookieベースのJWT認証時にCSRFトークンを要求するミドルウェア
pub async fn csrf_protect_middleware(request: Request, next: Next) -> Result<Response, Response> {
    if !method_requires_csrf(request.method()) {
        return Ok(next.run(request).await);
    }

    let headers_snapshot = request.headers().clone();

    // ヘッダー認証（APIキー/Authorization）はCSRF対象外（CookieベースのJWT認証のみ保護する）
    if request.headers().contains_key(header::AUTHORIZATION)
        || request.headers().contains_key("X-API-Key")
    {
        return Ok(next.run(request).await);
    }

    let csrf_cookie = extract_csrf_cookie(request.headers()).ok_or_else(|| {
        (StatusCode::FORBIDDEN, "Missing CSRF cookie".to_string()).into_response()
    })?;
    let csrf_header = request
        .headers()
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            (StatusCode::FORBIDDEN, "Missing CSRF header".to_string()).into_response()
        })?;

    if csrf_cookie != csrf_header {
        return Err((StatusCode::FORBIDDEN, "Invalid CSRF token".to_string()).into_response());
    }

    if !origin_matches(request.headers()) {
        return Err((
            StatusCode::FORBIDDEN,
            "Origin validation failed".to_string(),
        )
            .into_response());
    }

    let mut response = next.run(request).await;
    if response.status().is_success() && !response_sets_csrf_cookie(&response) {
        let new_token = crate::auth::generate_random_token(32);
        let secure = request_is_secure(&headers_snapshot);
        let cookie = crate::auth::build_csrf_cookie(&new_token, 86400, secure);
        response
            .headers_mut()
            .append(header::SET_COOKIE, cookie.parse().unwrap());
    }
    Ok(response)
}

/// APIキー認証ミドルウェア
///
/// X-API-KeyヘッダーまたはAuthorization: Bearer形式でキーを抽出してSHA-256で検証を行う
///
/// # Arguments
/// * `State(pool)` - データベース接続プール
/// * `request` - HTTPリクエスト
/// * `next` - 次のミドルウェア/ハンドラー
///
/// # Returns
/// * `Ok(Response)` - 認証成功
/// * `Err(Response)` - 認証失敗、401 Unauthorized
pub async fn api_key_auth_middleware(
    State(pool): State<sqlx::SqlitePool>,
    mut request: Request,
    next: Next,
) -> Result<Response, Response> {
    let api_key = extract_api_key(&request)?;
    let auth_context = authenticate_api_key(&pool, &api_key).await?;
    request.extensions_mut().insert(auth_context);

    Ok(next.run(request).await)
}

/// APIキーの権限を要求するミドルウェア
pub async fn require_api_key_permission_middleware(
    State(required_permission): State<ApiKeyPermission>,
    request: Request,
    next: Next,
) -> Result<Response, Response> {
    let auth_context = request
        .extensions()
        .get::<ApiKeyAuthContext>()
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                "Missing API key authentication".to_string(),
            )
                .into_response()
        })?;

    if !has_permission(&auth_context.permissions, required_permission) {
        return Err((
            StatusCode::FORBIDDEN,
            "Insufficient API key permission".to_string(),
        )
            .into_response());
    }

    Ok(next.run(request).await)
}

/// JWTまたはAPIキー(permissions)で認証し、必要な権限を満たすことを要求するミドルウェア。
///
/// - JWTが存在する場合はJWTを優先（Authorization Bearer / Cookie）。
/// - APIキーは `X-API-Key` または `Authorization: Bearer sk_...` を許可。
///
/// NOTE:
/// - `jwt_required_role` が `Some(Admin)` の場合、JWTはadminのみ許可。
/// - APIキーは `required_permission` を必須とし、成功時に `api_key_role` で Claims を注入する。
#[derive(Clone)]
pub struct JwtOrApiKeyPermissionConfig {
    /// アプリケーション状態（DB/JWT secret 参照用）
    pub app_state: AppState,
    /// APIキーに要求する権限
    pub required_permission: ApiKeyPermission,
    /// JWTに要求するロール（Noneの場合は任意ロールを許可）
    pub jwt_required_role: Option<UserRole>,
    /// APIキー認証成功時に注入するClaimsのロール
    pub api_key_role: UserRole,
}

/// `JwtOrApiKeyPermissionConfig` に従って、JWTまたはAPIキーで認証・認可を行う。
pub async fn jwt_or_api_key_permission_middleware(
    State(config): State<JwtOrApiKeyPermissionConfig>,
    mut request: Request,
    next: Next,
) -> Result<Response, Response> {
    // JWTがあれば優先
    if let Some(token) = extract_jwt_from_headers(request.headers()) {
        let claims =
            crate::auth::jwt::verify_jwt(&token, &config.app_state.jwt_secret).map_err(|e| {
                tracing::warn!("JWT verification failed: {}", e);
                (StatusCode::UNAUTHORIZED, format!("Invalid token: {}", e)).into_response()
            })?;

        if let Some(required_role) = config.jwt_required_role {
            if claims.role != required_role {
                return Err(
                    (StatusCode::FORBIDDEN, "Admin access required".to_string()).into_response()
                );
            }
        }

        request.extensions_mut().insert(claims);
        return Ok(next.run(request).await);
    }

    // JWTがない/無効ならAPIキーで認証
    let api_key = extract_api_key(&request)?;
    let auth_context = authenticate_api_key(&config.app_state.db_pool, &api_key).await?;

    if !has_permission(&auth_context.permissions, config.required_permission) {
        let permission_str = serde_json::to_string(&config.required_permission)
            .unwrap_or_else(|_| "\"unknown\"".to_string());
        let permission_str = permission_str.trim_matches('"');
        return Err((
            StatusCode::FORBIDDEN,
            format!("Missing required permission: {}", permission_str),
        )
            .into_response());
    }

    // APIキーの発行者の情報でClaimsを構築
    let exp = auth_context
        .expires_at
        .map(|dt| dt.timestamp() as usize)
        .unwrap_or_else(|| (Utc::now() + chrono::Duration::hours(24)).timestamp() as usize);
    let claims = Claims {
        sub: auth_context.created_by.to_string(),
        role: config.api_key_role,
        exp,
    };
    request.extensions_mut().insert(claims);
    request.extensions_mut().insert(auth_context);

    Ok(next.run(request).await)
}

// SPEC-66555000: APIキー or ノードトークン認証ミドルウェアは廃止されました
// api_key_or_node_token_auth_middleware と node_token_auth_middleware は削除されました
// 新しい実装は POST /api/endpoints を使用してください

/// SHA-256ハッシュ化ヘルパー関数
///
/// # Arguments
/// * `input` - ハッシュ化する文字列
///
/// # Returns
/// * `String` - 16進数表現のSHA-256ハッシュ（64文字）
fn hash_with_sha256(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
}

/// LLMLB_AUTH_DISABLED（旧: AUTH_DISABLED）用ダミーClaims注入ミドルウェア
///
/// 認証無効化モードの場合、すべてのリクエストにダミーのAdmin Claimsを注入する
/// これにより、Extension<Claims>を要求するハンドラーが正常に動作する
///
/// # Arguments
/// * `request` - HTTPリクエスト
/// * `next` - 次のミドルウェア/ハンドラー
///
/// # Returns
/// * `Response` - レスポンス
pub async fn inject_dummy_admin_claims(mut request: Request, next: Next) -> Response {
    // ダミーのAdmin Claimsを作成
    let dummy_claims = Claims {
        sub: "00000000-0000-0000-0000-000000000000".to_string(), // ダミーUUID
        role: UserRole::Admin,
        exp: (chrono::Utc::now() + chrono::Duration::hours(24)).timestamp() as usize,
    };

    // リクエストの拡張データに格納
    request.extensions_mut().insert(dummy_claims);

    next.run(request).await
}

/// LLMLB_AUTH_DISABLED（旧: AUTH_DISABLED）用ダミーClaims注入ミドルウェア（管理者ID参照）
///
/// 既存の管理者ユーザーIDを取得してClaimsへ設定する。
/// 管理者が存在しない場合はnil UUIDを使用する。
pub async fn inject_dummy_admin_claims_with_state(
    State(app_state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let admin_id = match crate::db::users::find_any_admin_id(&app_state.db_pool).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            tracing::warn!("No admin user found; using nil UUID for dummy claims");
            Uuid::nil()
        }
        Err(e) => {
            tracing::error!("Failed to lookup admin user: {}", e);
            Uuid::nil()
        }
    };

    let dummy_claims = Claims {
        sub: admin_id.to_string(),
        role: UserRole::Admin,
        exp: (chrono::Utc::now() + chrono::Duration::hours(24)).timestamp() as usize,
    };

    request.extensions_mut().insert(dummy_claims);

    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::balancer::LoadManager;
    use axum::{body::Body, http::Request, middleware as axum_middleware, routing::get, Router};
    use std::sync::Arc;
    use tower::ServiceExt;

    #[test]
    fn test_hash_with_sha256() {
        let input = "test_api_key_12345";
        let hash = hash_with_sha256(input);

        // SHA-256ハッシュは64文字の16進数
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));

        // 同じ入力は同じハッシュを生成
        let hash2 = hash_with_sha256(input);
        assert_eq!(hash, hash2);

        // 異なる入力は異なるハッシュを生成
        let hash3 = hash_with_sha256("different_input");
        assert_ne!(hash, hash3);
    }

    #[tokio::test]
    async fn dummy_admin_claims_use_existing_admin_id() {
        let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .expect("create sqlite pool");
        sqlx::migrate!("./migrations")
            .run(&db_pool)
            .await
            .expect("Failed to run migrations");

        let admin = crate::db::users::create(&db_pool, "admin-user", "hash", UserRole::Admin)
            .await
            .expect("create admin user");

        let request_history = std::sync::Arc::new(
            crate::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
        );
        let endpoint_registry = crate::registry::endpoints::EndpointRegistry::new(db_pool.clone())
            .await
            .expect("Failed to create endpoint registry");
        let endpoint_registry_arc = Arc::new(endpoint_registry.clone());
        let load_manager = LoadManager::new(endpoint_registry_arc);
        let state = crate::AppState {
            load_manager,
            request_history,
            db_pool,
            jwt_secret: "test-secret".to_string(),
            http_client: reqwest::Client::new(),
            queue_config: crate::config::QueueConfig::from_env(),
            event_bus: crate::events::create_shared_event_bus(),
            endpoint_registry,
        };

        let app = Router::new()
            .route(
                "/t",
                get(
                    |axum::extract::Extension(claims): axum::extract::Extension<Claims>| async move {
                        claims.sub
                    },
                ),
            )
            .layer(axum_middleware::from_fn_with_state(
                state,
                inject_dummy_admin_claims_with_state,
            ));

        let res = app
            .oneshot(Request::builder().uri("/t").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = std::str::from_utf8(&body).unwrap();
        assert_eq!(body_str, admin.id.to_string());
    }

    #[cfg(debug_assertions)]
    #[tokio::test]
    async fn admin_middleware_allows_bearer_api_key() {
        let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .expect("create sqlite pool");
        sqlx::migrate!("./migrations")
            .run(&db_pool)
            .await
            .expect("Failed to run migrations");
        let request_history = std::sync::Arc::new(
            crate::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
        );
        let endpoint_registry = crate::registry::endpoints::EndpointRegistry::new(db_pool.clone())
            .await
            .expect("Failed to create endpoint registry");
        let endpoint_registry_arc = Arc::new(endpoint_registry.clone());
        let load_manager = LoadManager::new(endpoint_registry_arc);
        let state = crate::AppState {
            load_manager,
            request_history,
            db_pool,
            jwt_secret: "test-secret".to_string(),
            http_client: reqwest::Client::new(),
            queue_config: crate::config::QueueConfig::from_env(),
            event_bus: crate::events::create_shared_event_bus(),
            endpoint_registry,
        };

        let cfg = JwtOrApiKeyPermissionConfig {
            app_state: state,
            required_permission: ApiKeyPermission::UsersManage,
            jwt_required_role: Some(UserRole::Admin),
            api_key_role: UserRole::Admin,
        };
        let app = Router::new().route("/admin", get(|| async { "ok" })).layer(
            axum_middleware::from_fn_with_state(cfg, jwt_or_api_key_permission_middleware),
        );

        let res = app
            .oneshot(
                Request::builder()
                    .uri("/admin")
                    .header("authorization", format!("Bearer {}", DEBUG_API_KEY_ADMIN))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::OK);
    }

    #[cfg(debug_assertions)]
    #[tokio::test]
    async fn admin_middleware_rejects_invalid_jwt_even_with_api_key() {
        let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .expect("create sqlite pool");
        sqlx::migrate!("./migrations")
            .run(&db_pool)
            .await
            .expect("Failed to run migrations");
        let request_history = std::sync::Arc::new(
            crate::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
        );
        let endpoint_registry = crate::registry::endpoints::EndpointRegistry::new(db_pool.clone())
            .await
            .expect("Failed to create endpoint registry");
        let endpoint_registry_arc = Arc::new(endpoint_registry.clone());
        let load_manager = LoadManager::new(endpoint_registry_arc);
        let state = crate::AppState {
            load_manager,
            request_history,
            db_pool,
            jwt_secret: "test-secret".to_string(),
            http_client: reqwest::Client::new(),
            queue_config: crate::config::QueueConfig::from_env(),
            event_bus: crate::events::create_shared_event_bus(),
            endpoint_registry,
        };

        let cfg = JwtOrApiKeyPermissionConfig {
            app_state: state,
            required_permission: ApiKeyPermission::UsersManage,
            jwt_required_role: Some(UserRole::Admin),
            api_key_role: UserRole::Admin,
        };
        let app = Router::new().route("/admin", get(|| async { "ok" })).layer(
            axum_middleware::from_fn_with_state(cfg, jwt_or_api_key_permission_middleware),
        );

        let res = app
            .oneshot(
                Request::builder()
                    .uri("/admin")
                    .header(
                        "authorization",
                        "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJhZG1pbiIsInJvbGUiOiJhZG1pbiIsImV4cCI6MjAwMDAwMDAwMH0.invalidsig",
                    )
                    .header("x-api-key", DEBUG_API_KEY_ADMIN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[cfg(debug_assertions)]
    #[tokio::test]
    async fn debug_api_key_is_accepted_in_debug_build_without_db() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .expect("create sqlite pool");

        let app = axum::Router::new()
            .route(
                "/t",
                axum::routing::get(
                    |axum::extract::Extension(auth): axum::extract::Extension<
                        ApiKeyAuthContext,
                    >| async move {
                        format!("{}:{}", auth.id, auth.permissions.len())
                    },
                ),
            )
            .layer(axum::middleware::from_fn_with_state(
                pool,
                api_key_auth_middleware,
            ));

        let res = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/t")
                    .header("x-api-key", DEBUG_API_KEY_ALL)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = std::str::from_utf8(&body).unwrap();
        assert!(body_str.starts_with(&Uuid::nil().to_string()));
        assert!(body_str.contains(&ApiKeyPermission::all().len().to_string()));
    }

    #[cfg(not(debug_assertions))]
    #[tokio::test]
    async fn debug_api_key_is_rejected_in_release_build() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .expect("create sqlite pool");

        let app = axum::Router::new()
            .route("/t", axum::routing::get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(
                pool,
                api_key_auth_middleware,
            ));

        let res = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/t")
                    .header("x-api-key", "sk_debug")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }
}
