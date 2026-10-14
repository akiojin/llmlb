//! 自己更新の状態を UI 層へ通知する抽象。

use super::schedule;

/// 自己更新の状態を UI 層へ通知する抽象。
///
/// arch-review \[M8\]: update ドメインが `gui::tray::TrayEventProxy` へ直接依存して
/// いたため、通知先を trait として逆転させた（依存方向は gui → update）。
/// gui 側が本 trait を `TrayEventProxy` に実装する。
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub trait UpdateNotifier: Send + Sync {
    /// 新しいバージョンが利用可能。
    fn notify_update_available(&self, latest: String);
    /// 更新ペイロードのダウンロードが完了し適用可能。
    fn notify_update_ready(&self);
    /// 更新フローが失敗。
    fn notify_update_failed(&self, message: String);
    /// 既に最新。
    fn notify_update_up_to_date(&self);
    /// 現在の更新スケジュール（`None` で表示クリア）。
    fn notify_schedule(&self, schedule: Option<schedule::UpdateSchedule>);
}
