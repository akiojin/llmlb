//! メール本文テンプレート（日本語・英語）

use super::digest::DigestReport;
use super::offline_alert::OfflineAlert;
use crate::types::endpoint::EndpointStatus;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::LazyLock;

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
                without_url_credentials(&one_line(&endpoint.base_url))
            ));
            let last_seen = match endpoint.last_seen {
                Some(seen) => seen.format("%Y-%m-%d %H:%M UTC").to_string(),
                None => text.never.to_string(),
            };
            lines.push(format!("    {}{last_seen}", text.last_seen));
            if let Some(last_error) = &endpoint.last_error {
                lines.push(format!(
                    "    {}{}",
                    text.last_error,
                    without_url_credentials(&one_line(last_error))
                ));
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

/// 即時通知（エンドポイントの Offline 到達）を指定言語で展開する
pub fn render_offline_alert(_language: Language, _alert: &OfflineAlert) -> RenderedMail {
    RenderedMail {
        subject: String::new(),
        body: String::new(),
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

/// URL の `://` から、authority 内の最後の `@` まで（`user:password@`）
static URL_CREDENTIALS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"://[^/?#\s]*@").expect("valid regex"));

/// 文中の URL から、埋め込まれた認証情報（`scheme://user:password@host` の `user:password@`）を除く
fn without_url_credentials(value: &str) -> String {
    URL_CREDENTIALS.replace_all(value, "://").into_owned()
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

    /// URL に埋め込まれた認証情報はメールに載せない（メールは外部のメールサーバーを経由する）
    #[test]
    fn url_credentials_are_not_included_in_the_mail() {
        let mut report = report();
        report.endpoints[0].base_url = "https://ops:s3cr@t@gpu-b.example.com:8443/v1".to_string();
        report.endpoints[0].last_error = Some(
            "error sending request for url \
             (https://ops:s3cr@t@gpu-b.example.com:8443/v1/models): connection refused"
                .to_string(),
        );

        let mail = render_daily_digest(Language::En, &report);
        assert!(!mail.body.contains("s3cr"), "{}", mail.body);
        assert!(!mail.body.contains("ops:"), "{}", mail.body);
        assert!(
            mail.body
                .contains("- [Offline] gpu-b (ollama) https://gpu-b.example.com:8443/v1\n"),
            "{}",
            mail.body
        );
        assert!(
            mail.body.contains(
                "    Last error: error sending request for url \
                 (https://gpu-b.example.com:8443/v1/models): connection refused\n"
            ),
            "{}",
            mail.body
        );
    }

    #[test]
    fn only_the_credentials_of_a_url_are_removed() {
        // 認証情報を含まない URL、パス中の `@`、URL ではない `@` はそのまま残す
        for value in [
            "http://10.0.0.2:11434",
            "http://[::1]:8080/v1",
            "https://gpu.example.com/users/@me?contact=ops@example.com",
            "rejected by ops@example.com: quota exceeded",
        ] {
            assert_eq!(without_url_credentials(value), value);
        }
        assert_eq!(
            without_url_credentials("http://token@a.example.com then http://u:p@b.example.com/x"),
            "http://a.example.com then http://b.example.com/x"
        );
    }

    // --- 即時通知（Offline 到達）---

    fn alert() -> OfflineAlert {
        OfflineAlert {
            detected_at: NaiveDateTime::parse_from_str("2026-10-01 03:14:00", "%Y-%m-%d %H:%M:%S")
                .unwrap(),
            previous_status: EndpointStatus::Error,
            endpoint: report().endpoints.remove(0),
        }
    }

    #[test]
    fn renders_the_offline_alert_in_japanese() {
        let mail = render_offline_alert(Language::Ja, &alert());
        assert_eq!(
            mail.subject,
            "[llmlb] エンドポイントが Offline になりました: gpu-b"
        );
        assert_eq!(
            mail.body,
            "llmlb 障害通知\n\
             検知時刻: 2026-10-01 03:14（サーバーのローカル時刻）\n\
             \n\
             エンドポイントが Offline になりました。\n\
             - gpu-b (ollama) http://10.0.0.2:11434\n\
             \x20\x20\x20\x20直前の状態: Error\n\
             \x20\x20\x20\x20最終確認: 2026-09-30 22:10 UTC\n\
             \x20\x20\x20\x20最後のエラー: connection refused\n\
             \n\
             このメールは llmlb の運用通知です。通知の有効／無効と宛先は llmlb の通知設定で変更できます。\n"
        );
    }

    #[test]
    fn renders_the_offline_alert_in_english() {
        let mail = render_offline_alert(Language::En, &alert());
        assert_eq!(mail.subject, "[llmlb] Endpoint went offline: gpu-b");
        assert_eq!(
            mail.body,
            "llmlb outage alert\n\
             Detected at: 2026-10-01 03:14 (server local time)\n\
             \n\
             An endpoint went offline.\n\
             - gpu-b (ollama) http://10.0.0.2:11434\n\
             \x20\x20\x20\x20Previous status: Error\n\
             \x20\x20\x20\x20Last seen: 2026-09-30 22:10 UTC\n\
             \x20\x20\x20\x20Last error: connection refused\n\
             \n\
             This is an operational notification from llmlb. Notifications and their recipients can be changed in the llmlb notification settings.\n"
        );
    }

    #[test]
    fn offline_alert_omits_details_that_are_unknown() {
        let mut alert = alert();
        alert.previous_status = EndpointStatus::Pending;
        alert.endpoint.last_seen = None;
        alert.endpoint.last_error = None;

        let mail = render_offline_alert(Language::En, &alert);
        assert!(
            mail.body.contains(
                "    Previous status: Pending\n    Last seen: never\n\nThis is an operational"
            ),
            "{}",
            mail.body
        );
    }

    /// 件名に改行が入るとメールヘッダが壊れるので、名前は 1 行に畳む。認証情報も載せない
    #[test]
    fn offline_alert_flattens_names_and_removes_url_credentials() {
        let mut alert = alert();
        alert.endpoint.name = "gpu\r\nb".to_string();
        alert.endpoint.base_url = "https://ops:s3cr@t@gpu-b.example.com:8443/v1".to_string();
        alert.endpoint.last_error = Some(
            "error sending request for url\n(https://ops:s3cr@t@gpu-b.example.com:8443/v1/models)"
                .to_string(),
        );

        let mail = render_offline_alert(Language::En, &alert);
        assert_eq!(mail.subject, "[llmlb] Endpoint went offline: gpu b");
        assert!(!mail.body.contains("s3cr"), "{}", mail.body);
        assert!(
            mail.body
                .contains("- gpu b (ollama) https://gpu-b.example.com:8443/v1\n"),
            "{}",
            mail.body
        );
        assert!(
            mail.body.contains(
                "    Last error: error sending request for url \
                 (https://gpu-b.example.com:8443/v1/models)\n"
            ),
            "{}",
            mail.body
        );
    }
}
