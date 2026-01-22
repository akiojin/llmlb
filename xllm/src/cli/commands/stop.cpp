// SPEC-58378000: node stop command
// Stops (unloads) a running model

#include "utils/cli.h"
#include "cli/cli_client.h"
#include <iostream>

namespace xllm {
namespace cli {
namespace commands {

/// Execute the 'node stop' command
/// @param options Model options (model name)
/// @return Exit code (0=success, 1=error, 2=connection error)
int stop(const ModelOptions& options) {
    if (options.model.empty()) {
        std::cerr << "Error: model name required" << std::endl;
        return 1;
    }

    // Create CLI client
    auto client = std::make_shared<CliClient>();

    // Check server connection
    if (!client->isServerRunning()) {
        std::cerr << "Error: Could not connect to allm server" << std::endl;
        std::cerr << "Start the server with: allm serve" << std::endl;
        return 2;
    }

    // Stop model
    auto result = client->stopModel(options.model);
    if (!result.ok()) {
        std::cerr << "Error: " << result.error_message << std::endl;
        return 1;
    }

    std::cout << "Stopped model '" << options.model << "'" << std::endl;
    return 0;
}

}  // namespace commands
}  // namespace cli
}  // namespace xllm
