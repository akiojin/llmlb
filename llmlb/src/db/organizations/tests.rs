use super::*;
use crate::common::auth::{ApiKeyPermission, UserRole};
use crate::db::{api_keys, users};
use sqlx::SqlitePool;
use uuid::Uuid;

async fn pool() -> SqlitePool {
    crate::db::test_utils::test_db_pool().await
}

async fn account(pool: &SqlitePool, name: &str, role: UserRole) -> Uuid {
    users::create(pool, name, "password-hash", role, false)
        .await
        .unwrap()
        .id
}

#[tokio::test]
async fn organization_crud_preserves_identity_and_rejects_empty_duplicate_names() {
    let pool = pool().await;
    assert!(create(&pool, " \t\n ").await.is_err());
    let organization = create(&pool, "  Alpha  ").await.unwrap();
    assert_eq!(organization.name, "Alpha");
    assert!(!organization.id.is_nil());
    assert!(create(&pool, "alpha").await.is_err());
    assert!(create(&pool, " ALPHA ").await.is_err());
    let other = create(&pool, "Beta").await.unwrap();
    assert!(rename(&pool, other.id, " Alpha ").await.is_err());
    assert!(rename(&pool, organization.id, " ").await.is_err());
    let renamed = rename(&pool, organization.id, "Gamma").await.unwrap();
    assert_eq!(renamed.id, organization.id);
    assert_eq!(renamed.name, "Gamma");
    assert_eq!(renamed.created_at, organization.created_at);
    let listed = list(&pool).await.unwrap();
    assert_eq!(listed.len(), 2);
    assert!(listed
        .iter()
        .any(|item| item.id == organization.id && item.name == "Gamma"));
}

