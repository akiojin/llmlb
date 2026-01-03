#include <gtest/gtest.h>
#include <chrono>

#include "system/resource_monitor.h"

using namespace llm_node;

TEST(ResourceMonitorTest, DoesNotEvictBelowThreshold) {
    ResourceUsage usage{50, 100, 10, 100};
    int evict_calls = 0;

    ResourceMonitor monitor(
        [&usage]() { return usage; },
        [&evict_calls]() {
            evict_calls++;
            return true;
        },
        std::chrono::milliseconds(0),
        0.9);

    monitor.pollOnce();

    EXPECT_EQ(evict_calls, 0);
    auto latest = monitor.latestUsage();
    EXPECT_EQ(latest.mem_used_bytes, 50u);
    EXPECT_EQ(latest.mem_total_bytes, 100u);
}

TEST(ResourceMonitorTest, EvictsWhenRamAboveThreshold) {
    ResourceUsage usage{95, 100, 10, 100};
    int evict_calls = 0;

    ResourceMonitor monitor(
        [&usage]() { return usage; },
        [&usage, &evict_calls]() {
            evict_calls++;
            usage.mem_used_bytes = 40;
            return true;
        },
        std::chrono::milliseconds(0),
        0.9);

    monitor.pollOnce();

    EXPECT_EQ(evict_calls, 1);
}

TEST(ResourceMonitorTest, EvictsWhenVramAboveThreshold) {
    ResourceUsage usage{10, 100, 95, 100};
    int evict_calls = 0;

    ResourceMonitor monitor(
        [&usage]() { return usage; },
        [&usage, &evict_calls]() {
            evict_calls++;
            usage.vram_used_bytes = 20;
            return true;
        },
        std::chrono::milliseconds(0),
        0.9);

    monitor.pollOnce();

    EXPECT_EQ(evict_calls, 1);
}
