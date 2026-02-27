//! Internal helper commands.
//!
//! This command tree is intentionally hidden from `--help`.
//! It's used by the self-update mechanism to:
//! - replace the running executable safely
//! - run OS installers and restart the server

use anyhow::Result;
use clap::{Args, Subcommand, ValueEnum};

/// Arguments for the hidden `__internal` command tree used by self-update helpers.
#[derive(Args, Debug)]
pub struct InternalArgs {
    #[command(subcommand)]
    /// Subcommand to execute.
    pub command: InternalCommand,
}

/// Internal helper commands invoked by the main process during self-update.
#[derive(Subcommand, Debug)]
pub enum InternalCommand {
    /// Replace the running executable and restart.
    ApplyUpdate(ApplyUpdateArgs),
    /// Run an installer (pkg/msi) and restart.
    RunInstaller(RunInstallerArgs),
    /// Rollback to the previous version from `.bak`.
    Rollback(RollbackArgs),
}

/// Arguments for `__internal apply-update`.
#[derive(Args, Debug)]
pub struct ApplyUpdateArgs {
    /// PID of the process that is being replaced
    #[arg(long)]
    pub old_pid: u32,
    /// Current executable path (to be replaced)
    #[arg(long)]
    pub target: std::path::PathBuf,
    /// New executable path (extracted)
    #[arg(long)]
    pub new_binary: std::path::PathBuf,
    /// Restart args file written by the main process
    #[arg(long)]
    pub args_file: std::path::PathBuf,
}

/// Arguments for `__internal run-installer`.
#[derive(Args, Debug)]
pub struct RunInstallerArgs {
    /// PID of the process that is being replaced
    #[arg(long)]
    pub old_pid: u32,
    /// Current executable path (used for restart)
    #[arg(long)]
    pub target: std::path::PathBuf,
    /// Installer payload path (.pkg/.exe)
    #[arg(long)]
    pub installer: std::path::PathBuf,
    /// Installer kind
    #[arg(long, value_enum)]
    pub installer_kind: InstallerKindArg,
    /// Restart args file written by the main process
    #[arg(long)]
    pub args_file: std::path::PathBuf,
}

/// Installer kind for `__internal run-installer`.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum InstallerKindArg {
    /// macOS `.pkg` installer.
    #[value(name = "mac_pkg")]
    MacPkg,
    /// Windows setup `.exe` installer.
    #[value(name = "windows_setup")]
    WindowsSetup,
}

/// Arguments for `__internal rollback`.
#[derive(Args, Debug)]
pub struct RollbackArgs {
    /// PID of the process that is being rolled back
    #[arg(long)]
    pub old_pid: u32,
    /// Current executable path (to be restored)
    #[arg(long)]
    pub target: std::path::PathBuf,
    /// Backup executable path (.bak)
    #[arg(long)]
    pub backup: std::path::PathBuf,
    /// Restart args file written by the main process
    #[arg(long)]
    pub args_file: std::path::PathBuf,
}

impl From<InstallerKindArg> for crate::update::InstallerKind {
    fn from(value: InstallerKindArg) -> Self {
        match value {
            InstallerKindArg::MacPkg => crate::update::InstallerKind::MacPkg,
            InstallerKindArg::WindowsSetup => crate::update::InstallerKind::WindowsSetup,
        }
    }
}

/// Execute an internal helper command.
pub fn execute(command: InternalCommand) -> Result<()> {
    match command {
        InternalCommand::ApplyUpdate(args) => crate::update::internal_apply_update(
            args.old_pid,
            args.target,
            args.new_binary,
            args.args_file,
        ),
        InternalCommand::RunInstaller(args) => crate::update::internal_run_installer(
            args.old_pid,
            args.target,
            args.installer,
            args.installer_kind.into(),
            args.args_file,
        ),
        InternalCommand::Rollback(args) => {
            crate::update::internal_rollback(args.old_pid, args.target, args.backup, args.args_file)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installer_kind_conversion_matches_update_kind() {
        assert!(matches!(
            crate::update::InstallerKind::from(InstallerKindArg::MacPkg),
            crate::update::InstallerKind::MacPkg
        ));
        assert!(matches!(
            crate::update::InstallerKind::from(InstallerKindArg::WindowsSetup),
            crate::update::InstallerKind::WindowsSetup
        ));
    }

    #[test]
    fn execute_apply_update_returns_error_when_new_binary_missing() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let target = dir.path().join("llmlb-target");
        let new_binary = dir.path().join("missing-new-binary");
        let args_file = dir.path().join("args.json");
        std::fs::write(&args_file, r#"{"args":[],"cwd":""}"#).expect("failed to write args file");

        let err = execute(InternalCommand::ApplyUpdate(ApplyUpdateArgs {
            old_pid: 0,
            target,
            new_binary,
            args_file,
        }))
        .expect_err("apply-update should fail when new binary is missing");

        assert!(err
            .to_string()
            .contains("Failed to replace target executable"));
    }

    #[test]
    fn execute_rollback_returns_error_when_backup_missing() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let target = dir.path().join("llmlb-target");
        let backup = dir.path().join("missing.bak");
        let args_file = dir.path().join("args.json");
        std::fs::write(&args_file, r#"{"args":[],"cwd":""}"#).expect("failed to write args file");

        let err = execute(InternalCommand::Rollback(RollbackArgs {
            old_pid: 0,
            target,
            backup,
            args_file,
        }))
        .expect_err("rollback should fail when backup does not exist");

        assert!(err.to_string().contains("Backup file does not exist"));
    }

    #[test]
    fn execute_run_installer_returns_error_on_unsupported_flow() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let target = dir.path().join("llmlb-target");
        let installer = dir.path().join("installer.exe");
        let args_file = dir.path().join("args.json");
        std::fs::write(&args_file, r#"{"args":[],"cwd":""}"#).expect("failed to write args file");

        let result = execute(InternalCommand::RunInstaller(RunInstallerArgs {
            old_pid: 0,
            target,
            installer,
            installer_kind: InstallerKindArg::WindowsSetup,
            args_file,
        }));

        assert!(
            result.is_err(),
            "run-installer should fail on this test path"
        );
    }
}
