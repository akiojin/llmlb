//! AC-3（Issue #809）: エンドポイントの Offline 到達の即時通知
//!
//! すべて記録用トランスポートを使い、実際の送信は行わない。接頭辞は SPEC #777 の受け入れ基準:
//! `ac3_` は購読・判定・重複抑止（Issue #809 AC-1 / AC-2）、`ac6_` は本文の言語（同 AC-3）、
//! `ac7_` は通知が無効・未設定のときの挙動（同 AC-4）。

use crate::helpers::{
    at, build_app_with_admin, capture_logs, complete_settings, create_user_with_email, ready_pool,
    register_endpoint, save_notification_settings, send_json, smtp_credentials, RecordingTransport,
};
use axum::http::{Method, StatusCode};
use chrono::{TimeZone, Utc};
use llmlb::common::auth::UserRole;
use llmlb::events::{DashboardEvent, DashboardEventBus};
use llmlb::notifications::{
    AlertOutcome, Language, NotificationSettings, NotificationState, OfflineAlertNotifier,
    SmtpCredentials,
};
use llmlb::types::endpoint::{Endpoint, EndpointStatus, EndpointType};
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use EndpointStatus::{Error, Offline, Online, Pending};

/// 記録用トランスポートを使う通知器を作る
fn notifier(pool: &SqlitePool, transport: &RecordingTransport) -> OfflineAlertNotifier {
    OfflineAlertNotifier::new(
        pool.clone(),
        Arc::new(transport.clone()),
        smtp_credentials(),
    )
}

fn status_changed(
    runtime_id: Uuid,
    old_status: EndpointStatus,
    new_status: EndpointStatus,
) -> DashboardEvent {
    DashboardEvent::EndpointStatusChanged {
        runtime_id,
        old_status,
        new_status,
    }
}

fn sent_to(recipients: &[&str]) -> AlertOutcome {
    AlertOutcome::Sent {
        recipients: recipients
            .iter()
            .map(|address| address.to_string())
            .collect(),
    }
}

/// ヘルスチェックに失敗して Offline になった Ollama エンドポイントを登録する
async fn register_failed_endpoint(pool: &SqlitePool, name: &str) -> Uuid {
    let mut endpoint = Endpoint::new(
        name.to_string(),
        format!("http://{name}.example:11434"),
        EndpointType::Ollama,
    );
    endpoint.status = Offline;
    endpoint.last_seen = Some(Utc.with_ymd_and_hms(2026, 9, 30, 22, 10, 0).unwrap());
    endpoint.last_error = Some("connection refused".to_string());
    llmlb::db::endpoints::create_endpoint(pool, &endpoint)
        .await
        .expect("register endpoint");
    endpoint.id
}

/// 発行済みのイベントを、購読タスクと同じ経路ですべて処理させる
///
/// バスを破棄するとチャネルが閉じ、タスクは残りのイベントを処理してから終了する。
async fn run_until_closed(
    notifier: OfflineAlertNotifier,
    bus: DashboardEventBus,
    events: tokio::sync::broadcast::Receiver<DashboardEvent>,
) {
    drop(bus);
    tokio::time::timeout(Duration::from_secs(30), notifier.run(events))
        .await
        .expect("the alert task must stop when the event bus is closed");
}

