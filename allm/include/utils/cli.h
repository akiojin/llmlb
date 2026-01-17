#pragma once

#include <cstdint>
#include <optional>
#include <string>
#include <vector>

namespace llm_node {

/// Subcommand types for llm-router CLI
enum class Subcommand {
    None,           // No subcommand (legacy server mode)
    // Direct commands (formerly node subcommands)
    Serve,          // serve
    Run,            // run <model>
    Pull,           // pull <model>
    List,           // list
    Show,           // show <model>
    Rm,             // rm <model>
    Stop,           // stop <model>
    Ps,             // ps
    // Router subcommands
    RouterEndpoints,  // router endpoints
    RouterModels,     // router models
    RouterStatus,     // router status
};

/// Options for serve command
struct ServeOptions {
    uint16_t port{32769};
    std::string host{"0.0.0.0"};
};

/// Options for run command
struct RunOptions {
    std::string model;
    bool show_thinking{false};
    bool hide_thinking{true};
};

/// Options for pull command
struct PullOptions {
    std::string model;
};

/// Options for show command
struct ShowOptions {
    std::string model;
    bool license_only{false};
    bool parameters_only{false};
    bool modelfile_only{false};
    bool template_only{false};
    bool system_only{false};
};

/// Options for model-related commands (rm, stop)
struct ModelOptions {
    std::string model;
};

/// Result of CLI argument parsing
struct CliResult {
    /// Whether the program should exit immediately (e.g., after --help or --version)
    bool should_exit{false};

    /// Exit code to use if should_exit is true
    int exit_code{0};

    /// Output message to display (help text, version info, or error message)
    std::string output;

    /// Parsed subcommand
    Subcommand subcommand{Subcommand::None};

    /// Options for serve command
    ServeOptions serve_options;

    /// Options for run command
    RunOptions run_options;

    /// Options for pull command
    PullOptions pull_options;

    /// Options for show command
    ShowOptions show_options;

    /// Options for model commands (rm, stop)
    ModelOptions model_options;
};

/// Parse command line arguments
///
/// @param argc Number of arguments
/// @param argv Argument values
/// @return CliResult indicating whether to continue or exit
CliResult parseCliArgs(int argc, char* argv[]);

/// Get the help message for the CLI
///
/// @return Help message string
std::string getHelpMessage();

/// Get the version message for the CLI
///
/// @return Version message string
std::string getVersionMessage();

/// Get help message for router subcommands
std::string getRouterHelpMessage();

/// Convert subcommand enum to string
std::string subcommandToString(Subcommand cmd);

}  // namespace llm_node
