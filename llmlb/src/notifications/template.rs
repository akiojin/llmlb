//! メール本文テンプレート（日本語・英語）

use super::digest::DigestReport;
use crate::types::endpoint::EndpointStatus;
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
    let text = DigestText::of(language);
    let date = report.generated_at.format("%Y-%m-%d");
    let [online, offline, error, pending] = [
        EndpointStatus::Online,
        EndpointStatus::Offline,
        EndpointStatus::Error,
        EndpointStatus::Pending,
    ]
    .map(|status| report.count(status));

    let counts = format!("Online {online} / Offline {offline} / Error {error} / Pending {pending}");
    let subject = match language {
        Language::Ja => format!("[llmlb] {} {date}（{counts}）", text.subject),
        Language::En => format!("[llmlb] {} {date} ({counts})", text.subject),
    };

    let mut lines = vec![
        text.title.to_string(),
        format!(
            "{}{}{}",
            text.generated_at,
            report.generated_at.format("%Y-%m-%d %H:%M"),
            text.local_time
        ),
        String::new(),
        format!(
            "{}{}{}",
            text.registered,
            report.endpoints.len(),
            text.registered_unit
        ),
        format!("  Online: {online} / Offline: {offline} / Error: {error} / Pending: {pending}"),
        String::new(),
    ];
    if report.endpoints.is_empty() {
        lines.push(text.no_endpoints.to_string());
    } else {
        lines.push(text.status_list.to_string());
        for endpoint in &report.endpoints {
            lines.push(format!(
                "- [{}] {} ({}) {}",
                status_label(endpoint.status),
                one_line(&endpoint.name),
                endpoint.endpoint_type,
                one_line(&endpoint.base_url)
            ));
            let last_seen = match endpoint.last_seen {
                Some(seen) => seen.format("%Y-%m-%d %H:%M UTC").to_string(),
                None => text.never.to_string(),
            };
            lines.push(format!("    {}{last_seen}", text.last_seen));
            if let Some(last_error) = &endpoint.last_error {
                lines.push(format!("    {}{}", text.last_error, one_line(last_error)));
            }
        }
    }
    lines.push(String::new());
    lines.push(text.footer.to_string());

    RenderedMail {
        subject,
        body: lines.join("\n") + "\n",
    }
}

/// 日次ダイジェストの言語別の文言
struct DigestText {
    subject: &'static str,
    title: &'static str,
    generated_at: &'static str,
    local_time: &'static str,
    registered: &'static str,
    registered_unit: &'static str,
    status_list: &'static str,
    no_endpoints: &'static str,
    last_seen: &'static str,
    never: &'static str,
    last_error: &'static str,
    footer: &'static str,
}

impl DigestText {
    fn of(language: Language) -> Self {
        match language {
            Language::Ja => Self {
                subject: "日次ダイジェスト",
                title: "llmlb 日次ダイジェスト",
                generated_at: "集計時刻: ",
                local_time: "（サーバーのローカル時刻）",
                registered: "登録エンドポイント: ",
                registered_unit: " 件",
                status_list: "状態一覧:",
                no_endpoints: "登録されているエンドポイントはありません。",
                last_seen: "最終確認: ",
                never: "なし",
                last_error: "最後のエラー: ",
                footer: "このメールは llmlb の運用通知です。送信時刻と宛先は llmlb の通知設定で変更できます。",
            },
            Language::En => Self {
                subject: "Daily digest",
                title: "llmlb daily digest",
                generated_at: "Generated at: ",
                local_time: " (server local time)",
                registered: "Registered endpoints: ",
                registered_unit: "",
                status_list: "Status:",
                no_endpoints: "No endpoints are registered.",
                last_seen: "Last seen: ",
                never: "never",
                last_error: "Last error: ",
                footer: "This is an operational notification from llmlb. The send time and recipients can be changed in the llmlb notification settings.",
            },
        }
    }
}

/// 状態の表示名（ダッシュボードの表記に合わせ、言語によらず同じ）
fn status_label(status: EndpointStatus) -> &'static str {
    match status {
        EndpointStatus::Online => "Online",
        EndpointStatus::Offline => "Offline",
        EndpointStatus::Error => "Error",
        EndpointStatus::Pending => "Pending",
    }
}

/// 改行や連続する空白を 1 つの空白に畳む
fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::digest::EndpointDigestEntry;
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

    /// エンドポイント名やエラー文に改行が含まれていても、一覧の体裁を崩さない
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
