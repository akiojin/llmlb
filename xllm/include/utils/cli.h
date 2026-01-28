#pragma once

#include <cstdint>
#include <optional>
#include <string>
#include <vector>

namespace xllm {

/// Subcommand types for xllm CLI
enum class Subcommand {
    None,           // No subcommand (legacy server mode)
    Serve,          // serve
    Run,            // run <model>
    Pull,           // pull <model>
    List,           // list
    Show,           // show <model>
    Rm,             // rm <model>
    Stop,           // stop <model>
    Ps,             // ps
    Profile,        // profile <model>
    Benchmark,      // benchmark <model>
    Compare,        // compare <model_a> <model_b>
    Convert,        // convert <src> --name <model>
    Export,         // export <model> --output <file>
    Import,         // import <model> --file <file>
};

/// Options for serve command
struct ServeOptions {
    uint16_t port{32769};
    std::string host{"0.0.0.0"};
    std::string model;       // Path to model file (e.g., --model path/to/model.gguf)
    std::string model_name;  // Model name/identifier (e.g., --model-name llava-v1.5-7b)
    std::string mmproj;      // Path to multimodal projector (e.g., --mmproj path/to/mmproj.gguf)
    int ctx_size{2048};      // Context size (e.g., --ctx-size 4096)
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

struct ProfileOptions {
    std::string model;
    std::string prompt{"Hello"};
    int max_tokens{128};
};

struct BenchmarkOptions {
    std::string model;
    std::string prompt{"Hello"};
    int max_tokens{128};
    int runs{3};
};

struct CompareOptions {
    std::string model_a;
    std::string model_b;
    std::string prompt{"Hello"};
    int max_tokens{128};
    int runs{3};
};

struct ConvertOptions {
    std::string source;
    std::string name;
    std::string format{"gguf"};
};

struct ExportOptions {
    std::string model;
    std::string output;
};

struct ImportOptions {
    std::string model;
    std::string file;
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

    ProfileOptions profile_options;
    BenchmarkOptions benchmark_options;
    CompareOptions compare_options;
    ConvertOptions convert_options;
    ExportOptions export_options;
    ImportOptions import_options;
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

/// Convert subcommand enum to string
std::string subcommandToString(Subcommand cmd);

}  // namespace xllm
