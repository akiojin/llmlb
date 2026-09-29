//! データベースアクセス層
//!
//! SQLiteベースのデータ永続化

pub mod request_history;

/// ユーザー管理
pub mod users;

/// APIキー管理
pub mod api_keys;

pub mod models;

/// 招待コード管理
pub mod invitations;

/// パスワードリセットトークン管理
pub mod password_reset_tokens;

/// データベースマイグレーション
pub mod migrations;

/// エンドポイント管理
pub mod endpoints;

/// エンドポイント日次統計（SPEC-8c32349f）
pub mod endpoint_daily_stats;

/// クラウドメトリクス日次集計（SPEC #582 US-005）
pub mod cloud_metrics;

/// ダウンロードタスク管理（SPEC-e8e9326e）
pub mod download_tasks;

/// 監査ログストレージ（SPEC-8301d106）
pub mod audit_log;

/// 設定管理
pub mod settings;

/// Repository traitパターン（テスタビリティ向上）
pub mod traits;

#[cfg(test)]
pub(crate) mod test_utils {
    use once_cell::sync::Lazy;
    use sqlx::SqlitePool;
    use tokio::sync::Mutex as TokioMutex;

    /// テスト用のグローバルロック（環境変数の競合を防ぐ）
    /// db配下のすべてのテストで共有
    pub static TEST_LOCK: Lazy<TokioMutex<()>> = Lazy::new(|| TokioMutex::new(()));

    /// テスト用のインメモリSQLiteプールを作成し、マイグレーションを実行する
    pub async fn test_db_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to create test database");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");
        pool
    }

    /// テスト用AppStateを構築するビルダー
    pub struct TestAppStateBuilder {
        db_pool: SqlitePool,
    }

    impl TestAppStateBuilder {
        /// デフォルト設定でビルダーを生成する
        pub async fn new() -> Self {
            let db_pool = test_db_pool().await;
            Self { db_pool }
        }

        /// AppStateを構築する
        pub async fn build(self) -> crate::AppState {
            let request_history = std::sync::Arc::new(
                crate::db::request_history::RequestHistoryStorage::new(self.db_pool.clone()),
            );
            let endpoint_registry =
                crate::registry::endpoints::EndpointRegistry::new(self.db_pool.clone())
                    .await
                    .expect("Failed to create endpoint registry");
            let endpoint_registry_arc = std::sync::Arc::new(endpoint_registry.clone());
            let load_manager = crate::balancer::LoadManager::new(endpoint_registry_arc);
            let http_client = reqwest::Client::new();
            let inference_gate = crate::inference_gate::InferenceGate::default();
            let shutdown = crate::shutdown::ShutdownController::default();
            let update_manager = crate::update::UpdateManager::new(
                http_client.clone(),
                inference_gate.clone(),
                shutdown.clone(),
            )
            .expect("Failed to create update manager");
            let audit_log_storage = std::sync::Arc::new(
                crate::db::audit_log::AuditLogStorage::new(self.db_pool.clone()),
            );
            let audit_log_writer = crate::audit::writer::AuditLogWriter::new(
                crate::db::audit_log::AuditLogStorage::new(self.db_pool.clone()),
                crate::audit::writer::AuditLogWriterConfig::default(),
            );
            crate::AppState {
                balancer: crate::BalancerState {
                    load_manager,
                    endpoint_registry,
                    request_history,
                },
                db_pool: self.db_pool,
                auth: crate::AuthState {
                    jwt_secret: "test-secret".into(),
                },
                http_client,
                event_bus: crate::events::create_shared_event_bus(),
                lifecycle: crate::LifecycleState {
                    inference_gate,
                    shutdown,
                    update_manager,
                },
                audit: crate::AuditState {
                    writer: audit_log_writer,
                    storage: audit_log_storage,
                    archive_pool: None,
                },
            }
        }
    }
}
