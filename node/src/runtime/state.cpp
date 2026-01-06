#include "runtime/state.h"

namespace llm_node {

std::atomic<bool> g_running_flag{true};
std::atomic<bool> g_ready_flag{false};
std::atomic<unsigned int> g_active_requests{0};

}  // namespace llm_node
