// SPEC-58378000: CLI command function declarations
#pragma once

#include "utils/cli.h"

namespace xllm {
namespace cli {
namespace commands {

/// Execute the 'serve' command
/// @param options Server options (port, host)
/// @return Exit code (0=success, 1=error)
int serve(const ServeOptions& options);

/// Execute the 'run' command (REPL)
/// @param options Run options (model, think flags)
/// @return Exit code (0=success, 1=error, 2=connection error)
int run(const RunOptions& options);

/// Execute the 'pull' command
/// @param options Pull options (model)
/// @return Exit code (0=success, 1=error, 2=connection error)
int pull(const PullOptions& options);

/// Execute the 'list' command
/// @param options Model options
/// @return Exit code (0=success, 1=error, 2=connection error)
int list(const ModelOptions& options);

/// Execute the 'show' command
/// @param options Show options (model, flags)
/// @return Exit code (0=success, 1=error, 2=connection error)
int show(const ShowOptions& options);

/// Execute the 'rm' command
/// @param options Model options (model)
/// @return Exit code (0=success, 1=error, 2=connection error)
int rm(const ModelOptions& options);

/// Execute the 'stop' command
/// @param options Model options (model)
/// @return Exit code (0=success, 1=error, 2=connection error)
int stop(const ModelOptions& options);

/// Execute the 'ps' command
/// @return Exit code (0=success, 1=error, 2=connection error)
int ps();

}  // namespace commands
}  // namespace cli
}  // namespace xllm
