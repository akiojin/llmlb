//! AC-1 / AC-4 / AC-6 / AC-7: 送信基盤・日次ダイジェスト・テンプレート言語・未設定時の挙動
//!
//! すべて記録用トランスポートを使い、実際の送信は行わない。
//! プロセスの再起動は、同じ DB に対して新しいスケジューラを作ることで再現する。

use crate::helpers::{
    at, capture_logs, complete_settings, create_user_with_email, ready_pool, register_endpoint,
    save_notification_settings, scheduler, send_json, RecordingTransport,
};
use axum::http::{Method, StatusCode};
use llmlb::common::auth::UserRole;
use llmlb::db::settings::SettingsStorage;
use llmlb::notifications::{
    load_settings, smtp_config, DigestOutcome, Language, NotificationSettings, NotificationState,
    SmtpCredentials,
};
use llmlb::types::endpoint::EndpointStatus;
use std::time::Duration;

const LAST_SENT_KEY: &str = "notifications.daily_digest_last_sent_date";
const LAST_ERROR_KEY: &str = "notifications.daily_digest_last_error";

fn sent_to(recipients: &[&str]) -> DigestOutcome {
    DigestOutcome::Sent {
        recipients: recipients
            .iter()
            .map(|address| address.to_string())
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// AC-1: 送信基盤
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ac1_mail_goes_through_a_swappable_transport_without_real_delivery() {
    let pool = ready_pool().await;
    let transport = RecordingTransport::default();

    let tick = scheduler(&pool, &transport)
        .tick(at("2026-10-01 09:00:00"))
        .await;

    assert_eq!(tick.outcome, sent_to(&["ops@example.com"]));
    let sent = transport.sent();
    assert_eq!(sent.len(), 1, "exactly one mail is handed to the transport");
    assert_eq!(sent[0].to, vec!["ops@example.com".to_string()]);
    assert!(
        sent[0].subject.contains("2026-10-01"),
        "{}",
        sent[0].subject
    );
    assert!(sent[0].body.contains("gpu-a"), "{}", sent[0].body);
}

#[tokio::test]
async fn ac1_smtp_secrets_come_from_env_and_other_settings_from_the_settings_table() {
    let pool = crate::support::lb::create_test_db_pool().await;
    save_notification_settings(&pool, &complete_settings()).await;
    let loaded = load_settings(&SettingsStorage::new(pool.clone()))
        .await
        .unwrap();

    let from_env = SmtpCredentials::from_lookup(|key| match key {
        "LLMLB_SMTP_USERNAME" => Some("mailer".to_string()),
        "LLMLB_SMTP_PASSWORD" => Some("s3cret".to_string()),
        _ => None,
    });
    let config = smtp_config(&loaded, &from_env).expect("complete SMTP config");
    assert_eq!(config.host, "smtp.example.com");
    assert_eq!(config.port, 587);
    assert_eq!(config.from, "llmlb@example.com");
    assert_eq!(config.username, "mailer");
    assert_eq!(config.password, "s3cret");

    // 秘密情報は settings テーブルに保存されない
    let stored: Vec<(String, String)> =
        sqlx::query_as("SELECT key, value FROM settings WHERE key LIKE 'notifications.%'")
            .fetch_all(&pool)
            .await
            .unwrap();
    let mut keys: Vec<&str> = stored.iter().map(|(key, _)| key.as_str()).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec![
            "notifications.daily_digest_time",
            "notifications.enabled",
            "notifications.language",
            "notifications.smtp_from",
            "notifications.smtp_host",
            "notifications.smtp_port",
        ]
    );

    // 環境変数が無ければ、テーブルの設定が揃っていても SMTP 設定は成立しない
    let reason = smtp_config(&loaded, &SmtpCredentials::from_lookup(|_| None)).unwrap_err();
    assert!(reason.contains("LLMLB_SMTP_USERNAME"), "{reason}");
}

#[tokio::test]
async fn ac1_recipients_are_the_admins_who_have_an_email_address() {
    let pool = ready_pool().await;
    create_user_with_email(
        &pool,
        "another-admin",
        UserRole::Admin,
        "another@example.com",
    )
    .await;
    create_user_with_email(&pool, "viewer", UserRole::Viewer, "viewer@example.com").await;
    crate::helpers::create_user_with_jwt(&pool, "admin-without-mail", UserRole::Admin).await;
    let transport = RecordingTransport::default();

    let tick = scheduler(&pool, &transport)
        .tick(at("2026-10-01 09:00:00"))
        .await;

    assert_eq!(
        tick.outcome,
        sent_to(&["another@example.com", "ops@example.com"])
    );
    assert_eq!(
        transport.sent()[0].to,
        vec![
            "another@example.com".to_string(),
            "ops@example.com".to_string()
        ]
    );
}

// ---------------------------------------------------------------------------
// AC-4: 日次ダイジェスト（壁時計基準のスケジューラ）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ac4_digest_lists_the_status_of_every_registered_endpoint() {
    let pool = ready_pool().await;
    register_endpoint(&pool, "gpu-down", EndpointStatus::Offline).await;
    register_endpoint(&pool, "gpu-broken", EndpointStatus::Error).await;
    register_endpoint(&pool, "gpu-new", EndpointStatus::Pending).await;
    let transport = RecordingTransport::default();

    scheduler(&pool, &transport)
        .tick(at("2026-10-01 09:00:00"))
        .await;

    let mail = &transport.sent()[0];
    assert!(
        mail.subject
            .contains("Online 1 / Offline 1 / Error 1 / Pending 1"),
        "{}",
        mail.subject
    );
    for line in [
        "[Online] gpu-a",
        "[Offline] gpu-down",
        "[Error] gpu-broken",
        "[Pending] gpu-new",
    ] {
        assert!(
            mail.body.contains(line),
            "missing {line:?} in:\n{}",
            mail.body
        );
    }
}

