#include "system/tensor_parallelism.h"

#include <algorithm>

namespace xllm {

namespace {

uint64_t deviceBudgetBytes(const GpuDevice& device) {
    if (!device.is_available) {
        return 0;
    }
    if (device.free_memory_bytes > 0) {
        return static_cast<uint64_t>(device.free_memory_bytes);
    }
    return static_cast<uint64_t>(device.memory_bytes);
}

}  // namespace

std::vector<float> computeTensorSplitRatios(const std::vector<GpuDevice>& devices) {
    std::vector<uint64_t> budgets;
    budgets.reserve(devices.size());

    uint64_t total_budget = 0;
    for (const auto& device : devices) {
        const uint64_t budget = deviceBudgetBytes(device);
        if (budget == 0) {
            continue;
        }
        budgets.push_back(budget);
        total_budget += budget;
    }

    if (total_budget == 0 || budgets.empty()) {
        return {};
    }

    std::vector<float> ratios;
    ratios.reserve(budgets.size());
    for (const auto budget : budgets) {
        const double ratio = static_cast<double>(budget) / static_cast<double>(total_budget);
        ratios.push_back(static_cast<float>(ratio));
    }

    return ratios;
}

size_t computeGpuLayersForOffload(size_t total_layers,
                                  uint64_t model_vram_bytes,
                                  uint64_t available_vram_bytes) {
    if (total_layers == 0) {
        return 0;
    }

    // モデルサイズが不明な場合は「すべてGPUに載せたい」という意図を尊重する
    if (model_vram_bytes == 0) {
        return total_layers;
    }

    if (available_vram_bytes >= model_vram_bytes) {
        return total_layers;
    }

    if (available_vram_bytes == 0) {
        return 0;
    }

    const double fraction = static_cast<double>(available_vram_bytes) /
                            static_cast<double>(model_vram_bytes);
    const double estimated_layers = fraction * static_cast<double>(total_layers);
    const size_t layers = static_cast<size_t>(estimated_layers);
    return std::min(layers, total_layers);
}

}  // namespace xllm
