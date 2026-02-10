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
    /// Installer payload path (.pkg/.msi)
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
    /// Windows `.msi` installer.
    #[value(name = "windows_msi")]
    WindowsMsi,
}

impl From<InstallerKindArg> for crate::update::InstallerKind {
    fn from(value: InstallerKindArg) -> Self {
        match value {
            InstallerKindArg::MacPkg => crate::update::InstallerKind::MacPkg,
            InstallerKindArg::WindowsMsi => crate::update::InstallerKind::WindowsMsi,
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
    }
}
