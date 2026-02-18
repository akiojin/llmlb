//! CLI module for llmlb
//!
//! Provides command-line interface for load balancer management.

pub mod assistant;
pub mod internal;
pub mod serve;
pub mod status;
pub mod stop;

use clap::{Parser, Subcommand};

/// LLM load balancer - Centralized management system for LLM inference nodes
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
    LLMLB_DEFAULT_EMBEDDING_MODEL  Default embedding model
    LLMLB_AUTH_DISABLED     Disable auth checks (dev/test only)
"#)]
pub struct Cli {
    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Available subcommands
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the load balancer server
    Serve(serve::ServeArgs),
    /// Stop a running server
    Stop(stop::StopArgs),
    /// Show status of running servers
    Status(status::StatusArgs),
    /// Assistant helper commands (MCP replacement)
    Assistant(assistant::AssistantArgs),

    /// Internal helper commands (self-update)
    #[command(name = "__internal", hide = true)]
    Internal(internal::InternalArgs),
}
