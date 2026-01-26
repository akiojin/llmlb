//! GUIユーティリティ（トレイアイコンなど、Windows/macOSのみ）。

#![cfg(any(target_os = "windows", target_os = "macos"))]

/// llmlb用システムトレイ機能。
pub mod tray;