#[tokio::test]
async fn ac4_digest_is_sent_even_when_no_endpoint_is_registered() {
    let pool = crate::support::lb::create_test_db_pool().await;
    save_notification_settings(&pool, &complete_settings()).await;
    create_user_with_email(&pool, "ops-admin", UserRole::Admin, "ops@example.com").await;
    let transport = RecordingTransport::default();

    let tick = scheduler(&pool, &transport)
        .tick(at("2026-10-01 09:00:00"))
        .await;

    assert_eq!(tick.outcome, sent_to(&["ops@example.com"]));
    assert!(transport.sent()[0]
        .subject
        .contains("Online 0 / Offline 0 / Error 0 / Pending 0"));
}

#[tokio::test]
async fn ac4_send_time_does_not_drift_across_process_restarts() {
    let pool = ready_pool().await;
    let transport = RecordingTransport::default();

    // 送信時刻より前は待機し、次回実行時刻は当日の 09:00
    let mut first_process = scheduler(&pool, &transport);
    let tick = first_process.tick(at("2026-10-01 08:59:30")).await;
    assert_eq!(tick.outcome, DigestOutcome::NotDue);
    assert_eq!(tick.next_wake, at("2026-10-01 09:00:00"));

    let tick = first_process.tick(at("2026-10-01 09:00:00")).await;
    assert_eq!(tick.outcome, sent_to(&["ops@example.com"]));
    assert_eq!(tick.next_wake, at("2026-10-02 09:00:00"));
    drop(first_process);

    // 13:37 に再起動しても、次回は 13:37 ではなく翌日の 09:00
    let mut restarted = scheduler(&pool, &transport);
    let tick = restarted.tick(at("2026-10-01 13:37:42")).await;
    assert_eq!(tick.outcome, DigestOutcome::NotDue);
    assert_eq!(tick.next_wake, at("2026-10-02 09:00:00"));

    let tick = restarted.tick(at("2026-10-02 08:59:59")).await;
    assert_eq!(tick.outcome, DigestOutcome::NotDue);
    assert_eq!(tick.next_wake, at("2026-10-02 09:00:00"));
    drop(restarted);

    // 深夜にもう一度再起動しても同じ
    let mut restarted_again = scheduler(&pool, &transport);
    let tick = restarted_again.tick(at("2026-10-02 09:00:00")).await;
    assert_eq!(tick.outcome, sent_to(&["ops@example.com"]));
    assert_eq!(tick.next_wake, at("2026-10-03 09:00:00"));

    let subjects: Vec<String> = transport
        .sent()
        .into_iter()
        .map(|mail| mail.subject)
        .collect();
    assert_eq!(subjects.len(), 2);
    assert!(subjects[0].contains("2026-10-01"), "{subjects:?}");
    assert!(subjects[1].contains("2026-10-02"), "{subjects:?}");
}

