//! Ollama Coordinator Server
//!
//! 複数Ollamaインスタンスを管理する中央サーバー

#![warn(missing_docs)]

/// REST APIハンドラー
pub mod api {
    //! エージェント登録、ヘルスチェック、プロキシAPI
    // TODO: T027で実装
}

/// ロードバランサー
pub mod balancer {
    //! ラウンドロビン、負荷ベースのロードバランシング
    // TODO: T037で実装
}

/// ヘルスチェック
pub mod health {
    //! 定期ヘルスチェック、タイムアウト検知
    // TODO: T033で実装
}

/// エージェント登録管理
pub mod registry {
    //! エージェント状態管理
    // TODO: T029で実装
}

/// データベースアクセス
pub mod db {
    //! SQLxクエリ、マイグレーション
    // TODO: T042で実装
}

/// 設定管理
pub mod config {
    //! 設定ファイル読み込み
    // TODO: T055で実装
}
