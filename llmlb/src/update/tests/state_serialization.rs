use super::*;

#[test]
fn applying_state_serializes_phase_metadata() {
    let state = UpdateState::Applying {
        latest: "4.5.1".to_string(),
        method: ApplyMethod::WindowsSetup,
        phase: ApplyPhase::RunningInstaller,
        phase_message: "Installer is running".to_string(),
        started_at: Utc::now(),
        timeout_at: None,
    };

    let json = serde_json::to_value(state).expect("serialize applying state");
    assert_eq!(json["state"], "applying");
    assert_eq!(json["phase"], "running_installer");
    assert!(json.get("phase_message").is_some());
    assert!(json.get("started_at").is_some());
    assert!(json.get("timeout_at").is_none());
}

// =======================================================================
// ApplyRequestMode
// =======================================================================
#[test]
fn apply_request_mode_from_u8_round_trip() {
    assert_eq!(ApplyRequestMode::from_u8(0), ApplyRequestMode::None);
    assert_eq!(ApplyRequestMode::from_u8(1), ApplyRequestMode::Normal);
    assert_eq!(ApplyRequestMode::from_u8(2), ApplyRequestMode::Force);
}

#[test]
fn apply_request_mode_from_u8_unknown_defaults_to_none() {
    assert_eq!(ApplyRequestMode::from_u8(3), ApplyRequestMode::None);
    assert_eq!(ApplyRequestMode::from_u8(255), ApplyRequestMode::None);
}

#[test]
fn apply_request_mode_ordering() {
    assert!(ApplyRequestMode::None < ApplyRequestMode::Normal);
    assert!(ApplyRequestMode::Normal < ApplyRequestMode::Force);
}

// =======================================================================
// ApplyPhase::message
// =======================================================================
#[test]
fn apply_phase_messages_are_non_empty() {
    let phases = [
        ApplyPhase::Starting,
        ApplyPhase::WaitingOldProcessExit,
        ApplyPhase::RunningInstaller,
        ApplyPhase::Restarting,
    ];
    for phase in &phases {
        assert!(
            !phase.message().is_empty(),
            "phase {:?} has empty message",
            phase
        );
    }
}

#[test]
fn apply_phase_starting_message() {
    assert_eq!(ApplyPhase::Starting.message(), "Preparing update apply");
}

#[test]
fn apply_phase_restarting_message() {
    assert_eq!(ApplyPhase::Restarting.message(), "Restarting service");
}

// =======================================================================
// UpdateState serialization
// =======================================================================
#[test]
fn update_state_up_to_date_serialization() {
    let state = UpdateState::UpToDate {
        checked_at: Some(Utc::now()),
    };
    let json = serde_json::to_value(&state).unwrap();
    assert_eq!(json["state"], "up_to_date");
    assert!(json.get("checked_at").is_some());
}

#[test]
fn update_state_up_to_date_none_checked_at() {
    let state = UpdateState::UpToDate { checked_at: None };
    let json = serde_json::to_value(&state).unwrap();
    assert_eq!(json["state"], "up_to_date");
}

#[test]
fn update_state_available_serialization() {
    let state = UpdateState::Available {
        current: "5.0.0".to_string(),
        latest: "5.1.0".to_string(),
        release_url: "https://example.com/release".to_string(),
        portable_asset_url: Some("https://example.com/portable.tar.gz".to_string()),
        installer_asset_url: None,
        payload: PayloadState::NotReady,
        checked_at: Utc::now(),
    };
    let json = serde_json::to_value(&state).unwrap();
    assert_eq!(json["state"], "available");
    assert_eq!(json["current"], "5.0.0");
    assert_eq!(json["latest"], "5.1.0");
    // PayloadState is internally tagged, nested as {"payload": "not_ready"}
    assert_eq!(json["payload"]["payload"], "not_ready");
}

#[test]
fn update_state_draining_serialization() {
    let state = UpdateState::Draining {
        latest: "5.1.0".to_string(),
        in_flight: 5,
        requested_at: Utc::now(),
        timeout_at: Utc::now() + chrono::Duration::seconds(300),
    };
    let json = serde_json::to_value(&state).unwrap();
    assert_eq!(json["state"], "draining");
    assert_eq!(json["in_flight"], 5);
}

#[test]
fn update_state_failed_serialization() {
    let state = UpdateState::Failed {
        latest: Some("5.1.0".to_string()),
        release_url: Some("https://example.com/release".to_string()),
        message: "download failed".to_string(),
        failed_at: Utc::now(),
    };
    let json = serde_json::to_value(&state).unwrap();
    assert_eq!(json["state"], "failed");
    assert_eq!(json["message"], "download failed");
}

