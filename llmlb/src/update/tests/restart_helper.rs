use super::*;

// =======================================================================
// T260: ヘルパー起動監視 — .bakから復元ロジックのテスト
// =======================================================================
#[test]
fn internal_rollback_restores_backup() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("llmlb");
    let backup = dir.path().join("llmlb.bak");
    let args_file = dir.path().join("restart_args.json");

    // Simulate a freshly updated target that rollback must replace.
    fs::write(&target, b"new-binary-content").unwrap();
    // Create a fake "old" binary.
    fs::write(&backup, b"old-binary-content").unwrap();
    // Create a fake args file (needed for restart_from_args_file).
    let args = RestartArgsFile {
        args: vec![],
        cwd: dir.path().to_string_lossy().to_string(),
    };
    fs::write(&args_file, serde_json::to_vec(&args).unwrap()).unwrap();

    // internal_rollback expects the old process to have exited.
    // Using PID 0 or a non-existent PID: use current PID which is alive.
    // Instead, use PID 1 which is always running on Unix — let's use a non-existent PID.
    // PID u32::MAX is unlikely to exist.
    let result = internal_rollback(u32::MAX, target.clone(), backup.clone(), args_file);

    // The rollback should have restored the backup to the target path.
    assert!(target.exists(), "target should be restored from backup");
    assert!(!backup.exists(), "backup should be consumed (renamed)");
    let content = fs::read(&target).unwrap();
    assert_eq!(content, b"old-binary-content");

    // The restart_from_args_file call will fail because the target is not
    // executable, but the backup restoration should have succeeded.
    // We check if the result is Err (from failed spawn) but not from rollback.
    if let Err(e) = result {
        // Expected: spawn failure because we wrote fake content, not a real binary.
        assert!(
            !e.to_string().contains("Backup file does not exist"),
            "should not fail due to missing backup: {e}"
        );
    }
}

#[test]
fn internal_rollback_fails_without_backup() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("llmlb");
    let backup = dir.path().join("llmlb.bak");
    let args_file = dir.path().join("restart_args.json");

    let result = internal_rollback(u32::MAX, target, backup, args_file);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Backup file does not exist"));
}

// =======================================================================
// T262: ロールバック結果の update-history.json 記録
// =======================================================================
#[test]
fn record_auto_rollback_history_writes_entry() {
    let dir = tempfile::tempdir().unwrap();
    // Create directory structure: data_dir/updates/rollback-X.Y.Z/restart_args.json
    let updates_dir = dir.path().join("updates").join("rollback-test");
    fs::create_dir_all(&updates_dir).unwrap();
    let args_file = updates_dir.join("restart_args.json");
    fs::write(&args_file, "{}").unwrap();

    record_auto_rollback_history(&args_file, "health check failed");

    let store = history::HistoryStore::new(dir.path());
    let entries = store.load().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].kind, history::HistoryEventKind::Rollback);
    assert!(entries[0]
        .message
        .as_ref()
        .unwrap()
        .contains("health check failed"));
}

#[test]
fn detect_server_port_reads_restart_args_file() {
    let dir = tempfile::tempdir().unwrap();
    let args_file = dir.path().join("restart_args.json");
    let args = RestartArgsFile {
        args: vec![
            "serve".to_string(),
            "--host".to_string(),
            "127.0.0.1".to_string(),
            "--port".to_string(),
            "40123".to_string(),
        ],
        cwd: dir.path().to_string_lossy().to_string(),
    };
    fs::write(&args_file, serde_json::to_vec(&args).unwrap()).unwrap();

    assert_eq!(detect_server_port(&args_file), 40123);
}

#[test]
fn parse_port_from_args_supports_equals_style() {
    let args = vec!["serve".to_string(), "--port=40124".to_string()];
    assert_eq!(parse_port_from_args(&args), Some(40124));
}

