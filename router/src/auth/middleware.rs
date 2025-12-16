// T047-T049: 認証ミドルウェア実装

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use llm_router_common::auth::{Claims, UserRole};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[cfg(debug_assertions)]
const DEBUG_API_KEY: &str = "sk_debug";

#[cfg(debug_assertions)]
fn debug_api_key_is_valid(request_key: &str) -> bool {
    request_key == DEBUG_API_KEY
}

#[cfg(not(debug_assertions))]
fn debug_api_key_is_valid(_request_key: &str) -> bool {
    false
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
    // Authorizationヘッダーを取得
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                "Missing Authorization header".to_string(),
            )
                .into_response()
        })?;

    // "Bearer {token}" から token を抽出
    let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Invalid Authorization header format".to_string(),
        )
            .into_response()
    })?;

    // JWTを検証
    let claims = crate::auth::jwt::verify_jwt(token, &jwt_secret).map_err(|e| {
        tracing::warn!("JWT verification failed: {}", e);
        (StatusCode::UNAUTHORIZED, format!("Invalid token: {}", e)).into_response()
    })?;

    // 検証済みのClaimsをrequestの拡張データに格納
    request.extensions_mut().insert(claims);

    // 次のミドルウェア/ハンドラーに進む
    Ok(next.run(request).await)
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
    // X-API-Keyヘッダーまたは Authorization: Bearer トークンを取得
    let api_key = if let Some(api_key) = request
        .headers()
        .get("X-API-Key")
        .and_then(|h| h.to_str().ok())
    {
        // X-API-Keyヘッダーから取得
        api_key.to_string()
    } else if let Some(auth_header) = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
    {
        // Authorization: Bearer トークンから取得
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            token.to_string()
        } else {
            return Err((
                StatusCode::UNAUTHORIZED,
                "Invalid Authorization header format. Expected 'Bearer <token>'".to_string(),
            )
                .into_response());
        }
    } else {
        return Err((
            StatusCode::UNAUTHORIZED,
            "Missing X-API-Key header or Authorization header".to_string(),
        )
            .into_response());
    };

    // デバッグビルド時のみ: 固定のデバッグ用APIキーを許可
    if debug_api_key_is_valid(&api_key) {
        tracing::warn!("Authenticated via debug API key (debug build only)");
        request.extensions_mut().insert(Uuid::nil());
        return Ok(next.run(request).await);
    }

    // SHA-256ハッシュ化
    let key_hash = hash_with_sha256(&api_key);

    // データベースでAPIキーを検証
    let api_key_record = crate::db::api_keys::find_by_hash(&pool, &key_hash)
        .await
        .map_err(|e| {
            tracing::warn!("API key verification failed: {}", e);
            (StatusCode::UNAUTHORIZED, "Invalid API key".to_string()).into_response()
        })?
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, "Invalid API key".to_string()).into_response())?;

    // 有効期限チェック
    if let Some(expires_at) = api_key_record.expires_at {
        if expires_at < chrono::Utc::now() {
            return Err((StatusCode::UNAUTHORIZED, "API key expired".to_string()).into_response());
        }
    }

    // APIキーIDをrequestの拡張データに格納
    request.extensions_mut().insert(api_key_record.id);

    Ok(next.run(request).await)
}

