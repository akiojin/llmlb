// T044-T046: JWT生成と検証（jsonwebtoken実装）

use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use llmlb_common::auth::{Claims, UserRole};
use llmlb_common::error::RouterError;

/// JWT有効期限（24時間）
const JWT_EXPIRATION_HOURS: i64 = 24;

/// JWTトークンを生成
///
/// # Arguments
/// * `user_id` - ユーザーID
/// * `role` - ユーザーロール
/// * `secret` - JWTシークレットキー
///
/// # Returns
/// * `Ok(String)` - JWTトークン（3つのドット区切り部分）
/// * `Err(RouterError)` - 生成失敗
pub fn create_jwt(user_id: &str, role: UserRole, secret: &str) -> Result<String, RouterError> {
    let expiration = Utc::now()
        .checked_add_signed(chrono::Duration::hours(JWT_EXPIRATION_HOURS))
        .ok_or_else(|| RouterError::Jwt("Failed to calculate expiration time".to_string()))?
        .timestamp() as usize;

    let claims = Claims {
        sub: user_id.to_string(),
        role,
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| RouterError::Jwt(format!("Failed to create JWT: {}", e)))
}

/// JWTトークンを検証
///
/// # Arguments
/// * `token` - 検証するJWTトークン
/// * `secret` - JWTシークレットキー
///
/// # Returns
/// * `Ok(Claims)` - 検証済みクレーム
/// * `Err(RouterError)` - 検証失敗（無効なトークン、期限切れなど）
pub fn verify_jwt(token: &str, secret: &str) -> Result<Claims, RouterError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| RouterError::Jwt(format!("Failed to verify JWT: {}", e)))
}