#[tokio::test]
async fn ac4_digest_is_not_sent_twice_on_the_same_day() {
    let pool = ready_pool().await;
    let transport = RecordingTransport::default();

    let mut running = scheduler(&pool, &transport);
    assert_eq!(
        running.tick(at("2026-10-01 09:00:00")).await.outcome,
        sent_to(&["ops@example.com"])
    );
    // 同じプロセスで何度判定しても送らない
    for now in ["2026-10-01 09:00:01", "2026-10-01 09:01:00"] {
        assert_eq!(running.tick(at(now)).await.outcome, DigestOutcome::NotDue);
    }
    drop(running);

    // 最終送信日は settings に永続化されている
    let last_sent = SettingsStorage::new(pool.clone())
        .get_setting(LAST_SENT_KEY)
        .await
        .unwrap();
    assert_eq!(last_sent.as_deref(), Some("2026-10-01"));

    // 再起動しても、同じ日のうちは送らない
    for now in [
        "2026-10-01 09:00:30",
        "2026-10-01 12:00:00",
        "2026-10-01 23:59:59",
    ] {
        let tick = scheduler(&pool, &transport).tick(at(now)).await;
        assert_eq!(tick.outcome, DigestOutcome::NotDue, "{now}");
    }

    // 送信後に送信時刻を後ろへ変更しても、同じ日には送らない
    save_notification_settings(
        &pool,
        &NotificationSettings {
            daily_digest_time: "18:00".to_string(),
            ..complete_settings()
        },
    )
    .await;
    let tick = scheduler(&pool, &transport)
        .tick(at("2026-10-01 18:00:00"))
        .await;
    assert_eq!(tick.outcome, DigestOutcome::NotDue);
    assert_eq!(tick.next_wake, at("2026-10-02 18:00:00"));

    assert_eq!(transport.sent().len(), 1);
}

#[tokio::test]
async fn ac4_digest_missed_while_stopped_is_sent_once_after_restart() {
    let pool = ready_pool().await;
    let transport = RecordingTransport::default();

    // 09:00 の時点では停止しており、10:15 に起動した
    let mut started_late = scheduler(&pool, &transport);
    let tick = started_late.tick(at("2026-10-01 10:15:00")).await;
    assert_eq!(tick.outcome, sent_to(&["ops@example.com"]));
    // 遅れて送っても、次回は本来の送信時刻に戻る
    assert_eq!(tick.next_wake, at("2026-10-02 09:00:00"));

    let tick = started_late.tick(at("2026-10-01 10:16:00")).await;
    assert_eq!(tick.outcome, DigestOutcome::NotDue);
    assert_eq!(transport.sent().len(), 1);
}

#[tokio::test]
async fn ac4_failed_delivery_is_retried_without_recording_the_day_as_sent() {
    let pool = ready_pool().await;
    let transport = RecordingTransport::default();
    transport.fail_next(1);

    let mut running = scheduler(&pool, &transport);
    let tick = running.tick(at("2026-10-01 09:00:00")).await;
    assert!(
        matches!(&tick.outcome, DigestOutcome::Failed(error) if error.contains("simulated SMTP outage")),
        "{:?}",
        tick.outcome
    );
    assert_eq!(tick.next_wake, at("2026-10-01 09:15:00"));
    let storage = SettingsStorage::new(pool.clone());
    let last_sent = storage.get_setting(LAST_SENT_KEY).await.unwrap();
    assert_eq!(last_sent, None, "a failed delivery must not count as sent");
    // 失敗理由は設定 API から確認できるよう記録される
    let last_error = storage.get_setting(LAST_ERROR_KEY).await.unwrap();
    assert_eq!(
        last_error.as_deref(),
        Some("2026-10-01 09:00 mail delivery failed: simulated SMTP outage")
    );

    // 再試行までの間は送信を試みない
    let tick = running.tick(at("2026-10-01 09:01:00")).await;
    assert_eq!(tick.outcome, DigestOutcome::NotDue);
    assert_eq!(tick.next_wake, at("2026-10-01 09:15:00"));

    let tick = running.tick(at("2026-10-01 09:15:00")).await;
    assert_eq!(tick.outcome, sent_to(&["ops@example.com"]));
    assert_eq!(tick.next_wake, at("2026-10-02 09:00:00"));
    assert_eq!(transport.sent().len(), 1);
    // 成功すると失敗理由は消える
    let last_error = storage.get_setting(LAST_ERROR_KEY).await.unwrap();
    assert_eq!(last_error.as_deref(), Some(""));
}

