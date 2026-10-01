//! 通知設定の読み込みと検証
//!
//! - 秘密情報（SMTP のユーザー名・パスワード）は環境変数から読む
//! - 非秘密設定（host / port / from / 有効化フラグ / 送信時刻 / 言語）は `settings` テーブルから読む
//!
//! 設定が未設定・不正でもエラーにはせず、通知を送れない理由を [`NotificationStatus`] として返す。

use super::template::Language;
use crate::common::error::RouterResult;
use crate::db::settings::SettingsStorage;
use crate::db::users::NotificationRecipient;
use chrono::NaiveTime;
use serde::{Deserialize, Serialize};

/// SMTP ユーザー名の環境変数
pub const ENV_SMTP_USERNAME: &str = "LLMLB_SMTP_USERNAME";
/// SMTP パスワードの環境変数
pub const ENV_SMTP_PASSWORD: &str = "LLMLB_SMTP_PASSWORD";

/// `settings` テーブル上の通知設定キーの接頭辞
pub const KEY_PREFIX: &str = "notifications.";
/// 通知の有効化フラグ（`true` / `false`）
pub const KEY_ENABLED: &str = "notifications.enabled";
/// SMTP ホスト
pub const KEY_SMTP_HOST: &str = "notifications.smtp_host";
/// SMTP ポート
pub const KEY_SMTP_PORT: &str = "notifications.smtp_port";
/// 差出人アドレス
pub const KEY_SMTP_FROM: &str = "notifications.smtp_from";
/// 日次ダイジェストの送信時刻（サーバーのローカル時刻、`HH:MM`）
pub const KEY_DAILY_DIGEST_TIME: &str = "notifications.daily_digest_time";
/// メール本文の言語（`ja` / `en`）
pub const KEY_LANGUAGE: &str = "notifications.language";
/// 日次ダイジェストの最終送信日（`YYYY-MM-DD`、スケジューラが更新する）
pub const KEY_DAILY_DIGEST_LAST_SENT_DATE: &str = "notifications.daily_digest_last_sent_date";

/// SMTP ポートの既定値（STARTTLS の submission ポート）
pub const DEFAULT_SMTP_PORT: u16 = 587;
/// 接続時から TLS を張る（implicit TLS）ポート。それ以外は STARTTLS を必須にする
pub const IMPLICIT_TLS_PORT: u16 = 465;
/// 日次ダイジェスト送信時刻の既定値
pub const DEFAULT_DAILY_DIGEST_TIME: &str = "09:00";

/// SMTP の認証情報（環境変数由来の秘密情報）
#[derive(Clone, Default, PartialEq, Eq)]
pub struct SmtpCredentials {
    username: Option<String>,
    password: Option<String>,
}

impl SmtpCredentials {
    /// 認証情報を作成する（空文字は未設定として扱う）
    pub fn new(username: Option<String>, password: Option<String>) -> Self {
        let _ = (username, password);
        todo!("SPEC #777 T-003")
    }

    /// プロセスの環境変数から読む
    pub fn from_env() -> Self {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// 任意の参照関数から読む（[`ENV_SMTP_USERNAME`] / [`ENV_SMTP_PASSWORD`]）
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
        Self::new(lookup(ENV_SMTP_USERNAME), lookup(ENV_SMTP_PASSWORD))
    }

    /// ユーザー名とパスワードの両方が設定されているか
    pub fn is_complete(&self) -> bool {
        self.username.is_some() && self.password.is_some()
    }
}

impl std::fmt::Debug for SmtpCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SmtpCredentials")
            .field("username", &self.username.as_ref().map(|_| "<set>"))
            .field("password", &self.password.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

/// 管理者が変更できる通知設定（非秘密）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationSettings {
    /// 通知を有効にするか
    pub enabled: bool,
    /// SMTP ホスト（空文字は未設定）
    pub smtp_host: String,
    /// SMTP ポート
    pub smtp_port: u16,
    /// 差出人アドレス（空文字は未設定）
    pub smtp_from: String,
    /// 日次ダイジェストの送信時刻（サーバーのローカル時刻、`HH:MM`）
    pub daily_digest_time: String,
    /// メール本文の言語
    pub language: Language,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            smtp_host: String::new(),
            smtp_port: DEFAULT_SMTP_PORT,
            smtp_from: String::new(),
            daily_digest_time: DEFAULT_DAILY_DIGEST_TIME.to_string(),
            language: Language::default(),
        }
    }
}

