// SPEC-58378000: CLI argument parser with ollama-compatible subcommands
#include "utils/cli.h"
#include "utils/version.h"
#include <sstream>
#include <cstring>
#include <cstdlib>
#include <vector>

namespace xllm {

// Forward declarations for help messages
std::string getServeHelpMessage();
std::string getRunHelpMessage();
std::string getPullHelpMessage();
std::string getListHelpMessage();
std::string getShowHelpMessage();
std::string getRmHelpMessage();
std::string getStopHelpMessage();
std::string getPsHelpMessage();
std::string getProfileHelpMessage();
std::string getBenchmarkHelpMessage();
std::string getCompareHelpMessage();
std::string getConvertHelpMessage();
std::string getExportHelpMessage();
std::string getImportHelpMessage();

std::string getHelpMessage() {
    std::ostringstream oss;
    oss << "xllm " << XLLM_VERSION << " - LLM inference engine\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    xllm <COMMAND>\n";
    oss << "\n";
    oss << "COMMANDS:\n";
    oss << "    serve      Start the server (foreground)\n";
    oss << "    run        Chat with a model (REPL)\n";
    oss << "    pull       Download a model from HuggingFace\n";
    oss << "    list       List local models\n";
    oss << "    show       Show model metadata\n";
    oss << "    rm         Delete a model\n";
    oss << "    stop       Unload a running model\n";
    oss << "    ps         List running models\n";
    oss << "    profile    Profile a model (latency/tokens)\n";
    oss << "    benchmark  Benchmark a model\n";
    oss << "    compare    Compare two models\n";
    oss << "    convert    Convert model format\n";
    oss << "    export     Export model metadata\n";
    oss << "    import     Import model metadata\n";
    oss << "\n";
    oss << "OPTIONS:\n";
    oss << "    -h, --help       Print help information\n";
    oss << "    -V, --version    Print version information\n";
    oss << "\n";
    oss << "Run 'xllm <COMMAND> --help' for more info.\n";
    return oss.str();
}

std::string getServeHelpMessage() {
    std::ostringstream oss;
    oss << "xllm serve - Start the server\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    xllm serve [OPTIONS]\n";
    oss << "\n";
    oss << "OPTIONS:\n";
    oss << "    --port <PORT>         Server port (default: 32769, or XLLM_PORT)\n";
    oss << "    --host <HOST>         Bind address (default: 0.0.0.0)\n";
    oss << "    --model <PATH>        Path to model file (e.g., model.gguf)\n";
    oss << "    --model-name <NAME>   Model name/identifier (default: filename stem)\n";
    oss << "    --mmproj <PATH>       Path to multimodal projector for vision models\n";
    oss << "    --ctx-size <SIZE>     Context size (default: 2048)\n";
    oss << "    -h, --help            Print help\n";
    oss << "\n";
    oss << "ENVIRONMENT VARIABLES:\n";
    oss << "    XLLM_PORT                 HTTP server port (default: 32769)\n";
    oss << "    XLLM_BIND_ADDRESS         Bind address\n";
    oss << "    XLLM_MODELS_DIR           Model files directory\n";
    oss << "    XLLM_CONFIG               Config file path (default: ~/.xllm/config.json)\n";
    oss << "    XLLM_LOG_LEVEL            Log level (trace|debug|info|warn|error)\n";
    oss << "    XLLM_LOG_DIR              Log directory (default: ~/.xllm/logs)\n";
    oss << "    XLLM_LOG_RETENTION_DAYS   Log retention days (default: 7)\n";
    oss << "    XLLM_PGP_VERIFY           Verify HuggingFace PGP signatures (default: false)\n";
    oss << "    LLMLB_HOST                Router URL for registration\n";
    oss << "    LLMLB_DEBUG               Enable debug logging\n";
    oss << "    HF_TOKEN                  HuggingFace API token (for gated models)\n";
    return oss.str();
}

std::string getRunHelpMessage() {
    std::ostringstream oss;
    oss << "xllm run - Chat with a model\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    xllm run <MODEL> [OPTIONS]\n";
    oss << "\n";
    oss << "ARGUMENTS:\n";
    oss << "    <MODEL>          Model name (e.g., llama3.2, ollama:mistral)\n";
    oss << "\n";
    oss << "OPTIONS:\n";
    oss << "    --think          Show reasoning output (for deepseek-r1 etc.)\n";
    oss << "    --hide-think     Hide reasoning output (default)\n";
    oss << "    -h, --help       Print help\n";
    oss << "\n";
    oss << "REPL COMMANDS:\n";
    oss << "    /bye             Exit the session\n";
    oss << "    /clear           Clear conversation history\n";
    return oss.str();
}

std::string getPullHelpMessage() {
    std::ostringstream oss;
    oss << "xllm pull - Download a model\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    xllm pull <MODEL>\n";
    oss << "\n";
    oss << "ARGUMENTS:\n";
    oss << "    <MODEL>          Model name or HuggingFace URL\n";
    oss << "                     Examples: Qwen/Qwen2.5-0.5B-GGUF\n";
    oss << "                              https://huggingface.co/...\n";
    oss << "\n";
    oss << "OPTIONS:\n";
    oss << "    -h, --help       Print help\n";
    oss << "\n";
    oss << "ENVIRONMENT:\n";
    oss << "    HF_TOKEN         HuggingFace token (required for gated models)\n";
    return oss.str();
}

std::string getListHelpMessage() {
    std::ostringstream oss;
    oss << "xllm list - List local models\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    xllm list\n";
    oss << "\n";
    oss << "OPTIONS:\n";
    oss << "    -h, --help       Print help\n";
    oss << "\n";
    oss << "Shows models from:\n";
    oss << "    - xllm models directory\n";
    oss << "    - ollama models (~/.ollama/models/) with 'ollama:' prefix\n";
    return oss.str();
}

std::string getShowHelpMessage() {
    std::ostringstream oss;
    oss << "xllm show - Show model metadata\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    xllm show <MODEL> [OPTIONS]\n";
    oss << "\n";
    oss << "ARGUMENTS:\n";
    oss << "    <MODEL>          Model name\n";
    oss << "\n";
    oss << "OPTIONS:\n";
    oss << "    --license        Show license only\n";
    oss << "    --modelfile      Show modelfile only\n";
    oss << "    --parameters     Show parameters only\n";
    oss << "    --template       Show template only\n";
    oss << "    --system         Show system prompt only\n";
    oss << "    -h, --help       Print help\n";
    return oss.str();
}

std::string getRmHelpMessage() {
    std::ostringstream oss;
    oss << "xllm rm - Delete a model\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    xllm rm <MODEL>\n";
    oss << "\n";
    oss << "ARGUMENTS:\n";
    oss << "    <MODEL>          Model name to delete\n";
    oss << "\n";
    oss << "OPTIONS:\n";
    oss << "    -h, --help       Print help\n";
    oss << "\n";
    oss << "NOTE: ollama: prefixed models cannot be deleted (read-only)\n";
    return oss.str();
}

std::string getStopHelpMessage() {
    std::ostringstream oss;
    oss << "xllm stop - Unload a running model\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    xllm stop <MODEL>\n";
    oss << "\n";
    oss << "ARGUMENTS:\n";
    oss << "    <MODEL>          Model name to stop\n";
    oss << "\n";
    oss << "OPTIONS:\n";
    oss << "    -h, --help       Print help\n";
    return oss.str();
}

std::string getPsHelpMessage() {
    std::ostringstream oss;
    oss << "xllm ps - List running models\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    xllm ps\n";
    oss << "\n";
    oss << "OPTIONS:\n";
    oss << "    -h, --help       Print help\n";
    oss << "\n";
    oss << "COLUMNS:\n";
    oss << "    NAME, ID, SIZE, PROCESSOR, VRAM, TEMP, REQS, UNTIL\n";
    return oss.str();
}

std::string getProfileHelpMessage() {
    std::ostringstream oss;
    oss << "xllm profile - Profile a model\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    xllm profile <MODEL> [OPTIONS]\n";
    oss << "\n";
    oss << "OPTIONS:\n";
    oss << "    --prompt <TEXT>   Prompt to send (default: Hello)\n";
    oss << "    --tokens <N>      Max tokens to generate (default: 128)\n";
    oss << "    -h, --help        Print help\n";
    return oss.str();
}

std::string getBenchmarkHelpMessage() {
    std::ostringstream oss;
    oss << "xllm benchmark - Benchmark a model\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    xllm benchmark <MODEL> [OPTIONS]\n";
    oss << "\n";
    oss << "OPTIONS:\n";
    oss << "    --prompt <TEXT>   Prompt to send (default: Hello)\n";
    oss << "    --tokens <N>      Max tokens to generate (default: 128)\n";
    oss << "    --runs <N>        Number of runs (default: 3)\n";
    oss << "    -h, --help        Print help\n";
    return oss.str();
}

std::string getCompareHelpMessage() {
    std::ostringstream oss;
    oss << "xllm compare - Compare two models\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    xllm compare <MODEL_A> <MODEL_B> [OPTIONS]\n";
    oss << "\n";
    oss << "OPTIONS:\n";
    oss << "    --prompt <TEXT>   Prompt to send (default: Hello)\n";
    oss << "    --tokens <N>      Max tokens to generate (default: 128)\n";
    oss << "    --runs <N>        Number of runs (default: 3)\n";
    oss << "    -h, --help        Print help\n";
    return oss.str();
}

std::string getConvertHelpMessage() {
    std::ostringstream oss;
    oss << "xllm convert - Convert model format\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    xllm convert <SOURCE> --name <MODEL> [OPTIONS]\n";
    oss << "\n";
    oss << "OPTIONS:\n";
    oss << "    --name <MODEL>    Output model name\n";
    oss << "    --format <FMT>    Output format (default: gguf)\n";
    oss << "    -h, --help        Print help\n";
    return oss.str();
}

std::string getExportHelpMessage() {
    std::ostringstream oss;
    oss << "xllm export - Export model metadata\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    xllm export <MODEL> --output <FILE>\n";
    oss << "\n";
    oss << "OPTIONS:\n";
    oss << "    --output <FILE>   Output file path\n";
    oss << "    -h, --help        Print help\n";
    return oss.str();
}

std::string getImportHelpMessage() {
    std::ostringstream oss;
    oss << "xllm import - Import model metadata\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    xllm import <MODEL> --file <FILE>\n";
    oss << "\n";
    oss << "OPTIONS:\n";
    oss << "    --file <FILE>     Modelfile path to import\n";
    oss << "    -h, --help        Print help\n";
    return oss.str();
}

std::string getVersionMessage() {
    std::ostringstream oss;
    oss << "xllm " << XLLM_VERSION << "\n";
    return oss.str();
}

// Helper to check for help flag in arguments
bool hasHelpFlag(int argc, char* argv[], int start) {
    for (int i = start; i < argc; ++i) {
        if (std::strcmp(argv[i], "-h") == 0 || std::strcmp(argv[i], "--help") == 0) {
            return true;
        }
    }
    return false;
}

CliResult parseCliArgs(int argc, char* argv[]) {
    CliResult result;

    // No arguments - show help
    if (argc < 2) {
        result.should_exit = false;
        result.subcommand = Subcommand::None;
        return result;
    }

    const char* command = argv[1];

    // Global help and version
    if (std::strcmp(command, "-h") == 0 || std::strcmp(command, "--help") == 0) {
        result.should_exit = true;
        result.exit_code = 0;
        result.output = getHelpMessage();
        return result;
    }

    if (std::strcmp(command, "-V") == 0 || std::strcmp(command, "--version") == 0) {
        result.should_exit = true;
        result.exit_code = 0;
        result.output = getVersionMessage();
        return result;
    }

    // Direct commands (formerly node subcommands)
    if (std::strcmp(command, "serve") == 0) {
        result.subcommand = Subcommand::Serve;

        // Check for --help
        if (hasHelpFlag(argc, argv, 2)) {
            result.should_exit = true;
            result.exit_code = 0;
            result.output = getServeHelpMessage();
            return result;
        }

        // Parse serve options
        for (int i = 2; i < argc; ++i) {
            if (std::strcmp(argv[i], "--port") == 0 && i + 1 < argc) {
                result.serve_options.port = static_cast<uint16_t>(std::stoi(argv[++i]));
            } else if (std::strcmp(argv[i], "--host") == 0 && i + 1 < argc) {
                result.serve_options.host = argv[++i];
            } else if (std::strcmp(argv[i], "--model") == 0 && i + 1 < argc) {
                result.serve_options.model = argv[++i];
            } else if (std::strcmp(argv[i], "--model-name") == 0 && i + 1 < argc) {
                result.serve_options.model_name = argv[++i];
            } else if (std::strcmp(argv[i], "--mmproj") == 0 && i + 1 < argc) {
                result.serve_options.mmproj = argv[++i];
            } else if (std::strcmp(argv[i], "--ctx-size") == 0 && i + 1 < argc) {
                result.serve_options.ctx_size = std::stoi(argv[++i]);
            }
        }
        return result;
    }

    if (std::strcmp(command, "run") == 0) {
        result.subcommand = Subcommand::Run;

        // Check for --help
        if (hasHelpFlag(argc, argv, 2)) {
            result.should_exit = true;
            result.exit_code = 0;
            result.output = getRunHelpMessage();
            return result;
        }

        // Parse run options
        bool model_found = false;
        for (int i = 2; i < argc; ++i) {
            if (std::strcmp(argv[i], "--think") == 0) {
                result.run_options.show_thinking = true;
                result.run_options.hide_thinking = false;
            } else if (std::strcmp(argv[i], "--hide-think") == 0) {
                result.run_options.hide_thinking = true;
                result.run_options.show_thinking = false;
            } else if (argv[i][0] != '-' && !model_found) {
                result.run_options.model = argv[i];
                model_found = true;
            }
        }

        // Model name is required
        if (!model_found) {
            result.should_exit = true;
            result.exit_code = 1;
            result.output = "Error: model name required\n\nUsage: xllm run <MODEL>\n";
            return result;
        }
        return result;
    }

    if (std::strcmp(command, "pull") == 0) {
        result.subcommand = Subcommand::Pull;

        // Check for --help
        if (hasHelpFlag(argc, argv, 2)) {
            result.should_exit = true;
            result.exit_code = 0;
            result.output = getPullHelpMessage();
            return result;
        }

        // Parse pull options - model name
        for (int i = 2; i < argc; ++i) {
            if (argv[i][0] != '-') {
                result.pull_options.model = argv[i];
                break;
            }
        }

        // Model name is required
        if (result.pull_options.model.empty()) {
            result.should_exit = true;
            result.exit_code = 1;
            result.output = "Error: model name required\n\nUsage: xllm pull <MODEL>\n";
            return result;
        }
        return result;
    }

    if (std::strcmp(command, "list") == 0) {
        result.subcommand = Subcommand::List;

        // Check for --help
        if (hasHelpFlag(argc, argv, 2)) {
            result.should_exit = true;
            result.exit_code = 0;
            result.output = getListHelpMessage();
            return result;
        }
        return result;
    }

    if (std::strcmp(command, "show") == 0) {
        result.subcommand = Subcommand::Show;

        // Check for --help
        if (hasHelpFlag(argc, argv, 2)) {
            result.should_exit = true;
            result.exit_code = 0;
            result.output = getShowHelpMessage();
            return result;
        }

        // Parse show options
        bool model_found = false;
        for (int i = 2; i < argc; ++i) {
            if (std::strcmp(argv[i], "--license") == 0) {
                result.show_options.license_only = true;
            } else if (std::strcmp(argv[i], "--modelfile") == 0) {
                result.show_options.modelfile_only = true;
            } else if (std::strcmp(argv[i], "--parameters") == 0) {
                result.show_options.parameters_only = true;
            } else if (std::strcmp(argv[i], "--template") == 0) {
                result.show_options.template_only = true;
            } else if (std::strcmp(argv[i], "--system") == 0) {
                result.show_options.system_only = true;
            } else if (argv[i][0] != '-' && !model_found) {
                result.show_options.model = argv[i];
                model_found = true;
            }
        }

        // Model name is required
        if (!model_found) {
            result.should_exit = true;
            result.exit_code = 1;
            result.output = "Error: model name required\n\nUsage: xllm show <MODEL>\n";
            return result;
        }
        return result;
    }

    if (std::strcmp(command, "rm") == 0) {
        result.subcommand = Subcommand::Rm;

        // Check for --help
        if (hasHelpFlag(argc, argv, 2)) {
            result.should_exit = true;
            result.exit_code = 0;
            result.output = getRmHelpMessage();
            return result;
        }

        // Parse model name
        for (int i = 2; i < argc; ++i) {
            if (argv[i][0] != '-') {
                result.model_options.model = argv[i];
                break;
            }
        }

        // Model name is required
        if (result.model_options.model.empty()) {
            result.should_exit = true;
            result.exit_code = 1;
            result.output = "Error: model name required\n\nUsage: xllm rm <MODEL>\n";
            return result;
        }
        return result;
    }

    if (std::strcmp(command, "stop") == 0) {
        result.subcommand = Subcommand::Stop;

        // Check for --help
        if (hasHelpFlag(argc, argv, 2)) {
            result.should_exit = true;
            result.exit_code = 0;
            result.output = getStopHelpMessage();
            return result;
        }

        // Parse model name
        for (int i = 2; i < argc; ++i) {
            if (argv[i][0] != '-') {
                result.model_options.model = argv[i];
                break;
            }
        }

        // Model name is required
        if (result.model_options.model.empty()) {
            result.should_exit = true;
            result.exit_code = 1;
            result.output = "Error: model name required\n\nUsage: xllm stop <MODEL>\n";
            return result;
        }
        return result;
    }

    if (std::strcmp(command, "ps") == 0) {
        result.subcommand = Subcommand::Ps;

        // Check for --help
        if (hasHelpFlag(argc, argv, 2)) {
            result.should_exit = true;
            result.exit_code = 0;
            result.output = getPsHelpMessage();
            return result;
        }
        return result;
    }

    if (std::strcmp(command, "profile") == 0) {
        result.subcommand = Subcommand::Profile;

        if (hasHelpFlag(argc, argv, 2)) {
            result.should_exit = true;
            result.exit_code = 0;
            result.output = getProfileHelpMessage();
            return result;
        }

        for (int i = 2; i < argc; ++i) {
            if (std::strcmp(argv[i], "--prompt") == 0 && i + 1 < argc) {
                result.profile_options.prompt = argv[++i];
            } else if (std::strcmp(argv[i], "--tokens") == 0 && i + 1 < argc) {
                result.profile_options.max_tokens = std::stoi(argv[++i]);
            } else if (argv[i][0] != '-' && result.profile_options.model.empty()) {
                result.profile_options.model = argv[i];
            }
        }

        if (result.profile_options.model.empty()) {
            result.should_exit = true;
            result.exit_code = 1;
            result.output = "Error: model name required\n\nUsage: xllm profile <MODEL>\n";
            return result;
        }
        return result;
    }

    if (std::strcmp(command, "benchmark") == 0) {
        result.subcommand = Subcommand::Benchmark;

        if (hasHelpFlag(argc, argv, 2)) {
            result.should_exit = true;
            result.exit_code = 0;
            result.output = getBenchmarkHelpMessage();
            return result;
        }

        for (int i = 2; i < argc; ++i) {
            if (std::strcmp(argv[i], "--prompt") == 0 && i + 1 < argc) {
                result.benchmark_options.prompt = argv[++i];
            } else if (std::strcmp(argv[i], "--tokens") == 0 && i + 1 < argc) {
                result.benchmark_options.max_tokens = std::stoi(argv[++i]);
            } else if (std::strcmp(argv[i], "--runs") == 0 && i + 1 < argc) {
                result.benchmark_options.runs = std::stoi(argv[++i]);
            } else if (argv[i][0] != '-' && result.benchmark_options.model.empty()) {
                result.benchmark_options.model = argv[i];
            }
        }

        if (result.benchmark_options.model.empty()) {
            result.should_exit = true;
            result.exit_code = 1;
            result.output = "Error: model name required\n\nUsage: xllm benchmark <MODEL>\n";
            return result;
        }
        return result;
    }

    if (std::strcmp(command, "compare") == 0) {
        result.subcommand = Subcommand::Compare;

        if (hasHelpFlag(argc, argv, 2)) {
            result.should_exit = true;
            result.exit_code = 0;
            result.output = getCompareHelpMessage();
            return result;
        }

        std::vector<std::string> models;
        for (int i = 2; i < argc; ++i) {
            if (std::strcmp(argv[i], "--prompt") == 0 && i + 1 < argc) {
                result.compare_options.prompt = argv[++i];
            } else if (std::strcmp(argv[i], "--tokens") == 0 && i + 1 < argc) {
                result.compare_options.max_tokens = std::stoi(argv[++i]);
            } else if (std::strcmp(argv[i], "--runs") == 0 && i + 1 < argc) {
                result.compare_options.runs = std::stoi(argv[++i]);
            } else if (argv[i][0] != '-') {
                models.push_back(argv[i]);
            }
        }
        if (models.size() >= 2) {
            result.compare_options.model_a = models[0];
            result.compare_options.model_b = models[1];
        }

        if (result.compare_options.model_a.empty() || result.compare_options.model_b.empty()) {
            result.should_exit = true;
            result.exit_code = 1;
            result.output = "Error: two model names required\n\nUsage: xllm compare <MODEL_A> <MODEL_B>\n";
            return result;
        }
        return result;
    }

    if (std::strcmp(command, "convert") == 0) {
        result.subcommand = Subcommand::Convert;

        if (hasHelpFlag(argc, argv, 2)) {
            result.should_exit = true;
            result.exit_code = 0;
            result.output = getConvertHelpMessage();
            return result;
        }

        for (int i = 2; i < argc; ++i) {
            if (std::strcmp(argv[i], "--name") == 0 && i + 1 < argc) {
                result.convert_options.name = argv[++i];
            } else if (std::strcmp(argv[i], "--format") == 0 && i + 1 < argc) {
                result.convert_options.format = argv[++i];
            } else if (argv[i][0] != '-' && result.convert_options.source.empty()) {
                result.convert_options.source = argv[i];
            }
        }

        if (result.convert_options.source.empty()) {
            result.should_exit = true;
            result.exit_code = 1;
            result.output = "Error: source path required\n\nUsage: xllm convert <SOURCE> --name <MODEL>\n";
            return result;
        }
        if (result.convert_options.name.empty()) {
            result.should_exit = true;
            result.exit_code = 1;
            result.output = "Error: name required\n\nUsage: xllm convert <SOURCE> --name <MODEL>\n";
            return result;
        }
        return result;
    }

    if (std::strcmp(command, "export") == 0) {
        result.subcommand = Subcommand::Export;

        if (hasHelpFlag(argc, argv, 2)) {
            result.should_exit = true;
            result.exit_code = 0;
            result.output = getExportHelpMessage();
            return result;
        }

        for (int i = 2; i < argc; ++i) {
            if (std::strcmp(argv[i], "--output") == 0 && i + 1 < argc) {
                result.export_options.output = argv[++i];
            } else if (argv[i][0] != '-' && result.export_options.model.empty()) {
                result.export_options.model = argv[i];
            }
        }

        if (result.export_options.model.empty()) {
            result.should_exit = true;
            result.exit_code = 1;
            result.output = "Error: model name required\n\nUsage: xllm export <MODEL> --output <FILE>\n";
            return result;
        }
        if (result.export_options.output.empty()) {
            result.should_exit = true;
            result.exit_code = 1;
            result.output = "Error: output file required\n\nUsage: xllm export <MODEL> --output <FILE>\n";
            return result;
        }
        return result;
    }

    if (std::strcmp(command, "import") == 0) {
        result.subcommand = Subcommand::Import;

        if (hasHelpFlag(argc, argv, 2)) {
            result.should_exit = true;
            result.exit_code = 0;
            result.output = getImportHelpMessage();
            return result;
        }

        for (int i = 2; i < argc; ++i) {
            if (std::strcmp(argv[i], "--file") == 0 && i + 1 < argc) {
                result.import_options.file = argv[++i];
            } else if (argv[i][0] != '-' && result.import_options.model.empty()) {
                result.import_options.model = argv[i];
            }
        }

        if (result.import_options.model.empty()) {
            result.should_exit = true;
            result.exit_code = 1;
            result.output = "Error: model name required\n\nUsage: xllm import <MODEL> --file <FILE>\n";
            return result;
        }
        if (result.import_options.file.empty()) {
            result.should_exit = true;
            result.exit_code = 1;
            result.output = "Error: file required\n\nUsage: xllm import <MODEL> --file <FILE>\n";
            return result;
        }
        return result;
    }

    // Check for unknown flags (starting with - or --)
    if (command[0] == '-') {
        result.should_exit = true;
        result.exit_code = 1;
        std::ostringstream oss;
        oss << "Unknown option: " << command << "\n\n";
        oss << getHelpMessage();
        result.output = oss.str();
        return result;
    }

    // Unknown command
    result.should_exit = true;
    result.exit_code = 1;
    std::ostringstream oss;
    oss << "Unknown command: " << command << "\n\n";
    oss << getHelpMessage();
    result.output = oss.str();
    return result;
}

std::string subcommandToString(Subcommand subcommand) {
    switch (subcommand) {
        case Subcommand::None: return "none";
        case Subcommand::Serve: return "serve";
        case Subcommand::Run: return "run";
        case Subcommand::Pull: return "pull";
        case Subcommand::List: return "list";
        case Subcommand::Show: return "show";
        case Subcommand::Rm: return "rm";
        case Subcommand::Stop: return "stop";
        case Subcommand::Ps: return "ps";
        case Subcommand::Profile: return "profile";
        case Subcommand::Benchmark: return "benchmark";
        case Subcommand::Compare: return "compare";
        case Subcommand::Convert: return "convert";
        case Subcommand::Export: return "export";
        case Subcommand::Import: return "import";
        default: return "unknown";
    }
}

}  // namespace xllm
