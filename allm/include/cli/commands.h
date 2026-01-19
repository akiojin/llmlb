// SPEC-58378000: CLI command function declarations
#pragma once

#include "utils/cli.h"

namespace llm_node {
namespace cli {
namespace commands {

/// Execute the 'node serve' command
/// @param options Server options (port, host)
/// @return Exit code (0=success, 1=error)
int serve(const ServeOptions& options);

/// Execute the 'node run' command (REPL)
/// @param options Run options (model, think flags)
/// @return Exit code (0=success, 1=error, 2=connection error)
int run(const RunOptions& options);

/// Execute the 'node pull' command
/// @param options Pull options (model)
/// @return Exit code (0=success, 1=error, 2=connection error)
int pull(const PullOptions& options);

/// Execute the 'node list' command
/// @param options Model options
/// @return Exit code (0=success, 1=error, 2=connection error)
int list(const ModelOptions& options);

/// Execute the 'node show' command
/// @param options Show options (model, flags)
/// @return Exit code (0=success, 1=error, 2=connection error)
int show(const ShowOptions& options);

/// Execute the 'node rm' command
/// @param options Model options (model)
/// @return Exit code (0=success, 1=error, 2=connection error)
int rm(const ModelOptions& options);

/// Execute the 'node stop' command
/// @param options Model options (model)
/// @return Exit code (0=success, 1=error, 2=connection error)
int stop(const ModelOptions& options);

/// Execute the 'node ps' command
/// @return Exit code (0=success, 1=error, 2=connection error)
int ps();

/// Execute the 'router nodes' command
/// @return Exit code (0=success, 1=error)
int router_nodes();

/// Execute the 'router models' command
/// @return Exit code (0=success, 1=error)
int router_models();

/// Execute the 'router status' command
/// @return Exit code (0=success, 1=error)
int router_status();

}  // namespace commands
}  // namespace cli
}  // namespace llm_node
