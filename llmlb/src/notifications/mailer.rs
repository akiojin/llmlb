//! メール送信の抽象
//!
//! 通知ロジックは [`MailTransport`] だけに依存する。本番は SMTP 実装
//! （[`super::SmtpMailTransport`]）を使い、テストは記録用実装に差し替えて
//! 実際の送信を行わずに宛先と本文を検証する。

use async_trait::async_trait;

/// 送信するメール
///
/// 差出人はトランスポート側の設定で決まるため、ここには含めない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailMessage {
    /// 宛先アドレス
    pub to: Vec<String>,
    /// 件名
    pub subject: String,
    /// 本文（プレーンテキスト）
    pub body: String,
}

/// メール送信エラー
#[derive(Debug, thiserror::Error)]
pub enum MailError {
    /// メールアドレスとして解釈できない
    #[error("invalid mail address: {0}")]
    InvalidAddress(String),
    /// トランスポートの設定が未設定・不正
    #[error("mail transport is not configured: {0}")]
    NotConfigured(String),
    /// メッセージを組み立てられない
    #[error("failed to build mail message: {0}")]
    Message(String),
    /// 配送に失敗した
    #[error("mail delivery failed: {0}")]
    Delivery(String),
}

/// メール送信トランスポート
#[async_trait]
pub trait MailTransport: Send + Sync {
    /// メールを 1 通送信する
    async fn send(&self, message: &MailMessage) -> Result<(), MailError>;
}

/// メールアドレスの長さの上限（RFC 5321）
const MAX_ADDRESS_LENGTH: usize = 254;

/// メールアドレスを検証し、前後の空白を除いた形で返す
pub fn parse_address(input: &str) -> Result<String, MailError> {
    let trimmed = input.trim();
    if trimmed.len() > MAX_ADDRESS_LENGTH {
        return Err(MailError::InvalidAddress(format!(
            "address is longer than {MAX_ADDRESS_LENGTH} characters"
        )));
    }
    trimmed
        .parse::<lettre::Address>()
        .map(|address| address.to_string())
        .map_err(|_| MailError::InvalidAddress(trimmed.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_address_accepts_and_trims_valid_addresses() {
        assert_eq!(parse_address("ops@example.com").unwrap(), "ops@example.com");
        assert_eq!(
            parse_address("  first.last+tag@sub.example.co.jp ").unwrap(),
            "first.last+tag@sub.example.co.jp"
        );
    }

    #[test]
    fn parse_address_rejects_overlong_addresses() {
        let overlong = format!("{}@example.com", "a".repeat(MAX_ADDRESS_LENGTH));
        let error = parse_address(&overlong).unwrap_err();
        assert!(matches!(error, MailError::InvalidAddress(_)), "{error:?}");
        assert!(!error.to_string().contains(&overlong), "{error}");
    }

    #[test]
    fn parse_address_rejects_malformed_addresses() {
        for invalid in [
            "",
            "   ",
            "not-an-address",
            "a@",
            "@example.com",
            "a b@example.com",
            "a@b@example.com",
            "Ops <ops@example.com>",
            "ops@example.com, other@example.com",
        ] {
            let error = parse_address(invalid).unwrap_err();
            assert!(
                matches!(error, MailError::InvalidAddress(_)),
                "{invalid:?} -> {error:?}"
            );
        }
    }
}