// ---------------------------------------------------------------------------
// AC-3: 購読と Offline 到達の判定
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ac3_subscriber_mails_the_admins_only_when_an_endpoint_arrives_at_offline() {
    let pool = ready_pool().await;
    create_user_with_email(&pool, "second-admin", UserRole::Admin, "sre@example.com").await;
    create_user_with_email(&pool, "viewer", UserRole::Viewer, "viewer@example.com").await;
    let gpu_b = register_endpoint(&pool, "gpu-b", Offline).await;
    let gpu_c = register_endpoint(&pool, "gpu-c", Online).await;
    let transport = RecordingTransport::default();

    let bus = DashboardEventBus::new();
    let events = bus.subscribe();
    // Offline 以外への遷移と、Offline のままの更新では送らない
    bus.publish(status_changed(gpu_c, Pending, Online));
    bus.publish(status_changed(gpu_b, Online, Error));
    bus.publish(status_changed(gpu_c, Offline, Offline));
    bus.publish(status_changed(gpu_c, Offline, Online));
    // 状態変化以外のイベントでも送らない
    bus.publish(DashboardEvent::UpdateStateChanged);
    bus.publish(DashboardEvent::NodeRemoved { runtime_id: gpu_c });
    assert!(transport.sent().is_empty());
    // Offline への到達で送る
    bus.publish(status_changed(gpu_b, Error, Offline));
    run_until_closed(notifier(&pool, &transport), bus, events).await;

    let sent = transport.sent();
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(sent[0].to, ["ops@example.com", "sre@example.com"]);
    assert_eq!(
        sent[0].subject,
        "[llmlb] エンドポイントが Offline になりました: gpu-b"
    );
    assert!(
        sent[0]
            .body
            .contains("- gpu-b (xllm) http://gpu-b.example:8080\n    直前の状態: Error\n"),
        "{}",
        sent[0].body
    );
}

