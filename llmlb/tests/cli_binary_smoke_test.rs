use std::process::Command;

fn bin_path() -> &'static str {
    env!("CARGO_BIN_EXE_llmlb")
}

fn unique_test_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("failed to reserve test port");
    let port = listener
        .local_addr()
        .expect("failed to read test port")
        .port();
    drop(listener);
    port
}

#[test]
fn status_subcommand_succeeds_for_unused_port() {
    let port = unique_test_port();
    let output = Command::new(bin_path())
        .args(["status", "--port", &port.to_string()])
        .output()
        .expect("failed to run llmlb status");

    assert!(
        output.status.success(),
        "status should exit successfully for unused port"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("No server running on port") || stdout.contains("No active server on port"),
        "unexpected status stdout: {stdout}"
    );
}

#[test]
fn stop_subcommand_succeeds_for_unused_port() {
    let port = unique_test_port();
    let output = Command::new(bin_path())
        .args(["stop", "--port", &port.to_string(), "--timeout", "1"])
        .output()
        .expect("failed to run llmlb stop");

    assert!(
        output.status.success(),
        "stop should exit successfully when no server is running"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("No server running on port"),
        "unexpected stop stdout: {stdout}"
    );
}

#[test]
fn internal_rollback_subcommand_fails_without_backup() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let nonexistent_pid = (u32::MAX - 1).to_string();
    let target = temp_dir.path().join("llmlb-target");
    let backup = temp_dir.path().join("missing.bak");
    let args_file = temp_dir.path().join("args.json");
    std::fs::write(&args_file, r#"{"args":[],"cwd":""}"#).expect("failed to write args file");

    let output = Command::new(bin_path())
        .args([
            "__internal",
            "rollback",
            "--old-pid",
            nonexistent_pid.as_str(),
            "--target",
            target.to_string_lossy().as_ref(),
            "--backup",
            backup.to_string_lossy().as_ref(),
            "--args-file",
            args_file.to_string_lossy().as_ref(),
        ])
        .output()
        .expect("failed to run llmlb __internal rollback");

    assert!(
        !output.status.success(),
        "rollback should fail when backup does not exist"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Backup file does not exist"),
        "unexpected rollback stderr: {stderr}"
    );
}
