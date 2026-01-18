// SPEC-58378000: ProgressRenderer unit tests

#include <gtest/gtest.h>
#include "cli/progress_renderer.h"
#include <sstream>

using namespace allm::cli;

// Test basic progress rendering
TEST(ProgressRendererTest, ConstructorWithTotal) {
    ProgressRenderer renderer(1000);
    // Default state should have no phase and zero progress
    EXPECT_NO_THROW(renderer.update(0, 0.0));
}

TEST(ProgressRendererTest, DefaultConstructor) {
    ProgressRenderer renderer;
    EXPECT_NO_THROW(renderer.update(0, 0.0));
}

TEST(ProgressRendererTest, SetPhase) {
    ProgressRenderer renderer;
    renderer.setPhase("downloading");
    // No exception should be thrown
}

TEST(ProgressRendererTest, UpdateProgress) {
    ProgressRenderer renderer(1000);
    renderer.update(500, 100.0);  // 50%, 100 bytes/sec
    EXPECT_NO_THROW(renderer.update(500, 100.0));
}

TEST(ProgressRendererTest, CompleteProgress) {
    ProgressRenderer renderer(1000);
    renderer.complete();
    // Should not throw
}

TEST(ProgressRendererTest, FailProgress) {
    ProgressRenderer renderer(1000);
    renderer.fail("Download failed");
    // Should not throw
}

// Test format functions
TEST(ProgressRendererTest, FormatBytesSmall) {
    std::string result = ProgressRenderer::formatBytes(512);
    EXPECT_FALSE(result.empty());
}

TEST(ProgressRendererTest, FormatBytesKilobytes) {
    std::string result = ProgressRenderer::formatBytes(1024);
    EXPECT_FALSE(result.empty());
}

TEST(ProgressRendererTest, FormatBytesMegabytes) {
    std::string result = ProgressRenderer::formatBytes(1024 * 1024);
    EXPECT_FALSE(result.empty());
}

TEST(ProgressRendererTest, FormatBytesGigabytes) {
    std::string result = ProgressRenderer::formatBytes(1024ULL * 1024 * 1024);
    EXPECT_FALSE(result.empty());
    // Should contain GB
    EXPECT_TRUE(result.find("GB") != std::string::npos || result.find("G") != std::string::npos);
}

TEST(ProgressRendererTest, FormatSpeedSmall) {
    std::string result = ProgressRenderer::formatSpeed(512);
    EXPECT_FALSE(result.empty());
}

TEST(ProgressRendererTest, FormatSpeedMegabytesPerSec) {
    std::string result = ProgressRenderer::formatSpeed(1024.0 * 1024.0);
    EXPECT_FALSE(result.empty());
}

TEST(ProgressRendererTest, FormatDurationSeconds) {
    std::string result = ProgressRenderer::formatDuration(30.0);
    EXPECT_FALSE(result.empty());
    EXPECT_TRUE(result.find("s") != std::string::npos);
}

TEST(ProgressRendererTest, FormatDurationMinutes) {
    std::string result = ProgressRenderer::formatDuration(90.0);
    EXPECT_FALSE(result.empty());
    EXPECT_TRUE(result.find("m") != std::string::npos);
}

TEST(ProgressRendererTest, FormatProgressBarZero) {
    std::string result = ProgressRenderer::formatProgressBar(0, 100);
    EXPECT_FALSE(result.empty());
}

TEST(ProgressRendererTest, FormatProgressBarHalf) {
    std::string result = ProgressRenderer::formatProgressBar(50, 100);
    EXPECT_FALSE(result.empty());
}

TEST(ProgressRendererTest, FormatProgressBarFull) {
    std::string result = ProgressRenderer::formatProgressBar(100, 100);
    EXPECT_FALSE(result.empty());
}
