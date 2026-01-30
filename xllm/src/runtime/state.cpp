#include "runtime/state.h"

namespace xllm {

std::atomic<bool> g_running_flag{true};
std::atomic<bool> g_ready_flag{false};
std::atomic<unsigned int> g_active_requests{0};
std::atomic<uint64_t> g_total_requests{0};
std::mutex g_request_mutex;
std::condition_variable g_request_cv;

}  // namespace xllm
