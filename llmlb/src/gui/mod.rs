//! GUIユーティリティ（トレイアイコンなど、Windows/macOSのみ）。
//! Linux ではコンパイルされないため、GUI・依存変更時の macOS CI で lint を検査する。

#![cfg(any(target_os = "windows", target_os = "macos"))]

/// llmlb用システムトレイ機能。
pub mod tray;
