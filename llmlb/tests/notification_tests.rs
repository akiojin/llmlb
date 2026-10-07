//! SPEC #777 運用通知の受け入れテスト entrypoint
//!
//! `make notification-tests`（CI の必須チェックと `make quality-checks` の両方）が
//! このバイナリを実行する。テスト名は対応する受け入れ基準（`ac<N>_`）を接頭辞に持つ。

#[path = "support/mod.rs"]
pub mod support;

#[path = "notifications/helpers.rs"]
mod helpers;

#[path = "notifications/users_email_test.rs"]
mod users_email_test;

#[path = "notifications/delivery_test.rs"]
mod delivery_test;

#[path = "notifications/settings_api_test.rs"]
mod settings_api_test;

#[path = "notifications/offline_alert_test.rs"]
mod offline_alert_test;

#[path = "notifications/startup_alert_test.rs"]
mod startup_alert_test;
