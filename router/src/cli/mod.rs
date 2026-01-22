//! CLI module for llmlb
//!
//! Provides command-line interface for router management.
//! All operations are performed via API/Dashboard UI.

use clap::Parser;

/// LLM Router - Centralized management system for LLM inference nodes
#[derive(Parser, Debug)]
#[command(name = "llmlb")]
#[command(version, about, long_about = None)]
#[command(after_help = r#"ENVIRONMENT VARIABLES:
    LLMLB_HOST              Bind address (default: 0.0.0.0)
    LLMLB_PORT              Listen port (default: 32768)
    LLMLB_LOG_LEVEL         Log level (default: info)
    LLMLB_DATABASE_URL      Database URL
    LLMLB_JWT_SECRET        JWT signing key (auto-generated if not set)
    LLMLB_ADMIN_USERNAME    Initial admin username (default: admin)
    LLMLB_ADMIN_PASSWORD    Initial admin password (required on first run)
"#)]
pub struct Cli;