// ---------------------------------------------------------------------------
// AC-6: テンプレート言語
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ac6_mail_language_follows_the_language_setting() {
    let pool = ready_pool().await;
    let transport = RecordingTransport::default();

    scheduler(&pool, &transport)
        .tick(at("2026-10-01 09:00:00"))
        .await;

    save_notification_settings(
        &pool,
        &NotificationSettings {
            language: Language::En,
            ..complete_settings()
        },
    )
    .await;
    scheduler(&pool, &transport)
        .tick(at("2026-10-02 09:00:00"))
        .await;

    let sent = transport.sent();
    assert_eq!(sent.len(), 2);
    assert!(
        sent[0].subject.contains("日次ダイジェスト"),
        "{}",
        sent[0].subject
    );
    assert!(sent[0].body.contains("状態一覧"), "{}", sent[0].body);
    assert!(
        sent[1].subject.contains("Daily digest"),
        "{}",
        sent[1].subject
    );
    assert!(
        sent[1].body.contains("Registered endpoints: 1"),
        "{}",
        sent[1].body
    );
    assert!(!sent[1].body.contains("状態一覧"), "{}", sent[1].body);
}

// ---------------------------------------------------------------------------
// AC-7: SMTP 設定が未設定・不正な場合
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ac7_incomplete_smtp_settings_disable_only_the_notifications() {
    let pool = ready_pool().await;
    save_notification_settings(
        &pool,
        &NotificationSettings {
            smtp_host: String::new(),
            ..complete_settings()
        },
    )
    .await;
    let transport = RecordingTransport::default();
    let mut running = scheduler(&pool, &transport);

    let tick = running.tick(at("2026-10-01 09:00:00")).await;
    let DigestOutcome::Inactive(status) = &tick.outcome else {
        panic!("expected an inactive outcome, got {:?}", tick.outcome);
    };
    assert_eq!(status.state, NotificationState::Unavailable);
    assert!(
        status
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("SMTP host"),
        "{status:?}"
    );
    assert!(transport.sent().is_empty());

    // 認証情報（環境変数）が無い場合も同じく送信だけを行わない
    let mut without_credentials = llmlb::notifications::DailyDigestScheduler::new(
        pool.clone(),
        std::sync::Arc::new(transport.clone()),
        SmtpCredentials::default(),
    );
    save_notification_settings(&pool, &complete_settings()).await;
    let tick = without_credentials.tick(at("2026-10-01 09:00:00")).await;
    let DigestOutcome::Inactive(status) = &tick.outcome else {
        panic!("expected an inactive outcome, got {:?}", tick.outcome);
    };
    assert!(
        status
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("LLMLB_SMTP_PASSWORD"),
        "{status:?}"
    );
    assert!(transport.sent().is_empty());

    // 設定を直せば、再起動なしで送信が始まる
    let tick = running.tick(at("2026-10-01 09:01:00")).await;
    assert_eq!(tick.outcome, sent_to(&["ops@example.com"]));
}

#[tokio::test]
async fn ac7_reason_for_disabled_notifications_is_written_to_the_log() {
    let pool = ready_pool().await;
    save_notification_settings(
        &pool,
        &NotificationSettings {
            smtp_host: String::new(),
            ..complete_settings()
        },
    )
    .await;
    let transport = RecordingTransport::default();
    let logs = capture_logs();

    let mut running = scheduler(&pool, &transport);
    running.tick(at("2026-10-01 09:00:00")).await;
    let first = logs.contents();
    assert!(first.contains("WARN"), "{first}");
    assert!(first.contains("SMTP host"), "{first}");

    // 同じ理由を毎回ログへ出し続けない
    running.tick(at("2026-10-01 09:01:00")).await;
    assert_eq!(logs.contents(), first);
}

#[tokio::test]
#[serial_test::serial]
async fn ac7_app_serves_requests_while_notifications_are_not_configured() {
    let (app, pool, jwt) = crate::helpers::build_app_with_admin().await;
    let logs = capture_logs();

    // 起動時に呼ばれるバックグラウンドタスク。SMTP 設定も環境変数も無い状態で開始する
    llmlb::notifications::start_daily_digest_task(pool.clone());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while !logs
        .contents()
        .contains("Operational notifications are disabled")
    {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the digest task never reported its state: {}",
            logs.contents()
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let (status, users) = send_json(&app, Method::GET, "/api/users", Some(&jwt), None).await;
    assert_eq!(status, StatusCode::OK, "{users}");
    let (status, me) = send_json(&app, Method::GET, "/api/auth/me", Some(&jwt), None).await;
    assert_eq!(status, StatusCode::OK, "{me}");
}
