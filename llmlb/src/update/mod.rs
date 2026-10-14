//! Self-update manager.
//!
//! This module implements:
//! - Update discovery via GitHub Releases
//! - Background download of the preferred payload for the current platform
//! - User-approved apply flow: drain inference requests, then restart into the new version
//! - Internal helper modes (`__internal`) to safely replace binaries / run installers
//! - Update scheduling (immediate / idle / time-based)
//! - Update history recording

mod apply;
mod cache;
mod check;
mod download;
mod dto;
mod github;
mod helper;
pub mod history;
#[cfg(target_os = "macos")]
mod macos_installer;
mod manager;
#[cfg(any(target_os = "windows", target_os = "macos"))]
mod notifier;
mod payload;
mod platform;
pub mod schedule;
#[cfg(any(target_os = "windows", target_os = "macos"))]
mod tray;

pub use dto::*;
pub(crate) use helper::{internal_apply_update, internal_rollback, internal_run_installer};
pub use manager::UpdateManager;
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub use notifier::UpdateNotifier;

#[cfg(test)]
mod tests;
