// SPEC-58378000: node pull command
// Downloads a model from HuggingFace

#include "utils/cli.h"
#include "cli/cli_client.h"
#include "cli/progress_renderer.h"
#include <iostream>
#include <cstdlib>

namespace xllm {
namespace cli {
namespace commands {

/// Execute the 'node pull' command
/// @param options Pull options (model name/URL)
/// @return Exit code (0=success, 1=error, 2=connection error)
int pull(const PullOptions& options) {
    if (options.model.empty()) {
        std::cerr << "Error: model name or HuggingFace URL required" << std::endl;
        return 1;
    }

    // Check for HF_TOKEN if this might be a gated model
    const char* hf_token = std::getenv("HF_TOKEN");

    // Create CLI client
    auto client = std::make_shared<CliClient>();

    // Check server connection
    if (!client->isServerRunning()) {
        std::cerr << "Error: Could not connect to xllm server" << std::endl;
        std::cerr << "Start the server with: xllm serve" << std::endl;
        return 2;
    }

    std::cout << "pulling " << options.model << std::endl;

    // Create progress renderer
    ProgressRenderer progress;
    progress.setPhase("pulling manifest");

    // Pull model with progress callback
    auto result = client->pullModel(options.model, [&progress](uint64_t downloaded, uint64_t total, double speed) {
        if (total > 0) {
            progress.update(downloaded, speed);
        }
    });

    if (!result.ok()) {
        progress.fail(result.error_message);

        // Check if it's a gated model error
        if (result.error_message.find("gated") != std::string::npos ||
            result.error_message.find("403") != std::string::npos) {
            if (!hf_token) {
                std::cerr << "This model requires authentication. Set HF_TOKEN environment variable." << std::endl;
            }
        }

        return 1;
    }

    progress.complete();
    return 0;
}

}  // namespace commands
}  // namespace cli
}  // namespace xllm
