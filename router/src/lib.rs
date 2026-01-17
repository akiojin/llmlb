//! LLM Router Server
//!
//! 複数LLMノードを管理する中央サーバー

#![warn(missing_docs)]

/// REST APIハンドラー
pub mod api;

/// ロードバランサー（ラウンドロビン、負荷ベースのロードバランシング）
pub mod balancer;

/// クラウド呼び出しメトリクス
pub mod cloud_metrics;

/// ヘルスチェック監視
pub mod health;

/// ノード登録管理
pub mod registry;

/// データベースアクセス
pub mod db;

/// メトリクス収集・管理
pub mod metrics;

/// モデル管理（GPU選択ロジック）
pub mod models;

/// ロギング初期化ユーティリティ
pub mod logging;

/// GUIユーティリティ（トレイアイコン等）
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub mod gui;

/// 設定管理（環境変数ヘルパー）
pub mod config;

/// JWT秘密鍵管理
pub mod jwt_secret;

/// 認証・認可機能
pub mod auth;

/// CLIインターフェース
pub mod cli;

/// ダッシュボードイベントバス
pub mod events;

/// トークン抽出・推定
pub mod token;

/// 型定義
pub mod types;

/// モデル同期
pub mod sync;

/// アプリケーション状態
#[derive(Clone)]
pub struct AppState {
    /// ノードレジストリ
    pub registry: registry::NodeRegistry,
    /// ロードマネージャー
    pub load_manager: balancer::LoadManager,
    /// リクエスト履歴ストレージ
    pub request_history: std::sync::Arc<db::request_history::RequestHistoryStorage>,
    /// データベース接続プール
    pub db_pool: sqlx::SqlitePool,
    /// JWT秘密鍵
    pub jwt_secret: String,
    /// 共有HTTPクライアント（接続プーリング有効）
    pub http_client: reqwest::Client,
    /// リクエスト待機キュー設定
    pub queue_config: config::QueueConfig,
    /// ダッシュボードイベントバス
    pub event_bus: events::SharedEventBus,
    /// エンドポイントレジストリ（新エンドポイント管理システム）
    pub endpoint_registry: Option<registry::endpoints::EndpointRegistry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_has_shared_http_client() {
        // AppStateにhttp_clientフィールドが存在することを確認
        // この時点ではコンパイルエラーになるはず（http_clientフィールドがまだない場合）
        let _client_type: fn(&AppState) -> &reqwest::Client = |state| &state.http_client;
    }
}
