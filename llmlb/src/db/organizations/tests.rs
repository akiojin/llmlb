use super::*;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::borrow::Cow;

#[tokio::test]
async fn organization_migration_preserves_legacy_keys_and_membership_survives_restart() {
    let directory = tempfile::tempdir().unwrap();
    let options = SqliteConnectOptions::new()
        .filename(directory.path().join("legacy.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options.clone())
        .await
        .unwrap();
    let current = sqlx::migrate!("./migrations");
    let previous = sqlx::migrate::Migrator {
        migrations: Cow::Owned(
            current
                .iter()
                .filter(|migration| migration.version < 20261030050000)
                .cloned()
                .collect(),
        ),
        ..sqlx::migrate::Migrator::DEFAULT
    };
    previous.run(&pool).await.unwrap();
    let issuer = Uuid::new_v4();
    let member = Uuid::new_v4();
    for (id, username, role) in [
        (issuer, "legacy-global", "admin"),
        (member, "legacy-member", "viewer"),
    ] {
        sqlx::query("INSERT INTO users(id,username,password_hash,role,created_at,password_changed_at) VALUES(?,?,?,?,?,?)")
            .bind(id.to_string()).bind(username).bind("legacy-password-hash").bind(role)
            .bind("2025-01-01T00:00:00Z").bind(123_i64)
            .execute(&pool).await.unwrap();
    }
    let key = Uuid::new_v4();
    sqlx::query("INSERT INTO api_keys(id,key_hash,key_prefix,name,created_by,created_at,expires_at,permissions) VALUES(?,?,?,?,?,?,?,?)")
        .bind(key.to_string()).bind("legacy-key-hash").bind("sk_legacy")
        .bind("Legacy operational key").bind(issuer.to_string())
        .bind("2025-01-01T00:00:00Z").bind("2099-01-01T00:00:00Z")
        .bind(r#"["openai.inference","users.manage"]"#)
        .execute(&pool).await.unwrap();
    let before: (String, String, String, String, String) = sqlx::query_as(
        "SELECT key_hash,created_by,created_at,expires_at,permissions FROM api_keys WHERE id = ?",
    )
    .bind(key.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();

    current.run(&pool).await.unwrap();
    let after: (String, String, String, String, String) = sqlx::query_as(
        "SELECT key_hash,created_by,created_at,expires_at,permissions FROM api_keys WHERE id = ?",
    )
    .bind(key.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(before, after);
    let attribution = key_attribution(&pool, key).await.unwrap();
    assert!(attribution.usage_account_id.is_none());
    assert!(attribution.organization_id.is_none());
    assert!(attribution.attribution_revoked_at.is_none());
    let global = account(&pool, issuer).await.unwrap().unwrap();
    assert_eq!(global.role, UserRole::Admin);
    assert_eq!(global.password_changed_at, 123);
    assert!(global.is_active);
    assert!(global.organization_id.is_none());

    let organization = create(&pool, "Persisted organization").await.unwrap();
    let organization_id = organization.id.parse().unwrap();
    set_membership(
        &pool,
        member,
        Some(organization_id),
        OrganizationRole::Admin,
        None,
    )
    .await
    .unwrap();
    pool.close().await;
    let restarted = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    current.run(&restarted).await.unwrap();
    let membership = account(&restarted, member).await.unwrap().unwrap();
    assert_eq!(membership.organization_id, Some(organization_id));
    assert_eq!(membership.organization_role, Some(OrganizationRole::Admin));
    assert_eq!(membership.role, UserRole::Viewer);
    assert_eq!(membership.password_changed_at, 123);
    assert!(membership.organization_active);
    assert!(key_attribution(&restarted, key)
        .await
        .unwrap()
        .organization_id
        .is_none());
    restarted.close().await;
}
