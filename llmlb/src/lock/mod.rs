//! サーバーインスタンスの排他制御（シングル実行制約）
//!
//! 同一ポートでのサーバー重複起動を防止するためのファイルロック機構を提供します。
//!
//! # 機能
//!
//! - クロスプラットフォームファイルロック（fs2）
//! - ロックファイルにJSON形式でPID・起動時刻・ポートを記録
//! - 残留ロックの自動検出と解除（PID検証）
//! - グレースフルシャットダウン対応（Dropトレイト）

mod server_lock;

pub use server_lock::{
    is_process_running, list_all_locks, lock_dir, lock_path, read_lock_info, stop_process,
    LockError, LockInfo, ServerLock,
};
