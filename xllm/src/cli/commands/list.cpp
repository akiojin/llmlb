// SPEC-58378000: node list command
// Lists locally available models

#include "utils/cli.h"
#include "cli/cli_client.h"
#include "cli/ollama_compat.h"
#include <iostream>
#include <iomanip>
#include <vector>
#include <string>

namespace xllm {
namespace cli {
namespace commands {

/// Execute the 'node list' command
/// @param options Model options (unused for list)
/// @return Exit code (0=success, 1=error, 2=connection error)
int list(const ModelOptions& /* options */) {
    // Create CLI client
    auto client = std::make_shared<CliClient>();

    // Check server connection
    if (!client->isServerRunning()) {
        std::cerr << "Error: Could not connect to xllm server" << std::endl;
        std::cerr << "Start the server with: xllm serve" << std::endl;
        return 2;
    }

    // Get models from server
    auto result = client->listModels();
    if (!result.ok()) {
        std::cerr << "Error: " << result.error_message << std::endl;
        return 1;
    }

    // Also check for ollama models (read-only reference)
    OllamaCompat ollama;
    auto ollama_models = ollama.listModels();

    // Print header
    std::cout << std::left
              << std::setw(40) << "NAME"
              << std::setw(20) << "ID"
              << std::setw(12) << "SIZE"
              << std::setw(20) << "MODIFIED"
              << std::endl;

    // Print models from server
    if (result.data && result.data->contains("models")) {
        for (const auto& model : (*result.data)["models"]) {
            std::string name = model.value("name", "unknown");
            std::string id = model.value("digest", "").substr(0, 12);
            uint64_t size = model.value("size", 0);
            std::string modified = model.value("modified_at", "");

            // Format size
            std::string size_str;
            if (size >= 1024ULL * 1024 * 1024) {
                size_str = std::to_string(size / (1024ULL * 1024 * 1024)) + " GB";
            } else if (size >= 1024ULL * 1024) {
                size_str = std::to_string(size / (1024ULL * 1024)) + " MB";
            } else {
                size_str = std::to_string(size) + " B";
            }

            std::cout << std::left
                      << std::setw(40) << name
                      << std::setw(20) << id
                      << std::setw(12) << size_str
                      << std::setw(20) << modified
                      << std::endl;
        }
    }

    // Print ollama models (with ollama: prefix indicator)
    for (const auto& model : ollama_models) {
        std::string size_str;
        if (model.size_bytes >= 1024ULL * 1024 * 1024) {
            size_str = std::to_string(model.size_bytes / (1024ULL * 1024 * 1024)) + " GB";
        } else if (model.size_bytes >= 1024ULL * 1024) {
            size_str = std::to_string(model.size_bytes / (1024ULL * 1024)) + " MB";
        } else {
            size_str = std::to_string(model.size_bytes) + " B";
        }

        std::cout << std::left
                  << std::setw(40) << ("ollama:" + model.name)
                  << std::setw(20) << model.blob_digest.substr(7, 12)
                  << std::setw(12) << size_str
                  << std::setw(20) << "(readonly)"
                  << std::endl;
    }

    return 0;
}

}  // namespace commands
}  // namespace cli
}  // namespace xllm
