//! LLM load balancer Server
//!
//! 複数LLMノードを管理する中央サーバー

#![warn(missing_docs)]

/// 共通型定義（llmlb-commonから統合）
pub mod common;

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

/// モデル管理
pub mod models;

/// ロギング初期化ユーティリティ
pub mod logging;

/// GUIユーティリティ（トレイアイコン等）
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub mod gui;

/// 設定管理（環境変数ヘルパー）
pub mod config;

/// エンドポイントタイプ自動判別
pub mod detection;

/// システム情報取得（GPU/デバイス情報）
pub mod system_info;

/// xLLMクライアント（ダウンロード・メタデータ）
pub mod xllm;

/// モデルメタデータ取得
pub mod metadata;

/// マルチエンジンモデルダウンロード
pub mod download;

/// マルチエンジンモデル削除
pub mod delete;

/// JWT秘密鍵管理
pub mod jwt_secret;

/// 認証・認可機能
pub mod auth;

/// 監査ログシステム (SPEC-8301d106)
pub mod audit;

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

/// サーバーインスタンスの排他制御（シングル実行制約）
pub mod lock;

/// Inference request gate (self-update drain)
pub mod inference_gate;

/// Shutdown controller (self-update restart)
pub mod shutdown;

/// Self-update manager
pub mod update;

/// サーバー初期化（DB接続、レジストリ、ヘルスチェッカー等）
pub mod bootstrap;

/// axumサーバー起動・シャットダウン
pub mod server;

/// アプリケーション状態
///
/// 横断的に使う基盤（DB・HTTPクライアント・イベントバス）はトップレベルに置き、
/// ドメイン固有の状態はサブステートにまとめる（FR-011）。
#[derive(Clone)]
pub struct AppState {
    /// ロードバランシング（エンドポイント選択・レジストリ・リクエスト履歴）
    pub balancer: BalancerState,
    /// データベース接続プール
    pub db_pool: sqlx::SqlitePool,
    /// 認証
    pub auth: AuthState,
    /// 共有HTTPクライアント（接続プーリング有効）
    pub http_client: reqwest::Client,
    /// ダッシュボードイベントバス
    pub event_bus: events::SharedEventBus,
    /// 推論ゲート・シャットダウン・自己更新
    pub lifecycle: LifecycleState,
    /// 監査ログ (SPEC-8301d106)
    pub audit: AuditState,
}

/// ロードバランシングの状態
#[derive(Clone)]
pub struct BalancerState {
    /// ロードマネージャー
    pub load_manager: balancer::LoadManager,
    /// エンドポイントレジストリ
    pub endpoint_registry: registry::endpoints::EndpointRegistry,
    /// リクエスト履歴ストレージ
    pub request_history: std::sync::Arc<db::request_history::RequestHistoryStorage>,
}

/// 認証の状態
#[derive(Clone)]
pub struct AuthState {
    /// JWT秘密鍵
    pub jwt_secret: String,
}

/// プロセスのライフサイクル（推論ゲート・シャットダウン・自己更新）の状態
#[derive(Clone)]
pub struct LifecycleState {
    /// Inference gate (used for self-update drain)
    pub inference_gate: inference_gate::InferenceGate,
    /// Cooperative shutdown controller
    pub shutdown: shutdown::ShutdownController,
    /// Self-update manager
    pub update_manager: update::UpdateManager,
}

/// 監査ログの状態 (SPEC-8301d106)
#[derive(Clone)]
pub struct AuditState {
    /// 監査ログライター
    pub writer: audit::writer::AuditLogWriter,
    /// 監査ログストレージ
    pub storage: std::sync::Arc<db::audit_log::AuditLogStorage>,
    /// 監査ログアーカイブDBプール
    pub archive_pool: Option<sqlx::SqlitePool>,
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

    #[test]
    fn test_app_state_groups_domain_substates() {
        // FR-011: ドメイン固有の状態はサブステート経由で参照する
        let _: fn(&AppState) -> &balancer::LoadManager = |state| &state.balancer.load_manager;
        let _: fn(&AppState) -> &registry::endpoints::EndpointRegistry =
            |state| &state.balancer.endpoint_registry;
        let _: fn(&AppState) -> &std::sync::Arc<db::request_history::RequestHistoryStorage> =
            |state| &state.balancer.request_history;
        let _: fn(&AppState) -> &String = |state| &state.auth.jwt_secret;
        let _: fn(&AppState) -> &inference_gate::InferenceGate =
            |state| &state.lifecycle.inference_gate;
        let _: fn(&AppState) -> &shutdown::ShutdownController = |state| &state.lifecycle.shutdown;
        let _: fn(&AppState) -> &update::UpdateManager = |state| &state.lifecycle.update_manager;
        let _: fn(&AppState) -> &audit::writer::AuditLogWriter = |state| &state.audit.writer;
        let _: fn(&AppState) -> &std::sync::Arc<db::audit_log::AuditLogStorage> =
            |state| &state.audit.storage;
        let _: fn(&AppState) -> &Option<sqlx::SqlitePool> = |state| &state.audit.archive_pool;
    }
}
