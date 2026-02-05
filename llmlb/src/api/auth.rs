//! 認証API
//!
//! ログイン、ログアウト、認証情報確認

use crate::common::auth::{Claims, UserRole};
use crate::{config, AppState};
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::{Deserialize, Serialize};

/// ログインリクエスト
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// ユーザー名
    pub username: String,
    /// パスワード
    pub password: String,
}

/// ログインレスポンス
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    /// JWTトークン
    pub token: String,
    /// トークン有効期限（秒）
    pub expires_in: usize,
    /// ユーザー情報
    pub user: UserInfo,
}

/// ユーザー情報（ログインレスポンス用）
#[derive(Debug, Serialize)]
pub struct UserInfo {
    /// ユーザーID
    pub id: String,
    /// ユーザー名
    pub username: String,
    /// ロール
    pub role: String,
}

/// 認証情報レスポンス
#[derive(Debug, Serialize)]
pub struct MeResponse {
    /// ユーザーID
    pub user_id: String,
    /// ユーザー名
    pub username: String,
    /// ロール
    pub role: String,
}

/// POST /api/auth/login - ログイン
///
/// ユーザー名とパスワードで認証し、JWTトークンを発行
///
/// # Arguments
/// * `State(app_state)` - アプリケーション状態（db_pool, jwt_secret）
/// * `Json(request)` - ログインリクエスト（username, password）
///
/// # Returns
/// * `200 OK` - ログイン成功（JWT token）
/// * `401 Unauthorized` - 認証失敗
/// * `500 Internal Server Error` - サーバーエラー
pub async fn login(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<impl IntoResponse, Response> {
    let is_secure = is_request_secure(&headers);

    // 開発モード: admin/test で固定ログイン可能
    #[cfg(debug_assertions)]
    if request.username == "admin" && request.password == "test" {
        let dev_user_id = uuid::Uuid::nil().to_string();
        let expires_in = 86400;
        let token = crate::auth::jwt::create_jwt(
            &dev_user_id,
            crate::common::auth::UserRole::Admin,
            &app_state.jwt_secret,
        )
        .map_err(|e| {
            tracing::error!("Failed to create JWT: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
        })?;

        tracing::info!("Development mode login: admin/test");
        let cookie = crate::auth::build_jwt_cookie(&token, expires_in, is_secure);
        return Ok((
            StatusCode::OK,
            [(header::SET_COOKIE, cookie)],
            Json(LoginResponse {
                token,
                expires_in,
                user: UserInfo {
                    id: dev_user_id,
                    username: "admin".to_string(),
                    role: "admin".to_string(),
                },
            }),
        ));
    }

    // ユーザーを検索
    let user = crate::db::users::find_by_username(&app_state.db_pool, &request.username)
        .await
        .map_err(|e| {
            tracing::error!("Failed to find user: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
        })?
        .ok_or_else(|| {
            (StatusCode::UNAUTHORIZED, "Invalid username or password").into_response()
        })?;

    // パスワードを検証
    let is_valid = crate::auth::password::verify_password(&request.password, &user.password_hash)
        .map_err(|e| {
        tracing::error!("Failed to verify password: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
    })?;

    if !is_valid {
        return Err((StatusCode::UNAUTHORIZED, "Invalid username or password").into_response());
    }

    // 最終ログイン時刻を更新
    crate::db::users::update_last_login(&app_state.db_pool, user.id)
        .await
        .map_err(|e| {
            tracing::warn!("Failed to update last login: {}", e);
            // エラーだがログイン自体は成功させる
        })
        .ok();

    // JWTを生成（有効期限24時間）
    let expires_in = 86400; // 24時間（秒）
    let token =
        crate::auth::jwt::create_jwt(&user.id.to_string(), user.role, &app_state.jwt_secret)
            .map_err(|e| {
                tracing::error!("Failed to create JWT: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
            })?;

    let cookie = crate::auth::build_jwt_cookie(&token, expires_in, is_secure);
    Ok((
        StatusCode::OK,
        [(header::SET_COOKIE, cookie)],
        Json(LoginResponse {
            token,
            expires_in,
            user: UserInfo {
                id: user.id.to_string(),
                username: user.username,
                role: format!("{:?}", user.role).to_lowercase(),
            },
        }),
    ))
}

/// POST /api/auth/logout - ログアウト
///
/// JWTはステートレスなのでクライアント側でトークンを破棄するだけ
/// Cookieの削除ヘッダーも返す
///
/// # Returns
/// * `204 No Content` - ログアウト成功
pub async fn logout(headers: HeaderMap) -> impl IntoResponse {
    let is_secure = is_request_secure(&headers);
    let cookie = crate::auth::clear_jwt_cookie(is_secure);
    (StatusCode::NO_CONTENT, [(header::SET_COOKIE, cookie)])
}

fn is_request_secure(headers: &HeaderMap) -> bool {
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

/// GET /api/auth/me - 認証情報確認
///
/// 現在の認証済みユーザー情報を返す
///
/// # Arguments
/// * `Extension(claims)` - JWTクレーム（ミドルウェアで注入）
/// * `State(app_state)` - アプリケーション状態
///
/// # Returns
/// * `200 OK` - ユーザー情報
/// * `401 Unauthorized` - 認証されていない
/// * `404 Not Found` - ユーザーが見つからない
/// * `500 Internal Server Error` - サーバーエラー
pub async fn me(
    Extension(claims): Extension<Claims>,
    State(app_state): State<AppState>,
) -> Result<Json<MeResponse>, Response> {
    if config::is_auth_disabled() {
        return Ok(Json(MeResponse {
            user_id: claims.sub.clone(),
            username: "admin".to_string(),
            role: format!("{:?}", UserRole::Admin).to_lowercase(),
        }));
    }

    // ユーザーIDをパース
    let user_id = claims.sub.parse::<uuid::Uuid>().map_err(|e| {
        tracing::error!("Failed to parse user ID: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
    })?;

    // 開発モード: nil UUIDの場合は開発ユーザー情報を返す
    #[cfg(debug_assertions)]
    if user_id.is_nil() {
        return Ok(Json(MeResponse {
            user_id: user_id.to_string(),
            username: "admin".to_string(),
            role: "admin".to_string(),
        }));
    }

    // ユーザー情報を取得
    let user = crate::db::users::find_by_id(&app_state.db_pool, user_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to find user: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
        })?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "User not found").into_response())?;

    Ok(Json(MeResponse {
        user_id: user.id.to_string(),
        username: user.username,
        role: format!("{:?}", user.role).to_lowercase(),
    }))
}

/// ユーザー登録リクエスト
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    /// 招待コード
    pub invitation_code: String,
    /// ユーザー名
    pub username: String,
    /// パスワード
    pub password: String,
}

/// ユーザー登録レスポンス
#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    /// ユーザーID
    pub id: String,
    /// ユーザー名
    pub username: String,
    /// ロール
    pub role: String,
    /// 作成日時
    pub created_at: String,
}

/// POST /api/auth/register - 招待コードでユーザー登録
///
/// 有効な招待コードを使ってユーザー登録する（認証不要）
///
/// # Arguments
/// * `State(app_state)` - アプリケーション状態
/// * `Json(request)` - 登録リクエスト（招待コード、ユーザー名、パスワード）
///
/// # Returns
/// * `201 Created` - 登録成功
/// * `400 Bad Request` - 招待コードが無効/期限切れ/使用済み
/// * `409 Conflict` - ユーザー名が既に存在
/// * `500 Internal Server Error` - サーバーエラー
pub async fn register(
    State(app_state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<RegisterResponse>), Response> {
    // 招待コードを検索
    let invitation =
        crate::db::invitations::find_by_code(&app_state.db_pool, &request.invitation_code)
            .await
            .map_err(|e| {
                tracing::error!("Failed to find invitation code: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
            })?
            .ok_or_else(|| (StatusCode::BAD_REQUEST, "Invalid invitation code").into_response())?;

    // 招待コードが有効かチェック
    crate::db::invitations::validate_invitation(&invitation).map_err(|e| {
        tracing::info!("Invalid invitation code: {}", e);
        (StatusCode::BAD_REQUEST, e.to_string()).into_response()
    })?;

    // ユーザー名の重複チェック
    let existing = crate::db::users::find_by_username(&app_state.db_pool, &request.username)
        .await
        .map_err(|e| {
            tracing::error!("Failed to check username: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
        })?;

    if existing.is_some() {
        return Err((StatusCode::CONFLICT, "Username already exists").into_response());
    }

    // パスワードをハッシュ化
    let password_hash = crate::auth::password::hash_password(&request.password).map_err(|e| {
        tracing::error!("Failed to hash password: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
    })?;

    // ユーザーを作成（招待登録は常にviewer）
    let user = crate::db::users::create(
        &app_state.db_pool,
        &request.username,
        &password_hash,
        UserRole::Viewer,
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to create user: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
    })?;

    // 招待コードを使用済みにする
    crate::db::invitations::mark_as_used(&app_state.db_pool, invitation.id, user.id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to mark invitation as used: {}", e);
            // ユーザーは既に作成されているため、エラーでもロールバックしない
            // ここでのエラーはログに記録するが、ユーザー作成は成功として扱う
        })
        .ok();

    tracing::info!(
        "User registered via invitation: {} (id={})",
        user.username,
        user.id
    );

    Ok((
        StatusCode::CREATED,
        Json(RegisterResponse {
            id: user.id.to_string(),
            username: user.username,
            role: format!("{:?}", UserRole::Viewer).to_lowercase(),
            created_at: user.created_at.to_rfc3339(),
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[tokio::test]
    async fn test_logout_returns_no_content() {
        let response = logout(HeaderMap::new()).await.into_response();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[test]
    fn test_login_request_deserialize() {
        let json = r#"{"username": "admin", "password": "secret"}"#;
        let request: LoginRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.username, "admin");
        assert_eq!(request.password, "secret");
    }

    #[test]
    fn test_login_response_serialize() {
        let response = LoginResponse {
            token: "jwt_token".to_string(),
            expires_in: 86400,
            user: UserInfo {
                id: "user-id".to_string(),
                username: "admin".to_string(),
                role: "admin".to_string(),
            },
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("jwt_token"));
        assert!(json.contains("86400"));
        assert!(json.contains("admin"));
    }

    #[test]
    fn test_me_response_serialize() {
        let response = MeResponse {
            user_id: "user-123".to_string(),
            username: "testuser".to_string(),
            role: "user".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("user-123"));
        assert!(json.contains("testuser"));
        assert!(json.contains("user"));
    }

    #[test]
    fn test_user_info_serialize() {
        let info = UserInfo {
            id: "id-456".to_string(),
            username: "bob".to_string(),
            role: "admin".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("id-456"));
        assert!(json.contains("bob"));
        assert!(json.contains("admin"));
    }
}
