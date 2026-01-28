#include <gtest/gtest.h>

#include <cmath>
#include <vector>

#include "system/tensor_parallelism.h"

namespace {

using xllm::GpuDevice;
using xllm::computeGpuLayersForOffload;
using xllm::computeTensorSplitRatios;

constexpr uint64_t GiB(uint64_t n) {
    return n * 1024ull * 1024ull * 1024ull;
}

// =============================================================================
// T203: Tensor Parallelism自動化テスト
// =============================================================================

TEST(TensorParallelismTest, TensorSplitUsesFreeMemoryWhenAvailable) {
    std::vector<GpuDevice> devices = {
        {0, "GPU0", GiB(24), GiB(12), "8.0", "nvidia", true},
        {1, "GPU1", GiB(24), GiB(6), "8.0", "nvidia", true},
    };

    const auto ratios = computeTensorSplitRatios(devices);
    ASSERT_EQ(ratios.size(), 2u);

    EXPECT_NEAR(ratios[0], 12.0 / 18.0, 1e-6);
    EXPECT_NEAR(ratios[1], 6.0 / 18.0, 1e-6);
}

TEST(TensorParallelismTest, TensorSplitFallsBackToTotalMemory) {
    std::vector<GpuDevice> devices = {
        {0, "GPU0", GiB(16), 0, "8.0", "nvidia", true},
        {1, "GPU1", GiB(8), 0, "8.0", "nvidia", true},
    };

    const auto ratios = computeTensorSplitRatios(devices);
    ASSERT_EQ(ratios.size(), 2u);

    EXPECT_NEAR(ratios[0], 16.0 / 24.0, 1e-6);
    EXPECT_NEAR(ratios[1], 8.0 / 24.0, 1e-6);
}

TEST(TensorParallelismTest, TensorSplitSkipsUnavailableOrZeroBudgetDevices) {
    std::vector<GpuDevice> devices = {
        {0, "GPU0", GiB(16), GiB(8), "8.0", "nvidia", false},
        {1, "GPU1", 0, 0, "8.0", "nvidia", true},
        {2, "GPU2", GiB(12), GiB(6), "8.0", "nvidia", true},
    };

    const auto ratios = computeTensorSplitRatios(devices);
    ASSERT_EQ(ratios.size(), 1u);
    EXPECT_NEAR(ratios[0], 1.0, 1e-6);
}

// =============================================================================
// T211: GPU Offload自動化テスト
// =============================================================================

TEST(GpuOffloadTest, UsesAllLayersWhenModelFits) {
    const size_t layers = computeGpuLayersForOffload(/*total_layers=*/40,
                                                     /*model_vram_bytes=*/4000,
                                                     /*available_vram_bytes=*/4000);
    EXPECT_EQ(layers, 40u);
}

TEST(GpuOffloadTest, ScalesLayersWhenVramIsLimited) {
    const size_t layers = computeGpuLayersForOffload(/*total_layers=*/40,
                                                     /*model_vram_bytes=*/4000,
                                                     /*available_vram_bytes=*/2500);
    // 2500/4000 = 0.625 -> floor(25.0)
    EXPECT_EQ(layers, 25u);
}

TEST(GpuOffloadTest, ReturnsZeroWhenNoVramAvailable) {
    const size_t layers = computeGpuLayersForOffload(/*total_layers=*/32,
                                                     /*model_vram_bytes=*/3200,
                                                     /*available_vram_bytes=*/0);
    EXPECT_EQ(layers, 0u);
}

TEST(GpuOffloadTest, DefaultsToAllLayersWhenModelSizeUnknown) {
    const size_t layers = computeGpuLayersForOffload(/*total_layers=*/28,
                                                     /*model_vram_bytes=*/0,
                                                     /*available_vram_bytes=*/1024);
    EXPECT_EQ(layers, 28u);
}

}  // namespace
