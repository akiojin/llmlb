//! AC-5 / AC-7: 通知設定 API（`/api/dashboard/notifications`）
//!
//! ダッシュボードの通知設定 UI が使う API。有効／無効・送信時刻・宛先の確認と変更、
//! および送信できない理由の表示を検証する。

use crate::helpers::{
    at, build_app_with_admin, complete_settings, create_user_with_email, create_user_with_jwt,
    register_endpoint, scheduler, send_json, RecordingTransport,
};
use axum::http::{Method, StatusCode};
use llmlb::common::auth::UserRole;
use llmlb::notifications::{DigestOutcome, NotificationState};
use llmlb::types::endpoint::EndpointStatus;
use serde_json::{json, Value};
use serial_test::serial;

const URI: &str = "/api/dashboard/notifications";

fn complete_settings_json() -> Value {
    serde_json::to_value(complete_settings()).unwrap()
}

#[tokio::test]
#[serial]
async fn ac5_admin_reads_the_default_notification_settings() {
    let (app, _pool, jwt) = build_app_with_admin().await;

    let (status, body) = send_json(&app, Method::GET, URI, Some(&jwt), None).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["settings"],
        json!({
            "enabled": false,
            "smtp_host": "",
            "smtp_port": 587,
            "smtp_from": "",
            "daily_digest_time": "09:00",
            "language": "ja",
        })
    );
    assert_eq!(body["status"]["state"], "disabled");
    assert!(body["status"]["reason"].is_string(), "{body}");
    assert_eq!(body["recipients"], json!([]));
    assert_eq!(body["last_digest_sent_date"], Value::Null);
    assert!(body["credentials_configured"].is_boolean(), "{body}");
}