#[test]
fn update_state_failed_with_none_fields() {
    let state = UpdateState::Failed {
        latest: None,
        release_url: None,
        message: "unknown error".to_string(),
        failed_at: Utc::now(),
    };
    let json = serde_json::to_value(&state).unwrap();
    assert_eq!(json["state"], "failed");
    assert!(json["latest"].is_null());
    assert!(json["release_url"].is_null());
}

// =======================================================================
// PayloadState serialization
// =======================================================================
#[test]
fn payload_state_not_ready_serialization() {
    let ps = PayloadState::NotReady;
    let json = serde_json::to_value(&ps).unwrap();
    assert_eq!(json["payload"], "not_ready");
}

#[test]
fn payload_state_downloading_serialization() {
    let ps = PayloadState::Downloading {
        started_at: Utc::now(),
        downloaded_bytes: Some(1024),
        total_bytes: Some(2048),
    };
    let json = serde_json::to_value(&ps).unwrap();
    assert_eq!(json["payload"], "downloading");
    assert_eq!(json["downloaded_bytes"], 1024);
    assert_eq!(json["total_bytes"], 2048);
}

#[test]
fn payload_state_downloading_skips_none_bytes() {
    let ps = PayloadState::Downloading {
        started_at: Utc::now(),
        downloaded_bytes: None,
        total_bytes: None,
    };
    let json = serde_json::to_value(&ps).unwrap();
    assert_eq!(json["payload"], "downloading");
    // skip_serializing_if = "Option::is_none" means no key at all
    assert!(json.get("downloaded_bytes").is_none());
    assert!(json.get("total_bytes").is_none());
}

#[test]
fn payload_state_ready_portable_serialization() {
    let ps = PayloadState::Ready {
        kind: PayloadKind::Portable {
            binary_path: "/tmp/llmlb-new".to_string(),
        },
    };
    let json = serde_json::to_value(&ps).unwrap();
    assert_eq!(json["payload"], "ready");
}

#[test]
fn payload_state_error_serialization() {
    let ps = PayloadState::Error {
        message: "download failed".to_string(),
    };
    let json = serde_json::to_value(&ps).unwrap();
    assert_eq!(json["payload"], "error");
    assert_eq!(json["message"], "download failed");
}

// =======================================================================
// PayloadKind serialization
// =======================================================================
#[test]
fn payload_kind_portable_serialization() {
    let kind = PayloadKind::Portable {
        binary_path: "/usr/local/bin/llmlb".to_string(),
    };
    let json = serde_json::to_value(&kind).unwrap();
    // Externally tagged: {"portable": {"binary_path": "..."}}
    assert_eq!(json["portable"]["binary_path"], "/usr/local/bin/llmlb");
}

#[test]
fn payload_kind_installer_serialization() {
    let kind = PayloadKind::Installer {
        installer_path: "/tmp/llmlb-setup.exe".to_string(),
        kind: InstallerKind::WindowsSetup,
    };
    let json = serde_json::to_value(&kind).unwrap();
    // Externally tagged: {"installer": {"installer_path": "...", "kind": "..."}}
    assert_eq!(json["installer"]["installer_path"], "/tmp/llmlb-setup.exe");
    assert_eq!(json["installer"]["kind"], "windows_setup");
}

// =======================================================================
// InstallerKind serialization
// =======================================================================
#[test]
fn installer_kind_serialization() {
    let mac = InstallerKind::MacPkg;
    let win = InstallerKind::WindowsSetup;
    assert_eq!(
        serde_json::to_value(&mac).unwrap(),
        serde_json::json!("mac_pkg")
    );
    assert_eq!(
        serde_json::to_value(&win).unwrap(),
        serde_json::json!("windows_setup")
    );
}

// =======================================================================
// ApplyMethod serialization
// =======================================================================
#[test]
fn apply_method_serialization() {
    assert_eq!(
        serde_json::to_value(&ApplyMethod::PortableReplace).unwrap(),
        serde_json::json!("portable_replace")
    );
    assert_eq!(
        serde_json::to_value(&ApplyMethod::MacPkg).unwrap(),
        serde_json::json!("mac_pkg")
    );
    assert_eq!(
        serde_json::to_value(&ApplyMethod::WindowsSetup).unwrap(),
        serde_json::json!("windows_setup")
    );
}
