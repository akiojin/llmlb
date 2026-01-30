#pragma once

#include <cstdint>
#include <mutex>

#include "core/inference_engine.h"

namespace xllm::metrics {

struct TokenMetricsSnapshot {
    double ttft_ms{0.0};
    double tokens_per_second{0.0};
    uint64_t tokens_total{0};
};

void record_token_metrics(const TokenMetrics& metrics);
TokenMetricsSnapshot token_metrics_snapshot();

}  // namespace xllm::metrics
