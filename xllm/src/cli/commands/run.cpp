// SPEC-58378000: node run command
// Interactive REPL for chatting with a model

#include "utils/cli.h"
#include "cli/cli_client.h"
#include "cli/repl_session.h"
#include <iostream>
#include <memory>

namespace xllm {
namespace cli {
namespace commands {

/// Execute the 'node run' command
/// @param options Run options (model name, thinking flags)
/// @return Exit code (0=success, 1=error, 2=connection error)
int run(const RunOptions& options) {
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

    // Configure session settings
    SessionSettings settings;
    settings.show_thinking = options.show_thinking;
    settings.stream = true;

    // Run REPL session
    ReplSession session(client, options.model, settings);
    return session.run();
}

}  // namespace commands
}  // namespace cli
}  // namespace xllm