// =======================================================================
// RestartArgsFile serialization
// =======================================================================
#[test]
fn restart_args_file_roundtrip() {
    let raf = RestartArgsFile {
        args: vec![
            "serve".to_string(),
            "--port".to_string(),
            "8080".to_string(),
        ],
        cwd: "/home/user".to_string(),
    };
    let json = serde_json::to_string(&raf).unwrap();
    let deserialized: RestartArgsFile = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.args, raf.args);
    assert_eq!(deserialized.cwd, raf.cwd);
}

#[test]
fn restart_args_file_empty_args() {
    let raf = RestartArgsFile {
        args: vec![],
        cwd: ".".to_string(),
    };
    let json = serde_json::to_string(&raf).unwrap();
    let deserialized: RestartArgsFile = serde_json::from_str(&json).unwrap();
    assert!(deserialized.args.is_empty());
}

// =======================================================================
// write_restart_args_file
// =======================================================================
#[test]
fn write_restart_args_file_creates_file() {
    let dir = tempfile::tempdir().unwrap();
    let update_dir = dir.path().join("updates").join("5.0.0");
    let result = write_restart_args_file(&update_dir);
    assert!(result.is_ok());
    let path = result.unwrap();
    assert!(path.exists());
    assert_eq!(path.file_name().unwrap(), "restart_args.json");

    // Verify content is valid JSON
    let content = fs::read_to_string(&path).unwrap();
    let parsed: RestartArgsFile = serde_json::from_str(&content).unwrap();
    assert!(!parsed.cwd.is_empty());
}

// =======================================================================
// parse_port_from_args
// =======================================================================
#[test]
fn parse_port_from_args_flag_style() {
    let args = vec![
        "serve".to_string(),
        "--port".to_string(),
        "9090".to_string(),
    ];
    assert_eq!(parse_port_from_args(&args), Some(9090));
}

#[test]
fn parse_port_from_args_short_flag() {
    let args = vec!["serve".to_string(), "-p".to_string(), "9090".to_string()];
    assert_eq!(parse_port_from_args(&args), Some(9090));
}

#[test]
fn parse_port_from_args_equals_style() {
    let args = vec!["serve".to_string(), "--port=12345".to_string()];
    assert_eq!(parse_port_from_args(&args), Some(12345));
}

#[test]
fn parse_port_from_args_no_port() {
    let args = vec![
        "serve".to_string(),
        "--host".to_string(),
        "0.0.0.0".to_string(),
    ];
    assert_eq!(parse_port_from_args(&args), None);
}

#[test]
fn parse_port_from_args_empty() {
    let args: Vec<String> = vec![];
    assert_eq!(parse_port_from_args(&args), None);
}

#[test]
fn parse_port_from_args_invalid_port_value() {
    let args = vec![
        "serve".to_string(),
        "--port".to_string(),
        "not_a_number".to_string(),
    ];
    assert_eq!(parse_port_from_args(&args), None);
}

#[test]
fn parse_port_from_args_port_at_end_without_value() {
    let args = vec!["serve".to_string(), "--port".to_string()];
    assert_eq!(parse_port_from_args(&args), None);
}

// =======================================================================
// detect_server_port
// =======================================================================
#[test]
fn detect_server_port_falls_back_to_default() {
    let dir = tempfile::tempdir().unwrap();
    let nonexistent = dir.path().join("nonexistent.json");
    let port = detect_server_port(&nonexistent);
    assert_eq!(port, DEFAULT_LISTEN_PORT);
}

#[test]
fn detect_server_port_from_args_file_reads_port() {
    let dir = tempfile::tempdir().unwrap();
    let args_file = dir.path().join("restart_args.json");
    let args = RestartArgsFile {
        args: vec!["serve".to_string(), "-p".to_string(), "55555".to_string()],
        cwd: dir.path().to_string_lossy().to_string(),
    };
    fs::write(&args_file, serde_json::to_vec(&args).unwrap()).unwrap();
    assert_eq!(detect_server_port(&args_file), 55555);
}
