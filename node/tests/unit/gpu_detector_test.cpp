#include <gtest/gtest.h>
#include <optional>

#include "system/gpu_detector.h"

namespace {

using llm_node::GpuDetector;

TEST(GpuDetectorSmokeTest, DefaultsAreEmpty) {
    GpuDetector detector;

    EXPECT_FALSE(detector.hasGpu());
    EXPECT_EQ(detector.getTotalMemory(), 0u);
    EXPECT_DOUBLE_EQ(detector.getCapabilityScore(), 0.0);
    EXPECT_EQ(detector.getGpuById(0), nullptr);
}

TEST(GpuDetectorTest, TotalMemorySumsAvailableDevicesOnly) {
    GpuDetector detector;

    std::vector<llm_node::GpuDevice> devices = {
        {0, "NVIDIA A100", 40ull * 1024 * 1024 * 1024, 30ull * 1024 * 1024 * 1024, "8.0", "nvidia", true},
        {1, "AMD Test", 16ull * 1024 * 1024 * 1024, 8ull * 1024 * 1024 * 1024, "gfx1100", "amd", false},
        {2, "Apple M3", 8ull * 1024 * 1024 * 1024, 7ull * 1024 * 1024 * 1024, "Metal3", "apple", true},
    };

    detector.setDetectedDevicesForTest(devices);

    // Unavailable AMD GPUはメモリ計算から除外される想定
    const size_t expected = (40ull + 8ull) * 1024 * 1024 * 1024;
    EXPECT_EQ(detector.getTotalMemory(), expected);
}

TEST(GpuDetectorTest, CapabilityScoreWeightsByVendorAndComputeCapability) {
    GpuDetector detector;

    std::vector<llm_node::GpuDevice> devices = {
        {0, "NVIDIA 8GB", 8ull * 1024 * 1024 * 1024, 6ull * 1024 * 1024 * 1024, "8.6", "nvidia", true},
        {1, "AMD 16GB", 16ull * 1024 * 1024 * 1024, 12ull * 1024 * 1024 * 1024, "gfx1100", "amd", true},
        {2, "Apple 4GB", 4ull * 1024 * 1024 * 1024, 3ull * 1024 * 1024 * 1024, "Metal3", "apple", true},
    };

    detector.setDetectedDevicesForTest(devices);

#if defined(__APPLE__)
    const double expected = (8.0 + 16.0 + 4.0) * 1.5;
#else
    const double nvidia = 8.0 * (8.6 / 5.0);
    const double amd = 16.0 * 1.2;
    const double apple = 4.0 * 1.5;
    const double expected = nvidia + amd + apple;
#endif

    EXPECT_NEAR(detector.getCapabilityScore(), expected, 1e-6);
}

TEST(GpuDetectorTest, RequireGpuReflectsAvailability) {
    GpuDetector detector;
    detector.setDetectedDevicesForTest({});
    EXPECT_FALSE(detector.requireGpu());

    std::vector<llm_node::GpuDevice> devices = {
        {0, "NVIDIA", 8ull * 1024 * 1024 * 1024, 6ull * 1024 * 1024 * 1024, "8.0", "nvidia", true},
        {1, "Disabled", 4ull * 1024 * 1024 * 1024, 1ull * 1024 * 1024 * 1024, "5.0", "nvidia", false},
    };
    detector.setDetectedDevicesForTest(devices);
    EXPECT_TRUE(detector.requireGpu());
}

TEST(GpuDetectorTest, SelectGpuPrefersLoadedDevice) {
    GpuDetector detector;
    std::vector<llm_node::GpuDevice> devices = {
        {0, "NVIDIA", 8ull * 1024 * 1024 * 1024, 2ull * 1024 * 1024 * 1024, "8.0", "nvidia", true},
        {1, "NVIDIA", 8ull * 1024 * 1024 * 1024, 6ull * 1024 * 1024 * 1024, "8.0", "nvidia", true},
    };
    detector.setDetectedDevicesForTest(devices);

    auto selected = detector.selectGpu(0);
    ASSERT_TRUE(selected.has_value());
    EXPECT_EQ(selected.value(), 0);
}

TEST(GpuDetectorTest, SelectGpuChoosesMostFreeMemory) {
    GpuDetector detector;
    std::vector<llm_node::GpuDevice> devices = {
        {0, "GPU0", 8ull * 1024 * 1024 * 1024, 1ull * 1024 * 1024 * 1024, "8.0", "nvidia", true},
        {1, "GPU1", 8ull * 1024 * 1024 * 1024, 5ull * 1024 * 1024 * 1024, "8.0", "nvidia", true},
        {2, "GPU2", 8ull * 1024 * 1024 * 1024, 3ull * 1024 * 1024 * 1024, "8.0", "nvidia", true},
    };
    detector.setDetectedDevicesForTest(devices);

    auto selected = detector.selectGpu(std::nullopt);
    ASSERT_TRUE(selected.has_value());
    EXPECT_EQ(selected.value(), 1);
}

TEST(GpuDetectorTest, SelectGpuSkipsUnavailableDevices) {
    GpuDetector detector;
    std::vector<llm_node::GpuDevice> devices = {
        {0, "GPU0", 8ull * 1024 * 1024 * 1024, 7ull * 1024 * 1024 * 1024, "8.0", "nvidia", false},
        {1, "GPU1", 8ull * 1024 * 1024 * 1024, 4ull * 1024 * 1024 * 1024, "8.0", "nvidia", true},
    };
    detector.setDetectedDevicesForTest(devices);

    auto selected = detector.selectGpu(std::nullopt);
    ASSERT_TRUE(selected.has_value());
    EXPECT_EQ(selected.value(), 1);
}

// T129: マルチGPU負荷分散テスト - エッジケース

TEST(GpuDetectorTest, SelectGpuReturnsNulloptWhenAllUnavailable) {
    GpuDetector detector;
    std::vector<llm_node::GpuDevice> devices = {
        {0, "GPU0", 8ull * 1024 * 1024 * 1024, 6ull * 1024 * 1024 * 1024, "8.0", "nvidia", false},
        {1, "GPU1", 8ull * 1024 * 1024 * 1024, 4ull * 1024 * 1024 * 1024, "8.0", "nvidia", false},
    };
    detector.setDetectedDevicesForTest(devices);

    auto selected = detector.selectGpu(std::nullopt);
    EXPECT_FALSE(selected.has_value());
}

TEST(GpuDetectorTest, SelectGpuWorksWithSingleGpu) {
    GpuDetector detector;
    std::vector<llm_node::GpuDevice> devices = {
        {0, "GPU0", 16ull * 1024 * 1024 * 1024, 10ull * 1024 * 1024 * 1024, "8.0", "nvidia", true},
    };
    detector.setDetectedDevicesForTest(devices);

    auto selected = detector.selectGpu(std::nullopt);
    ASSERT_TRUE(selected.has_value());
    EXPECT_EQ(selected.value(), 0);
}

TEST(GpuDetectorTest, SelectGpuTieBreaksById) {
    GpuDetector detector;
    std::vector<llm_node::GpuDevice> devices = {
        {0, "GPU0", 8ull * 1024 * 1024 * 1024, 5ull * 1024 * 1024 * 1024, "8.0", "nvidia", true},
        {1, "GPU1", 8ull * 1024 * 1024 * 1024, 5ull * 1024 * 1024 * 1024, "8.0", "nvidia", true},
        {2, "GPU2", 8ull * 1024 * 1024 * 1024, 5ull * 1024 * 1024 * 1024, "8.0", "nvidia", true},
    };
    detector.setDetectedDevicesForTest(devices);

    // 空きメモリが同じ場合は最初の利用可能GPUを選択
    auto selected = detector.selectGpu(std::nullopt);
    ASSERT_TRUE(selected.has_value());
    EXPECT_EQ(selected.value(), 0);
}

TEST(GpuDetectorTest, SelectGpuWithPreferredGpuOverridesMemoryCheck) {
    GpuDetector detector;
    std::vector<llm_node::GpuDevice> devices = {
        {0, "GPU0", 8ull * 1024 * 1024 * 1024, 1ull * 1024 * 1024 * 1024, "8.0", "nvidia", true},
        {1, "GPU1", 8ull * 1024 * 1024 * 1024, 6ull * 1024 * 1024 * 1024, "8.0", "nvidia", true},
    };
    detector.setDetectedDevicesForTest(devices);

    // 既存ロード済みGPU（空きメモリ少）を優先
    auto selected = detector.selectGpu(0);
    ASSERT_TRUE(selected.has_value());
    EXPECT_EQ(selected.value(), 0);
}

TEST(GpuDetectorTest, SelectGpuIgnoresPreferredIfUnavailable) {
    GpuDetector detector;
    std::vector<llm_node::GpuDevice> devices = {
        {0, "GPU0", 8ull * 1024 * 1024 * 1024, 6ull * 1024 * 1024 * 1024, "8.0", "nvidia", false},
        {1, "GPU1", 8ull * 1024 * 1024 * 1024, 4ull * 1024 * 1024 * 1024, "8.0", "nvidia", true},
    };
    detector.setDetectedDevicesForTest(devices);

    // 優先GPU(0)が利用不可の場合は空きメモリ最大のGPUを選択
    auto selected = detector.selectGpu(0);
    ASSERT_TRUE(selected.has_value());
    EXPECT_EQ(selected.value(), 1);
}

}  // namespace
