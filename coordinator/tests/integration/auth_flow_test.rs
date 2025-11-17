//! 認証フロー統合テスト
//!
//! TDD RED: これらのテストは実装前に失敗する必要があります
//! T015-T017: ログイン成功/失敗、未認証アクセス拒否

use crate::support;
use axum::Router;
use ollama_coordinator_coordinator::{
    api, balancer::LoadManager, registry::AgentRegistry, AppState,
};

async fn build_app() -> Router {
    // AUTH_DISABLED=trueで認証を無効化
    std::env::set_var("AUTH_DISABLED", "true");

    let registry = AgentRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let request_history = std::sync::Arc::new(
        ollama_coordinator_coordinator::db::request_history::RequestHistoryStorage::new().unwrap(),
    );
    let task_manager = ollama_coordinator_coordinator::tasks::DownloadTaskManager::new();
    let db_pool = support::coordinator::create_test_db_pool().await;
    let jwt_secret = support::coordinator::test_jwt_secret();

    let state = AppState {
        registry,
        load_manager,
        request_history,
        task_manager,
        db_pool,
        jwt_secret,
    };

    api::create_router(state)
}

/// T015: ログイン成功フローのテスト
#[tokio::test]
#[ignore = "RED phase: waiting for implementation"]
async fn test_login_success_flow() {
    let _app = build_app();

    // REDフェーズ: この機能は未実装
    // 実装後は以下のフローをテスト：
    // 1. POST /api/auth/login で正しいユーザー名とパスワードを送信
    // 2. 200 OK とJWTトークンを受信
    // 3. 受信したトークンで GET /api/auth/me にアクセス
    // 4. ユーザー情報が返される

    panic!("RED: Login success flow not yet implemented");
}

/// T016: ログイン失敗フロー（間違ったパスワード）のテスト
#[tokio::test]
#[ignore = "RED phase: waiting for implementation"]
async fn test_login_failure_wrong_password() {
    let _app = build_app();

    // REDフェーズ: この機能は未実装
    // 実装後は以下のフローをテスト：
    // 1. POST /api/auth/login で間違ったパスワードを送信
    // 2. 401 Unauthorized を受信
    // 3. エラーメッセージの検証

    panic!("RED: Login failure flow not yet implemented");
}

/// T017: 未認証でのダッシュボードアクセス拒否テスト
#[tokio::test]
#[ignore = "RED phase: waiting for implementation"]
async fn test_unauthorized_dashboard_access() {
    let _app = build_app();

    // REDフェーズ: この機能は未実装
    // 実装後は以下のフローをテスト：
    // 1. 認証トークンなしで GET /api/users にアクセス
    // 2. 401 Unauthorized を受信
    // 3. 無効なトークンでアクセス
    // 4. 401 Unauthorized を受信

    panic!("RED: Unauthorized dashboard access not yet implemented");
}
