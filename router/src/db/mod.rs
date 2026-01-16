//! データベースアクセス層
//!
//! SQLiteベースのデータ永続化

pub mod request_history;

/// ノード管理
pub mod nodes;

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

#[cfg(test)]
pub(crate) mod test_utils {
    use once_cell::sync::Lazy;
    use tokio::sync::Mutex as TokioMutex;

    /// テスト用のグローバルロック（環境変数の競合を防ぐ）
    /// db配下のすべてのテストで共有
    pub static TEST_LOCK: Lazy<TokioMutex<()>> = Lazy::new(|| TokioMutex::new(()));
}
