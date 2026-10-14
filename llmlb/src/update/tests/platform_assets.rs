use super::*;

#[test]
fn test_platform_asset_names() {
    let p = Platform {
        os: "linux".to_string(),
        arch: "x86_64".to_string(),
    };
    assert_eq!(
        p.portable_asset_name(),
        Some("llmlb-linux-x86_64.tar.gz".to_string())
    );
    assert_eq!(p.installer_asset_name(), None);

    let p = Platform {
        os: "windows".to_string(),
        arch: "x86_64".to_string(),
    };
    assert_eq!(
        p.portable_asset_name(),
        Some("llmlb-windows-x86_64.zip".to_string())
    );
    assert_eq!(
        p.installer_asset_name(),
        Some((
            "llmlb-windows-x86_64-setup.exe".to_string(),
            InstallerKind::WindowsSetup
        ))
    );
}

// =======================================================================
// Platform tests
// =======================================================================
#[test]
fn platform_detect_returns_current_os() {
    let p = Platform::detect().unwrap();
    assert_eq!(p.os, std::env::consts::OS);
    assert_eq!(p.arch, std::env::consts::ARCH);
}

#[test]
fn platform_artifact_linux_x86_64() {
    let p = Platform {
        os: "linux".to_string(),
        arch: "x86_64".to_string(),
    };
    assert_eq!(p.artifact(), Some("linux-x86_64"));
}

#[test]
fn platform_artifact_linux_arm64() {
    let p = Platform {
        os: "linux".to_string(),
        arch: "aarch64".to_string(),
    };
    assert_eq!(p.artifact(), Some("linux-arm64"));
}

#[test]
fn platform_artifact_macos_arm64() {
    let p = Platform {
        os: "macos".to_string(),
        arch: "aarch64".to_string(),
    };
    assert_eq!(p.artifact(), Some("macos-arm64"));
    assert_eq!(
        p.portable_asset_name(),
        Some("llmlb-macos-arm64.tar.gz".to_string())
    );
    assert_eq!(
        p.installer_asset_name(),
        Some(("llmlb-macos-arm64.pkg".to_string(), InstallerKind::MacPkg))
    );
}

#[test]
fn platform_artifact_macos_x86_64() {
    let p = Platform {
        os: "macos".to_string(),
        arch: "x86_64".to_string(),
    };
    assert_eq!(p.artifact(), Some("macos-x86_64"));
    assert_eq!(
        p.installer_asset_name(),
        Some(("llmlb-macos-x86_64.pkg".to_string(), InstallerKind::MacPkg))
    );
}

#[test]
fn platform_artifact_unknown() {
    let p = Platform {
        os: "freebsd".to_string(),
        arch: "x86_64".to_string(),
    };
    assert_eq!(p.artifact(), None);
    assert_eq!(p.portable_asset_name(), None);
    assert_eq!(p.installer_asset_name(), None);
}

#[test]
fn platform_binary_name_unix() {
    let p = Platform {
        os: "linux".to_string(),
        arch: "x86_64".to_string(),
    };
    assert_eq!(p.binary_name(), "llmlb");
}

#[test]
fn platform_binary_name_windows() {
    let p = Platform {
        os: "windows".to_string(),
        arch: "x86_64".to_string(),
    };
    assert_eq!(p.binary_name(), "llmlb.exe");
}

// =======================================================================
// select_assets
// =======================================================================
#[test]
fn select_assets_finds_matching_portable() {
    let release = GitHubRelease {
        tag_name: "v5.0.0".to_string(),
        html_url: "https://github.com/test/test/releases/v5.0.0".to_string(),
        assets: vec![
            GitHubAsset {
                name: "llmlb-linux-x86_64.tar.gz".to_string(),
                browser_download_url: "https://dl.example.com/llmlb-linux-x86_64.tar.gz"
                    .to_string(),
            },
            GitHubAsset {
                name: "llmlb-windows-x86_64.zip".to_string(),
                browser_download_url: "https://dl.example.com/llmlb-windows-x86_64.zip".to_string(),
            },
        ],
    };

    let platform = Platform {
        os: "linux".to_string(),
        arch: "x86_64".to_string(),
    };
    let (portable, installer) = select_assets(&release, &platform);
    assert!(portable.is_some());
    assert_eq!(portable.unwrap().name, "llmlb-linux-x86_64.tar.gz");
    assert!(installer.is_none()); // linux has no installer
}

