#include "runtime/state.h"

namespace xllm {

std::atomic<bool> g_running_flag{true};
std::atomic<bool> g_ready_flag{false};
std::atomic<unsigned int> g_active_requests{0};

}  // namespace xllm
