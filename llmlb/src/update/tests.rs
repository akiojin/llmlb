use super::cache::{save_cache, UpdateCacheFile};
use super::download::{asset_name_from_url, extract_archive, find_extracted_binary};
use super::github::{parse_tag_to_version, GitHubAsset, GitHubRelease};
use super::helper::{
    detect_server_port, parse_port_from_args, record_auto_rollback_history,
    write_restart_args_file, RestartArgsFile,
};
use super::manager::*;
use super::platform::{choose_apply_plan, is_dir_writable, select_assets, ApplyPlan, Platform};
use super::*;
use crate::{inference_gate::InferenceGate, shutdown::ShutdownController};
use chrono::Utc;
use semver::Version;
use std::{fs, sync::atomic::Ordering, time::Duration};

mod manager_state;
mod payload_download;
mod platform_assets;
mod restart_helper;
mod scheduling;
mod state_serialization;
mod version;

fn available_state_with_payload(payload: PayloadState) -> UpdateState {
    UpdateState::Available {
        current: "4.5.0".to_string(),
        latest: "4.5.1".to_string(),
        release_url: "https://example.com/release".to_string(),
        portable_asset_url: Some("https://example.com/portable.tar.gz".to_string()),
        installer_asset_url: None,
        payload,
        checked_at: Utc::now(),
    }
}

/// Helper to create an UpdateManager with an isolated temp data dir for testing.
///
/// Uses a unique env var approach with per-test isolation.
fn test_manager_with_gate(gate: InferenceGate) -> (UpdateManager, tempfile::TempDir) {
    let tmp = tempfile::tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path()).expect("create data dir");
    let manager = UpdateManager::new_with_data_dir(
        reqwest::Client::new(),
        gate,
        ShutdownController::default(),
        tmp.path(),
    )
    .expect("create update manager");
    (manager, tmp)
}
