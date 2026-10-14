//! Issue #913 AC-7: 登録申請を既存の通知経路で管理者へ届ける。

use crate::helpers::{
    build_app_with_admin, complete_settings, create_user_with_email, ready_pool,
    save_notification_settings, send_json, smtp_credentials, RecordingTransport,
};
use axum::http::{Method, StatusCode};
use llmlb::common::auth::UserRole;
use llmlb::notifications::{registration::send_registration_notification, Language, MailError};

#[tokio::test]
async fn ac7_registration_notifies_only_configured_admins_in_selected_language() {
    let pool = ready_pool().await;
    create_user_with_email(&pool, "viewer", UserRole::Viewer, "viewer@example.com").await;
    let transport = RecordingTransport::default();
    for (language, expected) in [
        (Language::Ja, "ユーザー登録申請"),
        (Language::En, "Registration request"),
    ] {
        let mut settings = crate::helpers::complete_settings();
        settings.language = language;
        save_notification_settings(&pool, &settings).await;
        assert!(send_registration_notification(
            &pool,
            &transport,
            &smtp_credentials(),
            "applicant@example.com",
        )
        .await
        .unwrap());
        let sent = transport.sent();
        let mail = sent.last().unwrap();
        assert_eq!(mail.to, ["ops@example.com"]);
        assert!(mail.subject.contains(expected), "{}", mail.subject);
        assert!(mail.body.contains("applicant@example.com"));
        assert!(mail.body.contains("Manage Users"));
        assert!(!mail.body.contains("password"));
    }
}

#[tokio::test]
async fn ac7_registration_skips_missing_email_or_disabled_delivery() {
    let pool = ready_pool().await;
    let transport = RecordingTransport::default();
    let admin = llmlb::db::users::find_by_username(&pool, "ops-admin")
        .await
        .unwrap()
        .unwrap();
    llmlb::db::users::set_email(&pool, admin.id, None)
        .await
        .unwrap();
    assert!(!send_registration_notification(
        &pool,
        &transport,
        &smtp_credentials(),
        "new@example.com"
    )
    .await
    .unwrap());
    let mut settings = crate::helpers::complete_settings();
    settings.enabled = false;
    save_notification_settings(&pool, &settings).await;
    assert!(!send_registration_notification(
        &pool,
        &transport,
        &smtp_credentials(),
        "new@example.com"
    )
    .await
    .unwrap());
    assert!(transport.sent().is_empty());
}

#[tokio::test]
async fn ac7_registration_delivery_failure_is_reported_without_modifying_users() {
    let (app, pool, _) = build_app_with_admin().await;
    let admin = llmlb::db::users::find_by_username(&pool, "notify-admin")
        .await
        .unwrap()
        .unwrap();
    llmlb::db::users::set_email(&pool, admin.id, Some("ops@example.com"))
        .await
        .unwrap();
    save_notification_settings(&pool, &complete_settings()).await;
    let (status, _) = send_json(
        &app,
        Method::POST,
        "/api/auth/register",
        None,
        Some(serde_json::json!({"username":"new@example.com","password":"StrongPass123"})),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let transport = RecordingTransport::default();
    transport.fail_next(1);
    assert!(matches!(
        send_registration_notification(&pool, &transport, &smtp_credentials(), "new@example.com")
            .await,
        Err(MailError::Delivery(_))
    ));
    assert!(llmlb::db::users::find_by_username(&pool, "new@example.com")
        .await
        .unwrap()
        .is_none());
    let pending: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM registration_requests WHERE username = 'new@example.com'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        pending, 1,
        "delivery failure must preserve the accepted request"
    );
    assert!(transport.sent().is_empty());
}
