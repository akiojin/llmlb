#include "metrics/metrics_state.h"

namespace xllm::metrics {

namespace {
std::mutex g_metrics_mutex;
TokenMetricsSnapshot g_snapshot;
}

void record_token_metrics(const TokenMetrics& metrics) {
    std::lock_guard<std::mutex> lock(g_metrics_mutex);
    g_snapshot.tokens_total += static_cast<uint64_t>(metrics.token_count);
    g_snapshot.tokens_per_second = metrics.tokens_per_second;
    g_snapshot.ttft_ms = metrics.ttft_ms;
}

TokenMetricsSnapshot token_metrics_snapshot() {
    std::lock_guard<std::mutex> lock(g_metrics_mutex);
    return g_snapshot;
}

}  // namespace xllm::metrics