#[tokio::test]
#[serial]
async fn ac5_admin_changes_the_switch_the_send_time_and_the_other_settings() {
    let (app, _pool, jwt) = build_app_with_admin().await;
    let updated = json!({
        "enabled": true,
        "smtp_host": "  smtp.example.com ",
        "smtp_port": 465,
        "smtp_from": "llmlb@example.com",
        "daily_digest_time": "18:30",
        "language": "en",
    });

    let (status, body) = send_json(&app, Method::PUT, URI, Some(&jwt), Some(updated)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let expected = json!({
        "enabled": true,
        "smtp_host": "smtp.example.com",
        "smtp_port": 465,
        "smtp_from": "llmlb@example.com",
        "daily_digest_time": "18:30",
        "language": "en",
    });
    assert_eq!(body["settings"], expected);

    let (status, body) = send_json(&app, Method::GET, URI, Some(&jwt), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["settings"], expected);

    // 無効へ戻せる
    let mut off = expected.clone();
    off["enabled"] = json!(false);
    let (status, body) = send_json(&app, Method::PUT, URI, Some(&jwt), Some(off)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["settings"]["enabled"], false);
    assert_eq!(body["status"]["state"], "disabled");
}

#[tokio::test]
#[serial]
async fn ac5_recipients_are_the_admins_who_have_an_email_address() {
    let (app, pool, jwt) = build_app_with_admin().await;
    create_user_with_email(&pool, "ops-b", UserRole::Admin, "b@example.com").await;
    create_user_with_email(&pool, "ops-a", UserRole::Admin, "a@example.com").await;
    create_user_with_email(&pool, "viewer", UserRole::Viewer, "viewer@example.com").await;

    let (status, body) = send_json(&app, Method::GET, URI, Some(&jwt), None).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    // `notify-admin`（email 未設定の管理者）と viewer は宛先に含まれない
    assert_eq!(
        body["recipients"],
        json!([
            { "username": "ops-a", "email": "a@example.com" },
            { "username": "ops-b", "email": "b@example.com" },
        ])
    );
}

#[tokio::test]
#[serial]
async fn ac5_scheduler_follows_the_settings_changed_through_the_api() {
    let (app, pool, jwt) = build_app_with_admin().await;
    create_user_with_email(&pool, "ops", UserRole::Admin, "ops@example.com").await;
    register_endpoint(&pool, "gpu-a", EndpointStatus::Online).await;
    let mut settings = complete_settings_json();
    settings["daily_digest_time"] = json!("18:30");
    let (status, body) =
        send_json(&app, Method::PUT, URI, Some(&jwt), Some(settings.clone())).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let transport = RecordingTransport::default();
    let mut running = scheduler(&pool, &transport);
    let tick = running.tick(at("2026-10-01 09:00:00")).await;
    assert_eq!(tick.outcome, DigestOutcome::NotDue);
    assert_eq!(tick.next_wake, at("2026-10-01 18:30:00"));

    let tick = running.tick(at("2026-10-01 18:30:00")).await;
    assert_eq!(
        tick.outcome,
        DigestOutcome::Sent {
            recipients: vec!["ops@example.com".to_string()]
        }
    );

    // 送信済みの日付が設定画面向けに返る
    let (_, body) = send_json(&app, Method::GET, URI, Some(&jwt), None).await;
    assert_eq!(body["last_digest_sent_date"], "2026-10-01");

    // API で無効にすると、次の送信時刻になっても送らない
    settings["enabled"] = json!(false);
    let (status, _) = send_json(&app, Method::PUT, URI, Some(&jwt), Some(settings)).await;
    assert_eq!(status, StatusCode::OK);
    let tick = running.tick(at("2026-10-02 18:30:00")).await;
    let DigestOutcome::Inactive(status) = tick.outcome else {
        panic!("expected an inactive outcome");
    };
    assert_eq!(status.state, NotificationState::Disabled);
    assert_eq!(transport.sent().len(), 1);
}

#[tokio::test]
#[serial]
async fn ac5_malformed_values_are_rejected_and_nothing_is_saved() {
    let (app, _pool, jwt) = build_app_with_admin().await;
    let (status, _) = send_json(
        &app,
        Method::PUT,
        URI,
        Some(&jwt),
        Some(complete_settings_json()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    for (field, value) in [
        ("daily_digest_time", json!("25:00")),
        ("daily_digest_time", json!("9am")),
        ("smtp_port", json!(0)),
        ("smtp_port", json!(70000)),
        ("smtp_from", json!("not-an-address")),
        ("smtp_host", json!("smtp.example.com:587")),
        ("language", json!("fr")),
        ("enabled", json!("yes")),
    ] {
        let mut invalid = complete_settings_json();
        invalid[field] = value.clone();
        let (status, body) = send_json(&app, Method::PUT, URI, Some(&jwt), Some(invalid)).await;
        assert!(
            status == StatusCode::BAD_REQUEST || status == StatusCode::UNPROCESSABLE_ENTITY,
            "{field}={value} must be rejected, got {status}: {body}"
        );
    }

    let (_, body) = send_json(&app, Method::GET, URI, Some(&jwt), None).await;
    assert_eq!(body["settings"], complete_settings_json());
}

#[tokio::test]
#[serial]
async fn ac5_notification_settings_are_admin_only() {
    let (app, pool, _admin_jwt) = build_app_with_admin().await;
    let viewer_jwt = create_user_with_jwt(&pool, "viewer", UserRole::Viewer).await;

    let (status, _) = send_json(&app, Method::GET, URI, Some(&viewer_jwt), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send_json(
        &app,
        Method::PUT,
        URI,
        Some(&viewer_jwt),
        Some(complete_settings_json()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send_json(&app, Method::GET, URI, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = send_json(&app, Method::PUT, URI, None, Some(complete_settings_json())).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[serial]
async fn ac7_enabling_without_smtp_settings_reports_the_reason_and_keeps_the_app_working() {
    let (app, _pool, jwt) = build_app_with_admin().await;
    let mut settings = complete_settings_json();
    settings["smtp_host"] = json!("");

    // 未設定のまま有効化しても拒否せず、送信できない理由を返す
    let (status, body) = send_json(&app, Method::PUT, URI, Some(&jwt), Some(settings)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["settings"]["enabled"], true);
    assert_eq!(body["status"]["state"], "unavailable");
    assert!(
        body["status"]["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("SMTP host"),
        "{body}"
    );

    // 通知以外の機能は影響を受けない
    let (status, _) = send_json(&app, Method::GET, "/api/users", Some(&jwt), None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send_json(
        &app,
        Method::GET,
        "/api/dashboard/endpoints",
        Some(&jwt),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
#[serial]
async fn ac7_uninterpretable_stored_values_are_reported_instead_of_failing() {
    let (app, _pool, jwt) = build_app_with_admin().await;
    let (status, _) = send_json(
        &app,
        Method::PUT,
        URI,
        Some(&jwt),
        Some(complete_settings_json()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // 汎用の設定 API からは検証なしで書き込める
    let (status, _) = send_json(
        &app,
        Method::PUT,
        "/api/dashboard/settings/notifications.smtp_port",
        Some(&jwt),
        Some(json!({ "value": "abc" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send_json(&app, Method::GET, URI, Some(&jwt), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"]["state"], "unavailable");
    assert!(
        body["status"]["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("notifications.smtp_port"),
        "{body}"
    );
    // 解釈できない値は既定値として表示される
    assert_eq!(body["settings"]["smtp_port"], 587);
}