/// APIキー or ノードトークン認証ミドルウェア
///
/// `/v1/models*` のように「外部クライアント(APIキー)」と「ノード(ノードトークン)」の両方から
/// アクセスされるエンドポイント向け。
///
/// 優先順位:
/// 1. `X-Node-Token` が存在する場合はノードトークンで認証
/// 2. それ以外は APIキー（`X-API-Key` または `Authorization: Bearer`）で認証
pub async fn api_key_or_node_token_auth_middleware(
    State(pool): State<sqlx::SqlitePool>,
    mut request: Request,
    next: Next,
) -> Result<Response, Response> {
    // まず X-Node-Token があればノード認証を優先
    if let Some(node_token) = request
        .headers()
        .get("X-Node-Token")
        .and_then(|h| h.to_str().ok())
    {
        let token_hash = hash_with_sha256(node_token);
        let node_token_record = crate::db::node_tokens::find_by_hash(&pool, &token_hash)
            .await
            .map_err(|e| {
                tracing::warn!("Node token verification failed: {}", e);
                (StatusCode::UNAUTHORIZED, "Invalid node token".to_string()).into_response()
            })?
            .ok_or_else(|| {
                (StatusCode::UNAUTHORIZED, "Invalid node token".to_string()).into_response()
            })?;

        request.extensions_mut().insert(node_token_record.node_id);
        return Ok(next.run(request).await);
    }

    // 次に APIキーで認証
    let api_key = if let Some(api_key) = request
        .headers()
        .get("X-API-Key")
        .and_then(|h| h.to_str().ok())
    {
        api_key.to_string()
    } else if let Some(auth_header) = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
    {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            token.to_string()
        } else {
            return Err((
                StatusCode::UNAUTHORIZED,
                "Invalid Authorization header format. Expected 'Bearer <token>'".to_string(),
            )
                .into_response());
        }
    } else {
        return Err((
            StatusCode::UNAUTHORIZED,
            "Missing X-Node-Token header or API key".to_string(),
        )
            .into_response());
    };

    // デバッグビルド時のみ: 固定のデバッグ用APIキーを許可
    if debug_api_key_is_valid(&api_key) {
        tracing::warn!("Authenticated via debug API key (debug build only)");
        request.extensions_mut().insert(Uuid::nil());
        return Ok(next.run(request).await);
    }

    let key_hash = hash_with_sha256(&api_key);
    let api_key_record = crate::db::api_keys::find_by_hash(&pool, &key_hash)
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

    request.extensions_mut().insert(api_key_record.id);
    Ok(next.run(request).await)
}

/// ノードトークン認証ミドルウェア
///
/// X-Node-Tokenヘッダーからトークンを抽出してSHA-256で検証を行う
///
/// # Arguments
/// * `State(pool)` - データベース接続プール
/// * `request` - HTTPリクエスト
/// * `next` - 次のミドルウェア/ハンドラー
///
/// # Returns
/// * `Ok(Response)` - 認証成功
/// * `Err(Response)` - 認証失敗、401 Unauthorized
pub async fn node_token_auth_middleware(
    State(pool): State<sqlx::SqlitePool>,
    mut request: Request,
    next: Next,
) -> Result<Response, Response> {
    // X-Node-Tokenヘッダーを取得
    let node_token = request
        .headers()
        .get("X-Node-Token")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                "Missing X-Node-Token header".to_string(),
            )
                .into_response()
        })?;

    // SHA-256ハッシュ化
    let token_hash = hash_with_sha256(node_token);

    // データベースでノードトークンを検証
    let node_token_record = crate::db::node_tokens::find_by_hash(&pool, &token_hash)
        .await
        .map_err(|e| {
            tracing::warn!("Node token verification failed: {}", e);
            (StatusCode::UNAUTHORIZED, "Invalid node token".to_string()).into_response()
        })?
        .ok_or_else(|| {
            (StatusCode::UNAUTHORIZED, "Invalid node token".to_string()).into_response()
        })?;

    // ノードIDをrequestの拡張データに格納
    request.extensions_mut().insert(node_token_record.node_id);

    Ok(next.run(request).await)
}

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

/// AUTH_DISABLED用ダミーClaims注入ミドルウェア
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

#[cfg(test)]
mod tests {
    use super::*;
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
                    |axum::extract::Extension(api_key_id): axum::extract::Extension<Uuid>| async move {
                        api_key_id.to_string()
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
                    .header("x-api-key", DEBUG_API_KEY)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&body[..], Uuid::nil().to_string().as_bytes());
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
