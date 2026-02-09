//! CLI integration tests
//!
//! Tests for command-line interface parsing and behavior.
//! The CLI supports a small set of management subcommands (serve/status/stop) plus basic flags.
//! Day-to-day configuration is done via the Dashboard UI (`/dashboard`) or HTTP APIs.

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

/// Test serve subcommand parses
#[test]
fn test_serve_subcommand_parses() {
    let cli = Cli::try_parse_from(["llmlb", "serve", "--no-tray"]);
    assert!(cli.is_ok());
}

/// Test status subcommand parses
#[test]
fn test_status_subcommand_parses() {
    let cli = Cli::try_parse_from(["llmlb", "status"]);
    assert!(cli.is_ok());
}

/// Test stop subcommand parses
#[test]
fn test_stop_subcommand_parses() {
    let cli = Cli::try_parse_from(["llmlb", "stop", "--port", "32768"]);
    assert!(cli.is_ok());
}

/// Test unknown argument is rejected
#[test]
fn test_unknown_arg_rejected() {
    let result = Cli::try_parse_from(["llmlb", "--unknown"]);
    assert!(result.is_err());
}

/// Test unknown subcommand is rejected
#[test]
fn test_unknown_subcommand_rejected() {
    let result = Cli::try_parse_from(["llmlb", "user"]);
    assert!(result.is_err());
}