#[tokio::test]
async fn membership_is_single_and_does_not_grant_system_admin() {
    let pool = pool().await;
    let alpha = create(&pool, "Alpha").await.unwrap();
    let beta = create(&pool, "Beta").await.unwrap();
    let member_id = account(&pool, "member", UserRole::Viewer).await;
    set_membership(&pool, member_id, Some(alpha.id), OrganizationRole::Admin)
        .await
        .unwrap();
    let member = membership(&pool, member_id).await.unwrap().unwrap();
    assert_eq!(member.organization_id, Some(alpha.id));
    assert_eq!(member.organization_role, OrganizationRole::Admin);
    assert!(!member.disabled);
    assert_eq!(
        users::find_by_id(&pool, member_id)
            .await
            .unwrap()
            .unwrap()
            .role,
        UserRole::Viewer
    );
    set_membership(&pool, member_id, Some(beta.id), OrganizationRole::Member)
        .await
        .unwrap();
    let moved = membership(&pool, member_id).await.unwrap().unwrap();
    assert_eq!(moved.organization_id, Some(beta.id));
    assert_eq!(moved.organization_role, OrganizationRole::Member);
    set_membership(&pool, member_id, None, OrganizationRole::Member)
        .await
        .unwrap();
    assert_eq!(
        membership(&pool, member_id)
            .await
            .unwrap()
            .unwrap()
            .organization_id,
        None
    );
    assert!(
        set_membership(&pool, member_id, None, OrganizationRole::Admin)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn membership_rejects_unknown_accounts_and_missing_or_archived_organizations() {
    let pool = pool().await;
    let member_id = account(&pool, "member", UserRole::Viewer).await;
    let organization = create(&pool, "Alpha").await.unwrap();
    assert!(set_membership(
        &pool,
        Uuid::new_v4(),
        Some(organization.id),
        OrganizationRole::Member
    )
    .await
    .is_err());
    assert!(set_membership(
        &pool,
        member_id,
        Some(Uuid::new_v4()),
        OrganizationRole::Member
    )
    .await
    .is_err());
    archive(&pool, organization.id).await.unwrap();
    assert!(set_membership(
        &pool,
        member_id,
        Some(organization.id),
        OrganizationRole::Member
    )
    .await
    .is_err());
    assert_eq!(
        membership(&pool, member_id)
            .await
            .unwrap()
            .unwrap()
            .organization_id,
        None
    );
}

#[tokio::test]
async fn archive_refuses_members_including_disabled_accounts_and_keeps_history_identity() {
    let pool = pool().await;
    let organization = create(&pool, "Alpha").await.unwrap();
    let member_id = account(&pool, "member", UserRole::Viewer).await;
    set_membership(
        &pool,
        member_id,
        Some(organization.id),
        OrganizationRole::Member,
    )
    .await
    .unwrap();
    assert!(archive(&pool, organization.id).await.is_err());
    set_disabled(&pool, member_id, true).await.unwrap();
    assert!(archive(&pool, organization.id).await.is_err());
    set_membership(&pool, member_id, None, OrganizationRole::Member)
        .await
        .unwrap();
    let archived = archive(&pool, organization.id).await.unwrap();
    assert!(archived.archived_at.is_some());
    let listed = list(&pool).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, organization.id);
    let replacement = create(&pool, "alpha").await.unwrap();
    assert_ne!(replacement.id, organization.id);
}

#[tokio::test]
async fn legacy_accounts_and_keys_stay_unattributed_and_operational() {
    let pool = pool().await;
    let issuer = account(&pool, "issuer", UserRole::Admin).await;
    let key = api_keys::create(
        &pool,
        "legacy",
        issuer,
        None,
        vec![ApiKeyPermission::OpenaiInference],
    )
    .await
    .unwrap();
    let member = membership(&pool, issuer).await.unwrap().unwrap();
    assert_eq!(member.organization_id, None);
    assert_eq!(member.organization_role, OrganizationRole::Member);
    assert!(!member.disabled);
    let (usage_account, organization): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT usage_account_id, organization_id FROM api_keys WHERE id = ?")
            .bind(key.id.to_string())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((usage_account, organization), (None, None));
    assert_eq!(
        api_keys::list_by_creator(&pool, issuer)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        users::find_by_id(&pool, issuer)
            .await
            .unwrap()
            .unwrap()
            .role,
        UserRole::Admin
    );
}

#[tokio::test]
async fn upgrading_existing_database_preserves_admin_key_bytes_and_null_attribution() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let all = sqlx::migrate!("./migrations");
    // This is the last schema delivered before organization attribution.
    let legacy = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(
            all.iter()
                .filter(|migration| migration.version <= 20261014154501)
                .cloned()
                .collect(),
        ),
        ..sqlx::migrate::Migrator::DEFAULT
    };
    legacy.run(&pool).await.unwrap();
    let issuer = account(&pool, "legacy-admin", UserRole::Admin).await;
    let key = api_keys::create(
        &pool,
        "legacy-key",
        issuer,
        None,
        vec![
            ApiKeyPermission::UsersManage,
            ApiKeyPermission::OpenaiInference,
        ],
    )
    .await
    .unwrap();
    let before: (String, String, String) =
        sqlx::query_as("SELECT key_hash, created_by, permissions FROM api_keys WHERE id = ?")
            .bind(key.id.to_string())
            .fetch_one(&pool)
            .await
            .unwrap();
    all.run(&pool).await.unwrap();
    let after: (String, String, String, Option<String>, Option<String>) = sqlx::query_as("SELECT key_hash, created_by, permissions, usage_account_id, organization_id FROM api_keys WHERE id = ?")
        .bind(key.id.to_string()).fetch_one(&pool).await.unwrap();
    assert_eq!((after.0, after.1, after.2), before);
    assert_eq!((after.3, after.4), (None, None));
    let administrator = users::find_by_id(&pool, issuer).await.unwrap().unwrap();
    assert_eq!(administrator.role, UserRole::Admin);
    assert_eq!(
        membership(&pool, issuer)
            .await
            .unwrap()
            .unwrap()
            .organization_id,
        None
    );
    assert!(list(&pool).await.unwrap().is_empty());
}

