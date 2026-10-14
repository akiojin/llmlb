//! メールID形式の検証（SPEC #580 T001）
//!
//! ユーザー名はメールアドレスを識別子として扱う。配送可否までは確認せず、
//! `local@domain.tld` の構造だけを検証する。

use crate::common::error::{CommonError, LbError};

/// RFC 5321 のアドレス長上限
const MAX_EMAIL_LENGTH: usize = 254;

/// メールID形式を検証する
pub fn validate_email(email: &str) -> Result<(), LbError> {
    if is_valid_email(email) {
        Ok(())
    } else {
        Err(LbError::Common(CommonError::Validation(
            "ユーザー名はメールアドレス形式で指定してください".to_string(),
        )))
    }
}

fn is_valid_email(email: &str) -> bool {
    if email.len() > MAX_EMAIL_LENGTH || email.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return false;
    }
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    if local.is_empty() || domain.contains('@') {
        return false;
    }
    let labels: Vec<&str> = domain.split('.').collect();
    labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_alphanumeric() || c == '-')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_typical_addresses() {
        for email in [
            "user@example.com",
            "first.last+tag@sub.example.co.jp",
            "a@b.io",
        ] {
            assert!(validate_email(email).is_ok(), "{email} should be valid");
        }
    }

    #[test]
    fn rejects_malformed_addresses() {
        for email in [
            "",
            "plainname",
            "no-at.example.com",
            "@example.com",
            "user@",
            "user@localhost",
            "user@@example.com",
            "user@exa mple.com",
            "user name@example.com",
            " user@example.com",
            "user@example..com",
            "user@-example.com",
            "user@example.com.",
        ] {
            assert!(
                validate_email(email).is_err(),
                "{email:?} should be invalid"
            );
        }
    }

    #[test]
    fn rejects_overlong_addresses() {
        let email = format!("{}@example.com", "a".repeat(MAX_EMAIL_LENGTH));
        assert!(validate_email(&email).is_err());
    }
}