impl NotificationSettings {
    /// 入力値を検証し、前後の空白を除いた形で返す
    ///
    /// host / from の空文字は「未設定」として受け付ける（通知が送れない理由は
    /// [`NotificationStatus`] で示す）。形式が不正な値は理由付きで拒否する。
    pub fn validated(self) -> Result<Self, String> {
        todo!("SPEC #777 T-008")
    }
}

/// `settings` テーブルに保存されていた、解釈できない値
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidSetting {
    /// 設定キー
    pub key: &'static str,
    /// 解釈できない理由
    pub message: String,
}

/// `settings` テーブルから読み込んだ通知設定
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedSettings {
    /// 通知設定（解釈できない値は既定値に置き換わる）
    pub settings: NotificationSettings,
    /// 解釈できなかった値
    pub invalid: Vec<InvalidSetting>,
}

/// `settings` テーブルから通知設定を読む
pub async fn load_settings(storage: &SettingsStorage) -> RouterResult<LoadedSettings> {
    let _ = storage;
    todo!("SPEC #777 T-003")
}

/// 通知設定を `settings` テーブルへ保存する
pub async fn save_settings(
    storage: &SettingsStorage,
    settings: &NotificationSettings,
) -> RouterResult<()> {
    let _ = (storage, settings);
    todo!("SPEC #777 T-008")
}

/// 検証済みの SMTP 接続設定
#[derive(Clone, PartialEq, Eq)]
pub struct SmtpConfig {
    /// SMTP ホスト
    pub host: String,
    /// SMTP ポート
    pub port: u16,
    /// 差出人アドレス
    pub from: String,
    /// SMTP ユーザー名
    pub username: String,
    /// SMTP パスワード
    pub password: String,
}

impl std::fmt::Debug for SmtpConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SmtpConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("from", &self.from)
            .field("username", &"<set>")
            .field("password", &"<redacted>")
            .finish()
    }
}

/// SMTP 接続設定を組み立てる。送れない場合はその理由を返す
pub fn smtp_config(
    loaded: &LoadedSettings,
    credentials: &SmtpCredentials,
) -> Result<SmtpConfig, String> {
    let _ = (loaded, credentials);
    todo!("SPEC #777 T-003")
}

/// 通知機能の状態
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationState {
    /// 送信できる
    Active,
    /// 管理者が無効にしている
    Disabled,
    /// 有効だが、設定の未設定・不正により送信できない
    Unavailable,
}

/// 通知機能の状態と、送信できない場合の理由
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NotificationStatus {
    /// 状態
    pub state: NotificationState,
    /// 送信できない理由（`Active` のときは `None`）
    pub reason: Option<String>,
}

/// 通知の配送に必要な設定一式
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivery {
    /// SMTP 接続設定
    pub smtp: SmtpConfig,
    /// 日次ダイジェストの送信時刻（サーバーのローカル時刻）
    pub daily_digest_time: NaiveTime,
    /// メール本文の言語
    pub language: Language,
    /// 宛先アドレス
    pub recipients: Vec<String>,
}

