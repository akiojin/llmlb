#include "runtime/state.h"

namespace ollama_node {

std::atomic<bool> g_running_flag{true};
std::atomic<bool> g_ready_flag{false};

}  // namespace ollama_node