#[test]
fn select_assets_finds_both_on_windows() {
    let release = GitHubRelease {
        tag_name: "v5.0.0".to_string(),
        html_url: "https://github.com/test/test/releases/v5.0.0".to_string(),
        assets: vec![
            GitHubAsset {
                name: "llmlb-windows-x86_64.zip".to_string(),
                browser_download_url: "https://dl.example.com/llmlb-windows-x86_64.zip".to_string(),
            },
            GitHubAsset {
                name: "llmlb-windows-x86_64-setup.exe".to_string(),
                browser_download_url: "https://dl.example.com/llmlb-windows-x86_64-setup.exe"
                    .to_string(),
            },
        ],
    };

    let platform = Platform {
        os: "windows".to_string(),
        arch: "x86_64".to_string(),
    };
    let (portable, installer) = select_assets(&release, &platform);
    assert!(portable.is_some());
    assert!(installer.is_some());
    assert_eq!(portable.unwrap().name, "llmlb-windows-x86_64.zip");
    assert_eq!(installer.unwrap().name, "llmlb-windows-x86_64-setup.exe");
}

#[test]
fn select_assets_returns_none_when_no_match() {
    let release = GitHubRelease {
        tag_name: "v5.0.0".to_string(),
        html_url: "https://github.com/test/test/releases/v5.0.0".to_string(),
        assets: vec![GitHubAsset {
            name: "llmlb-linux-x86_64.tar.gz".to_string(),
            browser_download_url: "https://dl.example.com/llmlb-linux-x86_64.tar.gz".to_string(),
        }],
    };

    let platform = Platform {
        os: "freebsd".to_string(),
        arch: "x86_64".to_string(),
    };
    let (portable, installer) = select_assets(&release, &platform);
    assert!(portable.is_none());
    assert!(installer.is_none());
}

// =======================================================================
// choose_apply_plan
// =======================================================================
#[test]
fn choose_apply_plan_prefers_portable_when_writable() {
    let dir = tempfile::tempdir().unwrap();
    let exe_path = dir.path().join("llmlb");
    fs::write(&exe_path, b"dummy").unwrap();

    let platform = Platform {
        os: "linux".to_string(),
        arch: "x86_64".to_string(),
    };
    let plan = choose_apply_plan(
        &platform,
        &exe_path,
        Some("https://example.com/portable.tar.gz"),
        None,
    );
    assert_eq!(
        plan,
        Some(ApplyPlan::Portable {
            url: "https://example.com/portable.tar.gz".to_string()
        })
    );
}

#[test]
fn choose_apply_plan_returns_none_when_no_urls() {
    let dir = tempfile::tempdir().unwrap();
    let exe_path = dir.path().join("llmlb");
    fs::write(&exe_path, b"dummy").unwrap();

    let platform = Platform {
        os: "linux".to_string(),
        arch: "x86_64".to_string(),
    };
    let plan = choose_apply_plan(&platform, &exe_path, None, None);
    assert!(plan.is_none());
}

#[test]
fn choose_apply_plan_falls_back_to_installer_when_writable() {
    let dir = tempfile::tempdir().unwrap();
    let exe_path = dir.path().join("llmlb");
    fs::write(&exe_path, b"dummy").unwrap();

    let platform = Platform {
        os: "macos".to_string(),
        arch: "aarch64".to_string(),
    };
    let plan = choose_apply_plan(
        &platform,
        &exe_path,
        None,
        Some("https://example.com/installer.pkg"),
    );
    assert!(matches!(plan, Some(ApplyPlan::Installer { .. })));
}

// =======================================================================
// is_dir_writable
// =======================================================================
#[test]
fn is_dir_writable_temp_dir() {
    let dir = tempfile::tempdir().unwrap();
    assert!(is_dir_writable(dir.path()).unwrap());
}

#[test]
fn is_dir_writable_concurrent_calls_all_true() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(16));
    let handles: Vec<_> = (0..16)
        .map(|_| {
            let path = path.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                (0..50)
                    .map(|_| is_dir_writable(&path).map_err(|e| e.to_string()))
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    for handle in handles {
        for result in handle.join().unwrap() {
            assert_eq!(result, Ok(true));
        }
    }
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn is_dir_writable_ignores_leftover_probe() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".llmlb_write_probe"), b"").unwrap();
    assert!(is_dir_writable(dir.path()).unwrap());
    assert!(is_dir_writable(dir.path()).unwrap());
    // 事前に存在したファイル以外（今回のプローブ）は残さない
    let names: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, vec![std::ffi::OsString::from(".llmlb_write_probe")]);
}

#[test]
fn is_dir_writable_propagates_non_permission_errors() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("not-a-dir");
    fs::write(&file, b"").unwrap();
    assert!(is_dir_writable(&file).is_err());
}

#[cfg(unix)]
#[test]
fn is_dir_writable_read_only_dir_is_false() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();
    // root は権限ビットを無視して書き込めるため、実際に書き込めない環境でのみ検証する
    let really_read_only = fs::write(dir.path().join("check"), b"").is_err();
    let result = is_dir_writable(dir.path());
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
    if really_read_only {
        assert!(!result.unwrap());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}
