// T-0004-T-0007: 招待キー生成・検証（TDD実装フェーズ）

use crate::common::error::LbError;

/// 招待キーを生成
///
/// # 仕様
/// - 8文字のランダム英数字
/// - 大文字、小文字、数字が含まれている
/// - 毎回異なる値が生成される
///
/// # Returns
/// * `String` - 生成された招待キー（例: A3X7B9K2）
pub fn generate_invitation_key() -> String {
    use crate::auth::generate_random_token;
    generate_random_token(8)
}

/// 招待キーを検証
///
/// # 仕様
/// - 有効期限確認（7日間）
/// - 一度のみ使用チェック
/// - フォーマット検証
///
/// # Arguments
/// * `key` - 検証する招待キー
/// * `user_id` - ユーザーID
///
/// # Returns
/// * `Ok(bool)` - 検証結果
/// * `Err(LbError)` - 検証エラー
pub fn verify_invitation_key(_key: &str, _user_id: i32) -> Result<bool, LbError> {
    // TODO: T-0007で実装（現在はダミー）
    Err(LbError::Common(
        crate::common::error::CommonError::Validation("Not implemented".to_string()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- T-0004: 招待キー生成関数のテスト実装（TDD RED フェーズ） ---
    // generate_invitation_key テストスイート

    #[test]
    fn generate_invitation_key_returns_8_characters() {
        // 生成されたキーが8文字であること
        let key = generate_invitation_key();
        assert_eq!(
            key.len(),
            8,
            "Generated invitation key should be exactly 8 characters long"
        );
    }

    #[test]
    fn generate_invitation_key_contains_only_alphanumeric() {
        // 生成されたキーが英数字のみで構成されていること
        let key = generate_invitation_key();
        assert!(
            key.chars().all(|c| c.is_alphanumeric()),
            "Generated invitation key should contain only alphanumeric characters"
        );
    }

    #[test]
    fn generate_invitation_key_produces_different_values() {
        // 毎回異なる値が生成されること
        let key1 = generate_invitation_key();
        let key2 = generate_invitation_key();
        let key3 = generate_invitation_key();

        assert_ne!(
            key1, key2,
            "Generated invitation keys should be different each time"
        );
        assert_ne!(
            key2, key3,
            "Generated invitation keys should be different each time"
        );
        assert_ne!(
            key1, key3,
            "Generated invitation keys should be different each time"
        );
    }

    #[test]
    fn generate_invitation_key_includes_uppercase() {
        // 大文字が含まれている（複数回テストして確認）
        let keys: Vec<String> = (0..10).map(|_| generate_invitation_key()).collect();
        let has_uppercase = keys.iter().any(|k| k.chars().any(|c| c.is_uppercase()));
        assert!(
            has_uppercase,
            "Generated invitation keys should include uppercase letters"
        );
    }

    #[test]
    fn generate_invitation_key_includes_lowercase() {
        // 小文字が含まれている（複数回テストして確認）
        let keys: Vec<String> = (0..10).map(|_| generate_invitation_key()).collect();
        let has_lowercase = keys.iter().any(|k| k.chars().any(|c| c.is_lowercase()));
        assert!(
            has_lowercase,
            "Generated invitation keys should include lowercase letters"
        );
    }

    #[test]
    fn generate_invitation_key_includes_digits() {
        // 数字が含まれている（複数回テストして確認）
        let keys: Vec<String> = (0..10).map(|_| generate_invitation_key()).collect();
        let has_digits = keys.iter().any(|k| k.chars().any(|c| c.is_numeric()));
        assert!(
            has_digits,
            "Generated invitation keys should include digits"
        );
    }

    // --- T-0006: 招待キー検証関数のテスト実装（TDD RED フェーズ） ---
    // verify_invitation_key テストスイート（将来的な実装向け予約）
}
