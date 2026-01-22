// SPEC-58378000: node rm command
// Removes a model (no confirmation, ollama-compatible)

#include "utils/cli.h"
#include "cli/cli_client.h"
#include "cli/ollama_compat.h"
#include <iostream>

namespace xllm {
namespace cli {
namespace commands {

/// Execute the 'node rm' command
/// @param options Model options (model name)
/// @return Exit code (0=success, 1=error, 2=connection error)
int rm(const ModelOptions& options) {
    if (options.model.empty()) {
        std::cerr << "Error: model name required" << std::endl;
        return 1;
    }

    // Check if this is an ollama model reference
    if (OllamaCompat::hasOllamaPrefix(options.model)) {
        std::cerr << "Error: cannot delete ollama models (read-only reference)" << std::endl;
        std::cerr << "Use 'ollama rm " << OllamaCompat::stripOllamaPrefix(options.model)
                  << "' to delete the model from ollama" << std::endl;
        return 1;
    }

    // Create CLI client
    auto client = std::make_shared<CliClient>();

    // Check server connection
    if (!client->isServerRunning()) {
        std::cerr << "Error: Could not connect to xllm server" << std::endl;
        std::cerr << "Start the server with: xllm serve" << std::endl;
        return 2;
    }

    // Delete model (no confirmation - ollama compatible)
    auto result = client->deleteModel(options.model);
    if (!result.ok()) {
        std::cerr << "Error: " << result.error_message << std::endl;
        return 1;
    }

    std::cout << "deleted '" << options.model << "'" << std::endl;
    return 0;
}

}  // namespace commands
}  // namespace cli
}  // namespace xllm