/// 通知を配送できるか判定する。できない場合は理由付きの状態を返す
pub fn resolve_delivery(
    loaded: &LoadedSettings,
    credentials: &SmtpCredentials,
    recipients: &[NotificationRecipient],
) -> Result<Delivery, NotificationStatus> {
    let _ = (loaded, credentials, recipients);
    todo!("SPEC #777 T-009")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credentials() -> SmtpCredentials {
        SmtpCredentials::new(Some("mailer".to_string()), Some("s3cret".to_string()))
    }

    fn recipients() -> Vec<NotificationRecipient> {
        vec![NotificationRecipient {
            username: "admin".to_string(),
            email: "admin@example.com".to_string(),
        }]
    }

    fn complete_settings() -> NotificationSettings {
        NotificationSettings {
            enabled: true,
            smtp_host: "smtp.example.com".to_string(),
            smtp_port: 587,
            smtp_from: "llmlb@example.com".to_string(),
            daily_digest_time: "09:00".to_string(),
            language: Language::Ja,
        }
    }

    fn loaded(settings: NotificationSettings) -> LoadedSettings {
        LoadedSettings {
            settings,
            invalid: Vec::new(),
        }
    }

    fn unavailable_reason(result: Result<Delivery, NotificationStatus>) -> String {
        let status = result.expect_err("delivery must not be resolved");
        assert_eq!(status.state, NotificationState::Unavailable, "{status:?}");
        status.reason.expect("unavailable status carries a reason")
    }

    async fn storage() -> SettingsStorage {
        SettingsStorage::new(crate::db::test_utils::test_db_pool().await)
    }

    // --- SmtpCredentials ---

    #[test]
    fn credentials_treat_blank_values_as_unset() {
        assert!(credentials().is_complete());
        assert!(!SmtpCredentials::default().is_complete());
        assert!(!SmtpCredentials::new(Some("mailer".to_string()), None).is_complete());
        assert!(!SmtpCredentials::new(None, Some("s3cret".to_string())).is_complete());
        assert!(
            !SmtpCredentials::new(Some("  ".to_string()), Some("s3cret".to_string())).is_complete()
        );
        assert!(
            !SmtpCredentials::new(Some("mailer".to_string()), Some(String::new())).is_complete()
        );
    }

    #[test]
    fn credentials_are_read_from_the_smtp_environment_variables() {
        let from_lookup = SmtpCredentials::from_lookup(|key| match key {
            "LLMLB_SMTP_USERNAME" => Some("mailer".to_string()),
            "LLMLB_SMTP_PASSWORD" => Some("s3cret".to_string()),
            _ => None,
        });
        assert_eq!(from_lookup, credentials());
        assert_eq!(
            SmtpCredentials::from_lookup(|_| None),
            SmtpCredentials::default()
        );
    }

    #[test]
    fn debug_output_never_contains_secrets() {
        let debug = format!("{:?}", credentials());
        assert!(!debug.contains("s3cret"), "{debug}");
        assert!(!debug.contains("mailer"), "{debug}");

        let config = smtp_config(&loaded(complete_settings()), &credentials()).unwrap();
        let debug = format!("{config:?}");
        assert!(!debug.contains("s3cret"), "{debug}");
        assert!(!debug.contains("mailer"), "{debug}");
    }

    // --- NotificationSettings::validated ---

    #[test]
    fn validated_trims_and_accepts_unset_host_and_from() {
        let validated = NotificationSettings {
            smtp_host: "  smtp.example.com ".to_string(),
            smtp_from: " llmlb@example.com  ".to_string(),
            ..complete_settings()
        }
        .validated()
        .unwrap();
        assert_eq!(validated.smtp_host, "smtp.example.com");
        assert_eq!(validated.smtp_from, "llmlb@example.com");

        let unset = NotificationSettings {
            smtp_host: String::new(),
            smtp_from: "  ".to_string(),
            ..complete_settings()
        }
        .validated()
        .unwrap();
        assert_eq!(unset.smtp_host, "");
        assert_eq!(unset.smtp_from, "");
    }

    #[test]
    fn validated_rejects_malformed_values() {
        let cases = [
            (
                NotificationSettings {
                    smtp_from: "not-an-address".to_string(),
                    ..complete_settings()
                },
                "smtp_from",
            ),
            (
                NotificationSettings {
                    daily_digest_time: "25:00".to_string(),
                    ..complete_settings()
                },
                "daily_digest_time",
            ),
            (
                NotificationSettings {
                    smtp_port: 0,
                    ..complete_settings()
                },
                "smtp_port",
            ),
            (
                NotificationSettings {
                    smtp_host: "smtp.example.com:587".to_string(),
                    ..complete_settings()
                },
                "smtp_host",
            ),
            (
                NotificationSettings {
                    smtp_host: "smtp example.com".to_string(),
                    ..complete_settings()
                },
                "smtp_host",
            ),
        ];
        for (settings, field) in cases {
            let error = settings.clone().validated().unwrap_err();
            assert!(error.contains(field), "{settings:?} -> {error}");
        }
    }

    // --- load_settings / save_settings ---

    #[tokio::test]
    async fn load_returns_defaults_when_nothing_is_stored() {
        let loaded = load_settings(&storage().await).await.unwrap();
        assert_eq!(loaded.settings, NotificationSettings::default());
        assert!(!loaded.settings.enabled);
        assert_eq!(loaded.settings.smtp_port, 587);
        assert_eq!(loaded.settings.daily_digest_time, "09:00");
        assert_eq!(loaded.settings.language, Language::Ja);
        assert!(loaded.invalid.is_empty());
    }

    #[tokio::test]
    async fn save_then_load_roundtrips_through_the_settings_table() {
        let storage = storage().await;
        let settings = NotificationSettings {
            smtp_port: 465,
            daily_digest_time: "18:30".to_string(),
            language: Language::En,
            ..complete_settings()
        };
        save_settings(&storage, &settings).await.unwrap();

        let loaded = load_settings(&storage).await.unwrap();
        assert_eq!(loaded.settings, settings);
        assert!(loaded.invalid.is_empty());

        // 非秘密設定はキーごとに settings テーブルへ保存される
        assert_eq!(
            storage.get_setting(KEY_SMTP_HOST).await.unwrap().as_deref(),
            Some("smtp.example.com")
        );
        assert_eq!(
            storage.get_setting(KEY_SMTP_PORT).await.unwrap().as_deref(),
            Some("465")
        );
        assert_eq!(
            storage.get_setting(KEY_ENABLED).await.unwrap().as_deref(),
            Some("true")
        );
        assert_eq!(
            storage.get_setting(KEY_LANGUAGE).await.unwrap().as_deref(),
            Some("en")
        );
    }

    #[tokio::test]
    async fn load_reports_values_it_cannot_interpret() {
        let storage = storage().await;
        save_settings(&storage, &complete_settings()).await.unwrap();
        for (key, value) in [
            (KEY_ENABLED, "yes"),
            (KEY_SMTP_PORT, "abc"),
            (KEY_DAILY_DIGEST_TIME, "25:00"),
            (KEY_LANGUAGE, "fr"),
        ] {
            storage.set_setting(key, value).await.unwrap();
        }

        let loaded = load_settings(&storage).await.unwrap();
        let invalid_keys: Vec<&str> = loaded.invalid.iter().map(|invalid| invalid.key).collect();
        assert_eq!(
            invalid_keys,
            vec![
                KEY_ENABLED,
                KEY_SMTP_PORT,
                KEY_DAILY_DIGEST_TIME,
                KEY_LANGUAGE
            ]
        );
        // 解釈できない値は安全側の既定値に置き換わる
        assert!(!loaded.settings.enabled);
        assert_eq!(loaded.settings.smtp_port, DEFAULT_SMTP_PORT);
        assert_eq!(loaded.settings.daily_digest_time, DEFAULT_DAILY_DIGEST_TIME);
        assert_eq!(loaded.settings.language, Language::Ja);
        assert_eq!(loaded.settings.smtp_host, "smtp.example.com");
    }

    // --- smtp_config / resolve_delivery ---

    #[test]
    fn smtp_config_combines_settings_table_values_with_environment_secrets() {
        let config = smtp_config(&loaded(complete_settings()), &credentials()).unwrap();
        assert_eq!(config.host, "smtp.example.com");
        assert_eq!(config.port, 587);
        assert_eq!(config.from, "llmlb@example.com");
        assert_eq!(config.username, "mailer");
        assert_eq!(config.password, "s3cret");
    }

    #[test]
    fn resolves_delivery_when_everything_is_configured() {
        let delivery =
            resolve_delivery(&loaded(complete_settings()), &credentials(), &recipients()).unwrap();
        assert_eq!(delivery.smtp.host, "smtp.example.com");
        assert_eq!(
            delivery.daily_digest_time,
            NaiveTime::from_hms_opt(9, 0, 0).unwrap()
        );
        assert_eq!(delivery.language, Language::Ja);
        assert_eq!(delivery.recipients, vec!["admin@example.com".to_string()]);
    }

    #[test]
    fn turned_off_notifications_are_disabled_not_unavailable() {
        let status = resolve_delivery(
            &loaded(NotificationSettings::default()),
            &SmtpCredentials::default(),
            &[],
        )
        .unwrap_err();
        assert_eq!(status.state, NotificationState::Disabled);
        assert!(status.reason.is_some());

        // 無効化されていれば、他の値が不正でも「無効」として扱う
        let off_with_garbage = LoadedSettings {
            settings: NotificationSettings::default(),
            invalid: vec![InvalidSetting {
                key: KEY_SMTP_PORT,
                message: "'abc' is not a valid port".to_string(),
            }],
        };
        let status =
            resolve_delivery(&off_with_garbage, &credentials(), &recipients()).unwrap_err();
        assert_eq!(status.state, NotificationState::Disabled);
    }

    #[test]
    fn missing_smtp_host_makes_notifications_unavailable() {
        let settings = NotificationSettings {
            smtp_host: String::new(),
            ..complete_settings()
        };
        let reason = unavailable_reason(resolve_delivery(
            &loaded(settings),
            &credentials(),
            &recipients(),
        ));
        assert!(reason.contains("SMTP host"), "{reason}");
    }

    #[test]
    fn missing_or_invalid_from_address_makes_notifications_unavailable() {
        for from in ["", "not-an-address"] {
            let settings = NotificationSettings {
                smtp_from: from.to_string(),
                ..complete_settings()
            };
            let reason = unavailable_reason(resolve_delivery(
                &loaded(settings),
                &credentials(),
                &recipients(),
            ));
            assert!(reason.contains("from address"), "{from:?} -> {reason}");
        }
    }

    #[test]
    fn missing_credentials_make_notifications_unavailable() {
        for incomplete in [
            SmtpCredentials::default(),
            SmtpCredentials::new(Some("mailer".to_string()), None),
            SmtpCredentials::new(None, Some("s3cret".to_string())),
        ] {
            let reason = unavailable_reason(resolve_delivery(
                &loaded(complete_settings()),
                &incomplete,
                &recipients(),
            ));
            assert!(reason.contains(ENV_SMTP_USERNAME), "{reason}");
            assert!(reason.contains(ENV_SMTP_PASSWORD), "{reason}");
            assert!(!reason.contains("s3cret"), "{reason}");
        }
    }

    #[test]
    fn uninterpretable_stored_values_make_notifications_unavailable() {
        let broken = LoadedSettings {
            settings: complete_settings(),
            invalid: vec![InvalidSetting {
                key: KEY_SMTP_PORT,
                message: "'abc' is not a valid port".to_string(),
            }],
        };
        let reason = unavailable_reason(resolve_delivery(&broken, &credentials(), &recipients()));
        assert!(reason.contains(KEY_SMTP_PORT), "{reason}");

        // 有効化フラグ自体が解釈できない場合も、黙って無効扱いにせず理由を出す
        let unknown_switch = LoadedSettings {
            settings: NotificationSettings::default(),
            invalid: vec![InvalidSetting {
                key: KEY_ENABLED,
                message: "'yes' is not true or false".to_string(),
            }],
        };
        let reason = unavailable_reason(resolve_delivery(
            &unknown_switch,
            &credentials(),
            &recipients(),
        ));
        assert!(reason.contains(KEY_ENABLED), "{reason}");
    }

    #[test]
    fn no_recipient_makes_notifications_unavailable() {
        let reason = unavailable_reason(resolve_delivery(
            &loaded(complete_settings()),
            &credentials(),
            &[],
        ));
        assert!(reason.contains("email"), "{reason}");
    }
}
