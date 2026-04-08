//! BIP39バックアップコード生成・検証
//!
//! T-0014, T-0015, T-0016, T-0017の実装

use crate::common::error::LbError;
use bip39::{Language, Mnemonic};
use rand::RngExt;

/// BIP39バックアップコードを3セット生成する
///
/// # Returns
/// - `Vec<String>` - 3つのバックアップコード（各12語のニーモニック文字列）
pub fn generate_backup_codes() -> Vec<String> {
    vec![
        generate_single_code(),
        generate_single_code(),
        generate_single_code(),
    ]
}

/// 単一のバックアップコード（12語）を生成する
fn generate_single_code() -> String {
    // 12語 = 128ビットのエントロピー
    let mut entropy = [0u8; 16];
    let mut rng = rand::rng();
    rng.fill(&mut entropy);

    Mnemonic::from_entropy(&entropy)
        .expect("Failed to generate mnemonic from entropy")
        .to_string()
}

/// BIP39バックアップコードを検証する
///
/// # Arguments
/// * `code` - 検証するバックアップコード（12語の文字列）
///
/// # Returns
/// * `Result<bool, LbError>` - 有効な場合 `Ok(true)`、無効な場合 `Ok(false)`、エラーの場合 `Err(LbError)`
pub fn verify_backup_code(code: &str) -> Result<bool, LbError> {
    match Mnemonic::parse_in_normalized(Language::English, code) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_backup_codes() {
        let codes = generate_backup_codes();
        assert_eq!(codes.len(), 3, "Should generate exactly 3 codes");

        for (idx, code) in codes.iter().enumerate() {
            let words: Vec<&str> = code.split_whitespace().collect();
            assert_eq!(words.len(), 12, "Code {} should have exactly 12 words", idx);
        }
    }

    #[test]
    fn test_generate_backup_codes_uniqueness() {
        let codes1 = generate_backup_codes();
        let codes2 = generate_backup_codes();

        // 生成されたコードはユニーク（ほぼ確実）
        assert_ne!(
            codes1, codes2,
            "Two generations should produce different codes"
        );
    }

    #[test]
    fn test_verify_backup_code_valid() {
        // 有効なBIP39ニーモニック
        let valid_code = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let result = verify_backup_code(valid_code);
        assert!(result.is_ok(), "Valid code should not error");
        assert_eq!(result.unwrap(), true, "Valid code should return true");
    }

    #[test]
    fn test_verify_backup_code_invalid() {
        // 無効なニーモニック
        let invalid_code = "invalid invalid invalid invalid invalid invalid invalid invalid invalid invalid invalid invalid";
        let result = verify_backup_code(invalid_code);
        assert!(result.is_ok(), "Invalid code check should not error");
        assert_eq!(result.unwrap(), false, "Invalid code should return false");
    }

    #[test]
    fn test_verify_backup_code_wrong_word_count() {
        // 単語数が異なる
        let wrong_count = "abandon abandon abandon abandon abandon abandon";
        let result = verify_backup_code(wrong_count);
        assert!(result.is_ok(), "Wrong word count check should not error");
        assert_eq!(
            result.unwrap(),
            false,
            "Wrong word count should return false"
        );
    }

    #[test]
    fn test_verify_generated_code() {
        let codes = generate_backup_codes();
        for code in codes {
            let result = verify_backup_code(&code);
            assert!(result.is_ok(), "Generated code should be valid");
            assert_eq!(
                result.unwrap(),
                true,
                "Generated code should verify as valid"
            );
        }
    }
}