#[tokio::test]
async fn membership_changes_never_rewrite_existing_key_attribution() {
    let pool = pool().await;
    let alpha = create(&pool, "Alpha").await.unwrap();
    let beta = create(&pool, "Beta").await.unwrap();
    let issuer = account(&pool, "issuer", UserRole::Admin).await;
    let member_id = account(&pool, "member", UserRole::Viewer).await;
    set_membership(&pool, member_id, Some(alpha.id), OrganizationRole::Member)
        .await
        .unwrap();
    let key = api_keys::create(
        &pool,
        "issued-for-member",
        issuer,
        None,
        vec![ApiKeyPermission::OpenaiInference],
    )
    .await
    .unwrap();
    sqlx::query("UPDATE api_keys SET usage_account_id = ?, organization_id = ? WHERE id = ?")
        .bind(member_id.to_string())
        .bind(alpha.id.to_string())
        .bind(key.id.to_string())
        .execute(&pool)
        .await
        .unwrap();
    set_membership(&pool, member_id, Some(beta.id), OrganizationRole::Member)
        .await
        .unwrap();
    let attribution: (Option<String>, Option<String>, String) = sqlx::query_as(
        "SELECT usage_account_id, organization_id, created_by FROM api_keys WHERE id = ?",
    )
    .bind(key.id.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        attribution,
        (
            Some(member_id.to_string()),
            Some(alpha.id.to_string()),
            issuer.to_string()
        )
    );
    users::delete(&pool, member_id).await.unwrap();
    let attribution: (Option<String>, Option<String>) =
        sqlx::query_as("SELECT usage_account_id, organization_id FROM api_keys WHERE id = ?")
            .bind(key.id.to_string())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        attribution,
        (Some(member_id.to_string()), Some(alpha.id.to_string()))
    );
}

#[tokio::test]
async fn disabling_last_enabled_admin_is_rejected_and_revocation_never_goes_backwards() {
    let pool = pool().await;
    let first = account(&pool, "first-admin", UserRole::Admin).await;
    assert!(set_disabled(&pool, first, true).await.is_err());
    let second = account(&pool, "second-admin", UserRole::Admin).await;
    let before = users::find_by_id(&pool, first)
        .await
        .unwrap()
        .unwrap()
        .password_changed_at;
    set_disabled(&pool, first, true).await.unwrap();
    let disabled = membership(&pool, first).await.unwrap().unwrap();
    assert!(disabled.disabled);
    let disabled_at = users::find_by_id(&pool, first)
        .await
        .unwrap()
        .unwrap()
        .password_changed_at;
    assert!(disabled_at > before);
    assert!(set_disabled(&pool, second, true).await.is_err());
    assert!(users::delete(&pool, second).await.is_err());
    assert!(
        users::update(&pool, second, None, None, Some(UserRole::Viewer))
            .await
            .is_err()
    );
    set_disabled(&pool, first, false).await.unwrap();
    let enabled_at = users::find_by_id(&pool, first)
        .await
        .unwrap()
        .unwrap()
        .password_changed_at;
    assert!(enabled_at > disabled_at);
    assert!(!membership(&pool, first).await.unwrap().unwrap().disabled);
    assert!(membership(&pool, Uuid::new_v4()).await.unwrap().is_none());
}

#[tokio::test]
async fn direct_sql_cannot_create_inconsistent_memberships_or_disable_the_last_admin() {
    let pool = pool().await;
    let admin = account(&pool, "admin", UserRole::Admin).await;
    let member_id = account(&pool, "member", UserRole::Viewer).await;
    assert!(sqlx::query("UPDATE users SET disabled = 1 WHERE id = ?")
        .bind(admin.to_string())
        .execute(&pool)
        .await
        .is_err());
    assert!(
        sqlx::query("UPDATE users SET organization_id = ? WHERE id = ?")
            .bind(Uuid::new_v4().to_string())
            .bind(member_id.to_string())
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE users SET organization_role = 'admin' WHERE id = ?")
            .bind(member_id.to_string())
            .execute(&pool)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn organizations_and_memberships_survive_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("organizations.sqlite");
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&database)
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options.clone())
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let organization = create(&pool, "Persistent").await.unwrap();
    let member_id = account(&pool, "member", UserRole::Viewer).await;
    set_membership(
        &pool,
        member_id,
        Some(organization.id),
        OrganizationRole::Admin,
    )
    .await
    .unwrap();
    pool.close().await;
    let reopened = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    assert_eq!(list(&reopened).await.unwrap()[0].id, organization.id);
    let membership = membership(&reopened, member_id).await.unwrap().unwrap();
    assert_eq!(membership.organization_id, Some(organization.id));
    assert_eq!(membership.organization_role, OrganizationRole::Admin);
}
