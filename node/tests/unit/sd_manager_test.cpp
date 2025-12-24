#include "core/sd_manager.h"

#include <gtest/gtest.h>

#include <vector>

namespace llm_node {

TEST(SDManagerTest, ToMaskChannelConvertsRgbToGray) {
    const int width = 2;
    const int height = 1;
    const std::vector<uint8_t> rgb = {0, 0, 0, 255, 255, 255};

    const auto mask = SDManager::toMaskChannel(rgb, width, height);

    ASSERT_EQ(mask.size(), static_cast<size_t>(width * height));
    EXPECT_EQ(mask[0], 0u);
    EXPECT_EQ(mask[1], 255u);
}

TEST(SDManagerTest, ToMaskChannelReturnsEmptyForInvalidDimensions) {
    const std::vector<uint8_t> rgb = {0, 0, 0};

    EXPECT_TRUE(SDManager::toMaskChannel(rgb, 0, 1).empty());
    EXPECT_TRUE(SDManager::toMaskChannel(rgb, 1, 0).empty());
    EXPECT_TRUE(SDManager::toMaskChannel(rgb, -1, 1).empty());
}

TEST(SDManagerTest, ToMaskChannelReturnsEmptyForInsufficientData) {
    const std::vector<uint8_t> rgb = {0, 0};

    EXPECT_TRUE(SDManager::toMaskChannel(rgb, 1, 1).empty());
}

TEST(SDManagerTest, MakeSolidMaskFillsValue) {
    const int width = 3;
    const int height = 2;
    const uint8_t value = 42;

    const auto mask = SDManager::makeSolidMask(width, height, value);

    ASSERT_EQ(mask.size(), static_cast<size_t>(width * height));
    for (auto entry : mask) {
        EXPECT_EQ(entry, value);
    }
}

TEST(SDManagerTest, MakeSolidMaskReturnsEmptyForInvalidDimensions) {
    EXPECT_TRUE(SDManager::makeSolidMask(0, 1, 42).empty());
    EXPECT_TRUE(SDManager::makeSolidMask(1, 0, 42).empty());
    EXPECT_TRUE(SDManager::makeSolidMask(-1, 1, 42).empty());
}

}  // namespace llm_node
