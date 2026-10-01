//! メール本文テンプレート（日本語・英語）

use super::digest::DigestReport;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// メール本文の言語
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// 日本語
    #[default]
    Ja,
    /// 英語
    En,
}

impl Language {
    /// `settings` テーブルに保存する値
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ja => "ja",
            Self::En => "en",
        }
    }
}

impl FromStr for Language {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "ja" => Ok(Self::Ja),
            "en" => Ok(Self::En),
            other => Err(format!(
                "unsupported language '{other}' (expected ja or en)"
            )),
        }
    }
}

/// 件名と本文に展開済みのメール
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedMail {
    /// 件名
    pub subject: String,
    /// 本文（プレーンテキスト）
    pub body: String,
}

/// 日次ダイジェストを指定言語で展開する
pub fn render_daily_digest(language: Language, report: &DigestReport) -> RenderedMail {
    let _ = (language, report);
    todo!("SPEC #777 T-007")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::digest::EndpointDigestEntry;
    use crate::types::endpoint::EndpointStatus;
    use chrono::{NaiveDateTime, TimeZone, Utc};

    fn report() -> DigestReport {
        DigestReport {
            generated_at: NaiveDateTime::parse_from_str("2026-10-01 09:00:00", "%Y-%m-%d %H:%M:%S")
                .unwrap(),
            endpoints: vec![
                EndpointDigestEntry {
                    name: "gpu-b".to_string(),
                    base_url: "http://10.0.0.2:11434".to_string(),
                    endpoint_type: "ollama",
                    status: EndpointStatus::Offline,
                    last_seen: Some(Utc.with_ymd_and_hms(2026, 9, 30, 22, 10, 0).unwrap()),
                    last_error: Some("connection refused".to_string()),
                },
                EndpointDigestEntry {
                    name: "gpu-a".to_string(),
                    base_url: "http://10.0.0.1:8080".to_string(),
                    endpoint_type: "xllm",
                    status: EndpointStatus::Online,
                    last_seen: None,
                    last_error: None,
                },
            ],
        }
    }

    fn empty_report() -> DigestReport {
        DigestReport {
            endpoints: Vec::new(),
            ..report()
        }
    }

    #[test]
    fn language_roundtrips_through_its_stored_value() {
        for language in [Language::Ja, Language::En] {
            assert_eq!(language.as_str().parse::<Language>(), Ok(language));
        }
        assert!("fr".parse::<Language>().is_err());
        assert!("JA".parse::<Language>().is_err());
        assert_eq!(serde_json::to_string(&Language::En).unwrap(), "\"en\"");
        assert_eq!(
            serde_json::from_str::<Language>("\"ja\"").unwrap(),
            Language::Ja
        );
    }

    #[test]
    fn renders_the_daily_digest_in_japanese() {
        let mail = render_daily_digest(Language::Ja, &report());
        assert_eq!(
            mail.subject,
            "[llmlb] 日次ダイジェスト 2026-10-01（Online 1 / Offline 1 / Error 0 / Pending 0）"
        );
        assert_eq!(
            mail.body,
            "llmlb 日次ダイジェスト\n\
             集計時刻: 2026-10-01 09:00（サーバーのローカル時刻）\n\
             \n\
             登録エンドポイント: 2 件\n\
             \x20\x20Online: 1 / Offline: 1 / Error: 0 / Pending: 0\n\
             \n\
             状態一覧:\n\
             - [Offline] gpu-b (ollama) http://10.0.0.2:11434\n\
             \x20\x20\x20\x20最終確認: 2026-09-30 22:10 UTC\n\
             \x20\x20\x20\x20最後のエラー: connection refused\n\
             - [Online] gpu-a (xllm) http://10.0.0.1:8080\n\
             \x20\x20\x20\x20最終確認: なし\n\
             \n\
             このメールは llmlb の運用通知です。送信時刻と宛先は llmlb の通知設定で変更できます。\n"
        );
    }

    #[test]
    fn renders_the_daily_digest_in_english() {
        let mail = render_daily_digest(Language::En, &report());
        assert_eq!(
            mail.subject,
            "[llmlb] Daily digest 2026-10-01 (Online 1 / Offline 1 / Error 0 / Pending 0)"
        );
        assert_eq!(
            mail.body,
            "llmlb daily digest\n\
             Generated at: 2026-10-01 09:00 (server local time)\n\
             \n\
             Registered endpoints: 2\n\
             \x20\x20Online: 1 / Offline: 1 / Error: 0 / Pending: 0\n\
             \n\
             Status:\n\
             - [Offline] gpu-b (ollama) http://10.0.0.2:11434\n\
             \x20\x20\x20\x20Last seen: 2026-09-30 22:10 UTC\n\
             \x20\x20\x20\x20Last error: connection refused\n\
             - [Online] gpu-a (xllm) http://10.0.0.1:8080\n\
             \x20\x20\x20\x20Last seen: never\n\
             \n\
             This is an operational notification from llmlb. The send time and recipients can be changed in the llmlb notification settings.\n"
        );
    }

    #[test]
    fn states_that_no_endpoint_is_registered() {
        let ja = render_daily_digest(Language::Ja, &empty_report());
        assert!(ja
            .subject
            .contains("Online 0 / Offline 0 / Error 0 / Pending 0"));
        assert!(ja.body.contains("登録エンドポイント: 0 件"), "{}", ja.body);
        assert!(
            ja.body
                .contains("登録されているエンドポイントはありません。"),
            "{}",
            ja.body
        );

        let en = render_daily_digest(Language::En, &empty_report());
        assert!(en.body.contains("Registered endpoints: 0"), "{}", en.body);
        assert!(
            en.body.contains("No endpoints are registered."),
            "{}",
            en.body
        );
    }

    /// 件名ヘッダーを壊さないよう、改行を含む値は 1 行に畳む
    #[test]
    fn multiline_values_are_flattened_into_one_line() {
        let mut report = report();
        report.endpoints[0].name = "gpu\r\nb".to_string();
        report.endpoints[0].last_error = Some("line one\nline two".to_string());

        let mail = render_daily_digest(Language::En, &report);
        assert!(
            mail.body.contains("- [Offline] gpu b (ollama)"),
            "{}",
            mail.body
        );
        assert!(
            mail.body.contains("    Last error: line one line two\n"),
            "{}",
            mail.body
        );
    }
}
