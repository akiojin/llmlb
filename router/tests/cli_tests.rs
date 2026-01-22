//! CLI integration tests
//!
//! Tests for command-line interface parsing and behavior.
//! The CLI only supports -h/--help and -V/--version flags.
//! All other operations are performed via API/Dashboard UI.

use clap::Parser;
use llmlb::cli::Cli;

/// T006: Test --version output contains version number
#[test]
fn test_version_available() {
    // Try parsing with --version should return error (because it prints and exits)
    let result = Cli::try_parse_from(["llmlb", "--version"]);
    // clap returns an error with kind DisplayVersion for --version
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
}

/// T005: Test --help is available
#[test]
fn test_help_available() {
    // Try parsing with --help should return error (because it prints and exits)
    let result = Cli::try_parse_from(["llmlb", "--help"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
}

/// Test no arguments (should start server)
#[test]
fn test_no_args_starts_server() {
    // Running without arguments should succeed and start the server
    let cli = Cli::try_parse_from(["llmlb"]);
    assert!(cli.is_ok());
}

/// Test short version flag
#[test]
fn test_short_version_flag() {
    let result = Cli::try_parse_from(["llmlb", "-V"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
}

/// Test short help flag
#[test]
fn test_short_help_flag() {
    let result = Cli::try_parse_from(["llmlb", "-h"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
}

/// Test unknown argument is rejected
#[test]
fn test_unknown_arg_rejected() {
    let result = Cli::try_parse_from(["llmlb", "--unknown"]);
    assert!(result.is_err());
}

/// Test subcommand is rejected (no subcommands available)
#[test]
fn test_subcommand_rejected() {
    let result = Cli::try_parse_from(["llmlb", "user"]);
    assert!(result.is_err());
}