#[tokio::test]
async fn ac3_registry_offline_transitions_reach_mail_once_until_recovery() {
    let pool = ready_pool().await;
    create_user_with_email(&pool, "viewer", UserRole::Viewer, "viewer@example.com").await;
    let id = register_endpoint(&pool, "registry-offline", Pending).await;
    let registry = llmlb::registry::endpoints::EndpointRegistry::new(pool.clone())
        .await
        .unwrap();
    let bus = llmlb::events::create_shared_event_bus();
    registry.set_event_bus(bus.clone());
    let events = bus.subscribe();
    let mut observed = bus.subscribe();
    let transport = RecordingTransport::default();

    for status in [Offline, Offline, Online, Error, Offline] {
        registry
            .update_status(id, status, None, None)
            .await
            .unwrap();
    }
    for (previous, next) in [
        (Pending, Offline),
        (Offline, Online),
        (Online, Error),
        (Error, Offline),
    ] {
        match observed.try_recv().unwrap() {
            DashboardEvent::EndpointStatusChanged {
                runtime_id,
                old_status,
                new_status,
            } => assert_eq!((runtime_id, old_status, new_status), (id, previous, next)),
            event => panic!("unexpected registry event: {event:?}"),
        }
    }
    assert!(matches!(
        observed.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
    drop(observed);
    drop(registry);
    // registryのpublisherと通知器のsubscriberを、手動publishを使わず接続する。
    let bus = Arc::try_unwrap(bus).ok().unwrap();
    run_until_closed(notifier(&pool, &transport), bus, events).await;

    let sent = transport.sent();
    assert_eq!(sent.len(), 2, "Offline同値更新は送らず、復帰後に再送する");
    for mail in sent {
        assert_eq!(mail.to, ["ops@example.com"]);
        assert_eq!(
            mail.subject,
            "[llmlb] エンドポイントが Offline になりました: registry-offline"
        );
    }
}

#[tokio::test]
async fn ac3_every_non_offline_arrival_is_ignored() {
    let pool = ready_pool().await;
    let gpu_b = register_endpoint(&pool, "gpu-b", Offline).await;
    let transport = RecordingTransport::default();
    let mut alerts = notifier(&pool, &transport);
    let now = at("2026-10-01 03:14:00");

    for (old_status, new_status) in [
        (Pending, Online),
        (Online, Error),
        (Offline, Online),
        (Offline, Error),
        (Offline, Pending),
        (Offline, Offline),
    ] {
        assert_eq!(
            alerts
                .handle(&status_changed(gpu_b, old_status, new_status), now)
                .await,
            AlertOutcome::Ignored,
            "{old_status:?} -> {new_status:?}"
        );
    }
    assert!(transport.sent().is_empty());

    for old_status in [Pending, Online, Error] {
        // 復帰を挟むので、到達のたびに送る
        alerts
            .handle(&status_changed(gpu_b, Offline, Online), now)
            .await;
        assert_eq!(
            alerts
                .handle(&status_changed(gpu_b, old_status, Offline), now)
                .await,
            sent_to(&["ops@example.com"]),
            "{old_status:?} -> Offline"
        );
    }
    assert_eq!(transport.sent().len(), 3);
}

#[tokio::test]
async fn ac3_endpoint_that_is_no_longer_registered_is_not_reported() {
    let pool = ready_pool().await;
    let transport = RecordingTransport::default();
    let mut alerts = notifier(&pool, &transport);

    let outcome = alerts
        .handle(
            &status_changed(Uuid::new_v4(), Online, Offline),
            at("2026-10-01 03:14:00"),
        )
        .await;
    assert_eq!(outcome, AlertOutcome::Ignored);
    assert!(transport.sent().is_empty());
}

// ---------------------------------------------------------------------------
// AC-3: 重複抑止
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ac3_consecutive_offline_events_send_one_mail_per_endpoint() {
    let pool = ready_pool().await;
    let gpu_b = register_endpoint(&pool, "gpu-b", Offline).await;
    let gpu_c = register_endpoint(&pool, "gpu-c", Offline).await;
    let transport = RecordingTransport::default();

    let bus = DashboardEventBus::new();
    let events = bus.subscribe();
    bus.publish(status_changed(gpu_b, Online, Offline));
    // 同じエンドポイントの Offline 到達が、復帰を挟まずに続く
    bus.publish(status_changed(gpu_b, Online, Offline));
    bus.publish(status_changed(gpu_b, Error, Offline));
    bus.publish(status_changed(gpu_b, Offline, Offline));
    // 別のエンドポイントは別に数える
    bus.publish(status_changed(gpu_c, Error, Offline));
    bus.publish(status_changed(gpu_c, Error, Offline));
    run_until_closed(notifier(&pool, &transport), bus, events).await;

    let subjects: Vec<String> = transport
        .sent()
        .into_iter()
        .map(|mail| mail.subject)
        .collect();
    assert_eq!(
        subjects,
        [
            "[llmlb] エンドポイントが Offline になりました: gpu-b",
            "[llmlb] エンドポイントが Offline になりました: gpu-c",
        ]
    );
}

#[tokio::test]
async fn ac3_offline_is_reported_again_only_after_leaving_offline() {
    let pool = ready_pool().await;
    let gpu_b = register_endpoint(&pool, "gpu-b", Offline).await;
    let transport = RecordingTransport::default();
    let mut alerts = notifier(&pool, &transport);
    let now = at("2026-10-01 03:14:00");

    assert_eq!(
        alerts
            .handle(&status_changed(gpu_b, Online, Offline), now)
            .await,
        sent_to(&["ops@example.com"])
    );
    assert_eq!(
        alerts
            .handle(&status_changed(gpu_b, Online, Offline), now)
            .await,
        AlertOutcome::Duplicate
    );
    assert_eq!(transport.sent().len(), 1);

    // Offline 以外を経由すると、次の到達でまた送る
    alerts
        .handle(&status_changed(gpu_b, Offline, Error), now)
        .await;
    assert_eq!(
        alerts
            .handle(&status_changed(gpu_b, Error, Offline), now)
            .await,
        sent_to(&["ops@example.com"])
    );
    assert_eq!(transport.sent().len(), 2);
}

#[tokio::test]
async fn ac3_failed_delivery_does_not_suppress_the_next_offline_event() {
    let pool = ready_pool().await;
    let gpu_b = register_endpoint(&pool, "gpu-b", Offline).await;
    let transport = RecordingTransport::default();
    let mut alerts = notifier(&pool, &transport);
    let now = at("2026-10-01 03:14:00");

    transport.fail_next(1);
    let outcome = alerts
        .handle(&status_changed(gpu_b, Online, Offline), now)
        .await;
    assert!(
        matches!(&outcome, AlertOutcome::Failed(reason) if reason.contains("simulated SMTP outage")),
        "{outcome:?}"
    );
    assert!(transport.sent().is_empty());

    // 届いていない通知を「通知済み」として抑止しない
    assert_eq!(
        alerts
            .handle(&status_changed(gpu_b, Online, Offline), now)
            .await,
        sent_to(&["ops@example.com"])
    );
}

/// イベントを取りこぼすと復帰を見逃している可能性があるので、抑止の状態を捨てる
/// （重複 1 通のほうが、障害の取りこぼしより安全）
#[tokio::test]
async fn ac3_missed_events_reset_the_duplicate_suppression() {
    let pool = ready_pool().await;
    let gpu_b = register_endpoint(&pool, "gpu-b", Offline).await;
    let transport = RecordingTransport::default();
    let mut alerts = notifier(&pool, &transport);
    alerts
        .handle(
            &status_changed(gpu_b, Online, Offline),
            at("2026-10-01 03:14:00"),
        )
        .await;
    assert_eq!(transport.sent().len(), 1);

    let bus = DashboardEventBus::new();
    let events = bus.subscribe();
    // チャネル容量（1024）を超えて発行し、購読側に取りこぼしを起こす
    for _ in 0..2048 {
        bus.publish(DashboardEvent::UpdateStateChanged);
    }
    bus.publish(status_changed(gpu_b, Online, Offline));
    run_until_closed(alerts, bus, events).await;

    assert_eq!(transport.sent().len(), 2);
}

// ---------------------------------------------------------------------------
// AC-6: テンプレート言語
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ac6_offline_alert_language_follows_the_language_setting() {
    let pool = ready_pool().await;
    let gpu_b = register_failed_endpoint(&pool, "gpu-b").await;
    let gpu_c = register_failed_endpoint(&pool, "gpu-c").await;
    let transport = RecordingTransport::default();
    let mut alerts = notifier(&pool, &transport);

    alerts
        .handle(
            &status_changed(gpu_b, Error, Offline),
            at("2026-10-01 03:14:00"),
        )
        .await;
    save_notification_settings(
        &pool,
        &NotificationSettings {
            language: Language::En,
            ..complete_settings()
        },
    )
    .await;
    alerts
        .handle(
            &status_changed(gpu_c, Online, Offline),
            at("2026-10-01 03:15:00"),
        )
        .await;

    let sent = transport.sent();
    assert_eq!(sent.len(), 2, "{sent:?}");
    assert_eq!(sent[0].to, ["ops@example.com"]);
    assert_eq!(
        sent[0].subject,
        "[llmlb] エンドポイントが Offline になりました: gpu-b"
    );
    assert_eq!(
        sent[0].body,
        "llmlb 障害通知\n\
         検知時刻: 2026-10-01 03:14（サーバーのローカル時刻）\n\
         \n\
         エンドポイントが Offline になりました。\n\
         - gpu-b (ollama) http://gpu-b.example:11434\n\
         \x20\x20\x20\x20直前の状態: Error\n\
         \x20\x20\x20\x20最終確認: 2026-09-30 22:10 UTC\n\
         \x20\x20\x20\x20最後のエラー: connection refused\n\
         \n\
         このメールは llmlb の運用通知です。通知の有効／無効と宛先は llmlb の通知設定で変更できます。\n"
    );
    assert_eq!(sent[1].to, ["ops@example.com"]);
    assert_eq!(sent[1].subject, "[llmlb] Endpoint went offline: gpu-c");
    assert_eq!(
        sent[1].body,
        "llmlb outage alert\n\
         Detected at: 2026-10-01 03:15 (server local time)\n\
         \n\
         An endpoint went offline.\n\
         - gpu-c (ollama) http://gpu-c.example:11434\n\
         \x20\x20\x20\x20Previous status: Online\n\
         \x20\x20\x20\x20Last seen: 2026-09-30 22:10 UTC\n\
         \x20\x20\x20\x20Last error: connection refused\n\
         \n\
         This is an operational notification from llmlb. Notifications and their recipients can be changed in the llmlb notification settings.\n"
    );
}

// ---------------------------------------------------------------------------
// AC-7: 通知が無効・SMTP 未設定の場合
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ac7_offline_alert_is_not_sent_while_notifications_are_disabled_or_unconfigured() {
    let pool = ready_pool().await;
    let gpu_b = register_endpoint(&pool, "gpu-b", Offline).await;
    let transport = RecordingTransport::default();
    let mut alerts = notifier(&pool, &transport);
    let now = at("2026-10-01 03:14:00");
    let arrival = status_changed(gpu_b, Online, Offline);

    // 通知が無効
    save_notification_settings(
        &pool,
        &NotificationSettings {
            enabled: false,
            ..complete_settings()
        },
    )
    .await;
    let outcome = alerts.handle(&arrival, now).await;
    let AlertOutcome::Inactive(status) = &outcome else {
        panic!("expected an inactive outcome, got {outcome:?}");
    };
    assert_eq!(status.state, NotificationState::Disabled);

    // SMTP の設定が欠けている
    save_notification_settings(
        &pool,
        &NotificationSettings {
            smtp_host: String::new(),
            ..complete_settings()
        },
    )
    .await;
    let outcome = alerts.handle(&arrival, now).await;
    let AlertOutcome::Inactive(status) = &outcome else {
        panic!("expected an inactive outcome, got {outcome:?}");
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

    // SMTP の認証情報（環境変数）が無い
    save_notification_settings(&pool, &complete_settings()).await;
    let mut without_credentials = OfflineAlertNotifier::new(
        pool.clone(),
        Arc::new(transport.clone()),
        SmtpCredentials::default(),
    );
    let outcome = without_credentials.handle(&arrival, now).await;
    let AlertOutcome::Inactive(status) = &outcome else {
        panic!("expected an inactive outcome, got {outcome:?}");
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

    // 設定を直せば、再起動なしで次の Offline 到達から送信が始まる
    assert_eq!(
        alerts.handle(&arrival, now).await,
        sent_to(&["ops@example.com"])
    );
}

#[tokio::test]
#[serial_test::serial]
async fn ac7_app_serves_requests_while_the_offline_alert_task_is_not_configured() {
    let (app, pool, jwt) = build_app_with_admin().await;
    let gpu_b = register_endpoint(&pool, "gpu-b", Offline).await;
    // 通知は有効だが、SMTP の設定が無い
    save_notification_settings(
        &pool,
        &NotificationSettings {
            smtp_host: String::new(),
            ..complete_settings()
        },
    )
    .await;
    let logs = capture_logs();

    // 起動時に呼ばれるバックグラウンドタスク
    let bus = DashboardEventBus::new();
    llmlb::notifications::start_offline_alert_task(pool.clone(), &bus);
    assert_eq!(bus.subscriber_count(), 1, "the task must be subscribed");

    bus.publish(status_changed(gpu_b, Online, Offline));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while !logs.contents().contains("Offline alert was not sent") {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the alert task never handled the event: {}",
            logs.contents()
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // 送れなかった理由がログに出る
    assert!(logs.contents().contains("WARN"), "{}", logs.contents());
    assert!(logs.contents().contains("SMTP host"), "{}", logs.contents());

    // 送信できなくてもタスクは購読を続け、アプリは応答する
    assert_eq!(bus.subscriber_count(), 1, "the task must keep running");
    let (status, users) = send_json(&app, Method::GET, "/api/users", Some(&jwt), None).await;
    assert_eq!(status, StatusCode::OK, "{users}");
    let (status, me) = send_json(&app, Method::GET, "/api/auth/me", Some(&jwt), None).await;
    assert_eq!(status, StatusCode::OK, "{me}");
}
