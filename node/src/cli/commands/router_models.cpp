// SPEC-58378000: router models command
// Model management in router cluster mode

#include "utils/cli.h"
#include <iostream>

namespace llm_node {
namespace cli {
namespace commands {

/// Execute the 'router models' command
/// @return Exit code (0=success, 1=error, 2=connection error)
int router_models() {
    // TODO: Implement router models management
    // - List deployed models across cluster
    // - Deploy model to nodes
    // - Stop model on nodes

    std::cerr << "Error: router models command not yet implemented" << std::endl;
    std::cerr << "This feature requires a running router instance" << std::endl;
    return 1;
}

}  // namespace commands
}  // namespace cli
}  // namespace llm_node
