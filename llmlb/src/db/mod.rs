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

/// データベースマイグレーション
pub mod migrations;

/// エンドポイント管理
pub mod endpoints;

/// エンドポイント日次統計（SPEC-8c32349f）
pub mod endpoint_daily_stats;

/// ダウンロードタスク管理（SPEC-e8e9326e）
pub mod download_tasks;

/// 監査ログストレージ（SPEC-8301d106）
pub mod audit_log;

/// 設定管理
pub mod settings;

#[cfg(test)]
pub(crate) mod test_utils {
    use once_cell::sync::Lazy;
    use tokio::sync::Mutex as TokioMutex;

    /// テスト用のグローバルロック（環境変数の競合を防ぐ）
    /// db配下のすべてのテストで共有
    pub static TEST_LOCK: Lazy<TokioMutex<()>> = Lazy::new(|| TokioMutex::new(()));
}
