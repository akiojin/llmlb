// T042-T043: パスワードハッシュ化と検証（bcrypt実装）

use bcrypt::{hash, verify};
use llmlb_common::error::RouterError;

/// パスワードハッシュ化のコスト（12推奨、200-300ms）
const HASH_COST: u32 = 12;

/// パスワードをbcryptでハッシュ化
///
/// # Arguments
/// * `password` - ハッシュ化するパスワード
///
/// # Returns
/// * `Ok(String)` - bcryptハッシュ文字列（$2b$で始まる）
/// * `Err(RouterError)` - ハッシュ化失敗
pub fn hash_password(password: &str) -> Result<String, RouterError> {
    hash(password, HASH_COST)
        .map_err(|e| RouterError::PasswordHash(format!("Failed to hash password: {}", e)))
}

/// パスワードを検証
///
/// # Arguments
/// * `password` - 検証する平文パスワード
/// * `hash` - bcryptハッシュ文字列
///
/// # Returns
/// * `Ok(true)` - パスワード一致
/// * `Ok(false)` - パスワード不一致
/// * `Err(RouterError)` - 検証失敗
pub fn verify_password(password: &str, hash: &str) -> Result<bool, RouterError> {
    verify(password, hash)
        .map_err(|e| RouterError::PasswordHash(format!("Failed to verify password: {}", e)))
}
