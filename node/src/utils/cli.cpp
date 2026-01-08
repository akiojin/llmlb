// SPEC-58378000: CLI argument parser with ollama-compatible subcommands
#include "utils/cli.h"
#include "utils/version.h"
#include <sstream>
#include <cstring>
#include <cstdlib>

namespace llm_node {

// Forward declarations for help messages
std::string getNodeHelpMessage();
std::string getRouterHelpMessage();
std::string getServeHelpMessage();
std::string getRunHelpMessage();
std::string getPullHelpMessage();
std::string getListHelpMessage();
std::string getShowHelpMessage();
std::string getRmHelpMessage();
std::string getStopHelpMessage();
std::string getPsHelpMessage();

std::string getHelpMessage() {
    std::ostringstream oss;
    oss << "llm-router " << LLM_NODE_VERSION << " - LLM inference router and node\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    llm-router <COMMAND>\n";
    oss << "\n";
    oss << "COMMANDS:\n";
    oss << "    node       Node commands (serve, run, pull, list, show, rm, stop, ps)\n";
    oss << "    router     Router commands (nodes, models, status)\n";
    oss << "\n";
    oss << "OPTIONS:\n";
    oss << "    -h, --help       Print help information\n";
    oss << "    -V, --version    Print version information\n";
    oss << "\n";
    oss << "Run 'llm-router node --help' or 'llm-router router --help' for more info.\n";
    return oss.str();
}

std::string getNodeHelpMessage() {
    std::ostringstream oss;
    oss << "llm-router node - Node subcommands\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    llm-router node <SUBCOMMAND>\n";
    oss << "\n";
    oss << "SUBCOMMANDS:\n";
    oss << "    serve      Start the server (foreground)\n";
    oss << "    run        Chat with a model (REPL)\n";
    oss << "    pull       Download a model from HuggingFace\n";
    oss << "    list       List local models\n";
    oss << "    show       Show model metadata\n";
    oss << "    rm         Delete a model\n";
    oss << "    stop       Unload a running model\n";
    oss << "    ps         List running models\n";
    oss << "\n";
    oss << "ENVIRONMENT VARIABLES:\n";
    oss << "    LLM_NODE_PORT                HTTP server port (default: 32769)\n";
    oss << "    LLM_NODE_MODELS_DIR          Model files directory\n";
    oss << "    LLM_ROUTER_HOST              Server host for client commands\n";
    oss << "    LLM_ROUTER_DEBUG             Enable debug logging\n";
    oss << "    HF_TOKEN                     HuggingFace API token (for gated models)\n";
    return oss.str();
}

std::string getRouterHelpMessage() {
    std::ostringstream oss;
    oss << "llm-router router - Router subcommands\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    llm-router router <SUBCOMMAND>\n";
    oss << "\n";
    oss << "SUBCOMMANDS:\n";
    oss << "    nodes      Manage cluster nodes\n";
    oss << "    models     Manage cluster models\n";
    oss << "    status     Show cluster status\n";
    return oss.str();
}

std::string getServeHelpMessage() {
    std::ostringstream oss;
    oss << "llm-router node serve - Start the server\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    llm-router node serve [OPTIONS]\n";
    oss << "\n";
    oss << "OPTIONS:\n";
    oss << "    --port <PORT>    Server port (default: 32769, or LLM_NODE_PORT)\n";
    oss << "    --host <HOST>    Bind address (default: 0.0.0.0)\n";
    oss << "    -h, --help       Print help\n";
    return oss.str();
}

std::string getRunHelpMessage() {
    std::ostringstream oss;
    oss << "llm-router node run - Chat with a model\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    llm-router node run <MODEL> [OPTIONS]\n";
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
    oss << "llm-router node pull - Download a model\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    llm-router node pull <MODEL>\n";
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
    oss << "llm-router node list - List local models\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    llm-router node list\n";
    oss << "\n";
    oss << "OPTIONS:\n";
    oss << "    -h, --help       Print help\n";
    oss << "\n";
    oss << "Shows models from:\n";
    oss << "    - llm-router models directory\n";
    oss << "    - ollama models (~/.ollama/models/) with 'ollama:' prefix\n";
    return oss.str();
}

std::string getShowHelpMessage() {
    std::ostringstream oss;
    oss << "llm-router node show - Show model metadata\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    llm-router node show <MODEL> [OPTIONS]\n";
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
    oss << "llm-router node rm - Delete a model\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    llm-router node rm <MODEL>\n";
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
    oss << "llm-router node stop - Unload a running model\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    llm-router node stop <MODEL>\n";
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
    oss << "llm-router node ps - List running models\n";
    oss << "\n";
    oss << "USAGE:\n";
    oss << "    llm-router node ps\n";
    oss << "\n";
    oss << "OPTIONS:\n";
    oss << "    -h, --help       Print help\n";
    oss << "\n";
    oss << "COLUMNS:\n";
    oss << "    NAME, ID, SIZE, PROCESSOR, VRAM, TEMP, REQS, UNTIL\n";
    return oss.str();
}

std::string getVersionMessage() {
    std::ostringstream oss;
    oss << "llm-router " << LLM_NODE_VERSION << "\n";
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

// Parse node subcommands
CliResult parseNodeSubcommand(int argc, char* argv[], int argIndex) {
    CliResult result;

    if (argIndex >= argc) {
        result.should_exit = true;
        result.exit_code = 0;
        result.output = getNodeHelpMessage();
        return result;
    }

    const char* subcommand = argv[argIndex];

    // Check for help
    if (std::strcmp(subcommand, "-h") == 0 || std::strcmp(subcommand, "--help") == 0) {
        result.should_exit = true;
        result.exit_code = 0;
        result.output = getNodeHelpMessage();
        return result;
    }

    // Parse subcommand
    if (std::strcmp(subcommand, "serve") == 0) {
        result.subcommand = Subcommand::NodeServe;

        // Check for --help
        if (hasHelpFlag(argc, argv, argIndex + 1)) {
            result.should_exit = true;
            result.exit_code = 0;
            result.output = getServeHelpMessage();
            return result;
        }

        // Parse serve options
        for (int i = argIndex + 1; i < argc; ++i) {
            if (std::strcmp(argv[i], "--port") == 0 && i + 1 < argc) {
                result.serve_options.port = static_cast<uint16_t>(std::stoi(argv[++i]));
            } else if (std::strcmp(argv[i], "--host") == 0 && i + 1 < argc) {
                result.serve_options.host = argv[++i];
            }
        }
        return result;
    }

    if (std::strcmp(subcommand, "run") == 0) {
        result.subcommand = Subcommand::NodeRun;

        // Check for --help
        if (hasHelpFlag(argc, argv, argIndex + 1)) {
            result.should_exit = true;
            result.exit_code = 0;
            result.output = getRunHelpMessage();
            return result;
        }

        // Parse run options
        bool model_found = false;
        for (int i = argIndex + 1; i < argc; ++i) {
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
            result.output = "Error: model name required\n\nUsage: llm-router node run <MODEL>\n";
            return result;
        }
        return result;
    }

    if (std::strcmp(subcommand, "pull") == 0) {
        result.subcommand = Subcommand::NodePull;

        // Check for --help
        if (hasHelpFlag(argc, argv, argIndex + 1)) {
            result.should_exit = true;
            result.exit_code = 0;
            result.output = getPullHelpMessage();
            return result;
        }

        // Parse pull options - model name
        for (int i = argIndex + 1; i < argc; ++i) {
            if (argv[i][0] != '-') {
                result.pull_options.model = argv[i];
                break;
            }
        }

        // Model name is required
        if (result.pull_options.model.empty()) {
            result.should_exit = true;
            result.exit_code = 1;
            result.output = "Error: model name required\n\nUsage: llm-router node pull <MODEL>\n";
            return result;
        }
        return result;
    }

    if (std::strcmp(subcommand, "list") == 0) {
        result.subcommand = Subcommand::NodeList;

        // Check for --help
        if (hasHelpFlag(argc, argv, argIndex + 1)) {
            result.should_exit = true;
            result.exit_code = 0;
            result.output = getListHelpMessage();
            return result;
        }
        return result;
    }

    if (std::strcmp(subcommand, "show") == 0) {
        result.subcommand = Subcommand::NodeShow;

        // Check for --help
        if (hasHelpFlag(argc, argv, argIndex + 1)) {
            result.should_exit = true;
            result.exit_code = 0;
            result.output = getShowHelpMessage();
            return result;
        }

        // Parse show options
        bool model_found = false;
        for (int i = argIndex + 1; i < argc; ++i) {
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
            result.output = "Error: model name required\n\nUsage: llm-router node show <MODEL>\n";
            return result;
        }
        return result;
    }

    if (std::strcmp(subcommand, "rm") == 0) {
        result.subcommand = Subcommand::NodeRm;

        // Check for --help
        if (hasHelpFlag(argc, argv, argIndex + 1)) {
            result.should_exit = true;
            result.exit_code = 0;
            result.output = getRmHelpMessage();
            return result;
        }

        // Parse model name
        for (int i = argIndex + 1; i < argc; ++i) {
            if (argv[i][0] != '-') {
                result.model_options.model = argv[i];
                break;
            }
        }

        // Model name is required
        if (result.model_options.model.empty()) {
            result.should_exit = true;
            result.exit_code = 1;
            result.output = "Error: model name required\n\nUsage: llm-router node rm <MODEL>\n";
            return result;
        }
        return result;
    }

    if (std::strcmp(subcommand, "stop") == 0) {
        result.subcommand = Subcommand::NodeStop;

        // Check for --help
        if (hasHelpFlag(argc, argv, argIndex + 1)) {
            result.should_exit = true;
            result.exit_code = 0;
            result.output = getStopHelpMessage();
            return result;
        }

        // Parse model name
        for (int i = argIndex + 1; i < argc; ++i) {
            if (argv[i][0] != '-') {
                result.model_options.model = argv[i];
                break;
            }
        }

        // Model name is required
        if (result.model_options.model.empty()) {
            result.should_exit = true;
            result.exit_code = 1;
            result.output = "Error: model name required\n\nUsage: llm-router node stop <MODEL>\n";
            return result;
        }
        return result;
    }

    if (std::strcmp(subcommand, "ps") == 0) {
        result.subcommand = Subcommand::NodePs;

        // Check for --help
        if (hasHelpFlag(argc, argv, argIndex + 1)) {
            result.should_exit = true;
            result.exit_code = 0;
            result.output = getPsHelpMessage();
            return result;
        }
        return result;
    }

    // Unknown subcommand
    result.should_exit = true;
    result.exit_code = 1;
    std::ostringstream oss;
    oss << "Error: Unknown node subcommand '" << subcommand << "'\n\n";
    oss << getNodeHelpMessage();
    result.output = oss.str();
    return result;
}

// Parse router subcommands
CliResult parseRouterSubcommand(int argc, char* argv[], int argIndex) {
    CliResult result;

    if (argIndex >= argc) {
        result.should_exit = true;
        result.exit_code = 0;
        result.output = getRouterHelpMessage();
        return result;
    }

    const char* subcommand = argv[argIndex];

    // Check for help
    if (std::strcmp(subcommand, "-h") == 0 || std::strcmp(subcommand, "--help") == 0) {
        result.should_exit = true;
        result.exit_code = 0;
        result.output = getRouterHelpMessage();
        return result;
    }

    if (std::strcmp(subcommand, "nodes") == 0) {
        result.subcommand = Subcommand::RouterNodes;
        return result;
    }

    if (std::strcmp(subcommand, "models") == 0) {
        result.subcommand = Subcommand::RouterModels;
        return result;
    }

    if (std::strcmp(subcommand, "status") == 0) {
        result.subcommand = Subcommand::RouterStatus;
        return result;
    }

    // Unknown subcommand
    result.should_exit = true;
    result.exit_code = 1;
    std::ostringstream oss;
    oss << "Error: Unknown router subcommand '" << subcommand << "'\n\n";
    oss << getRouterHelpMessage();
    result.output = oss.str();
    return result;
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

    // Main commands
    if (std::strcmp(command, "node") == 0) {
        return parseNodeSubcommand(argc, argv, 2);
    }

    if (std::strcmp(command, "router") == 0) {
        return parseRouterSubcommand(argc, argv, 2);
    }

    // Legacy mode: no command means start server (backward compatibility)
    // This allows running just "llm-node" or "llm-router" to start the server
    result.should_exit = false;
    result.subcommand = Subcommand::None;
    return result;
}

std::string subcommandToString(Subcommand subcommand) {
    switch (subcommand) {
        case Subcommand::None: return "none";
        case Subcommand::NodeServe: return "node serve";
        case Subcommand::NodeRun: return "node run";
        case Subcommand::NodePull: return "node pull";
        case Subcommand::NodeList: return "node list";
        case Subcommand::NodeShow: return "node show";
        case Subcommand::NodeRm: return "node rm";
        case Subcommand::NodeStop: return "node stop";
        case Subcommand::NodePs: return "node ps";
        case Subcommand::RouterNodes: return "router nodes";
        case Subcommand::RouterModels: return "router models";
        case Subcommand::RouterStatus: return "router status";
        default: return "unknown";
    }
}

}  // namespace llm_node
