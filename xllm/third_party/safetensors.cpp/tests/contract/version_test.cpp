/**
 * @file version_test.cpp
 * @brief Contract tests for version functions
 */

#include <gtest/gtest.h>
#include <cstring>
#include "safetensors.h"

class VersionTest : public ::testing::Test {
protected:
    void SetUp() override {
        stcpp_init();
    }

    void TearDown() override {
        stcpp_free();
    }
};

TEST_F(VersionTest, VersionStringNotNull) {
    const char* version = stcpp_version();
    EXPECT_NE(version, nullptr);
}

TEST_F(VersionTest, VersionStringNotEmpty) {
    const char* version = stcpp_version();
    ASSERT_NE(version, nullptr);
    EXPECT_GT(strlen(version), 0u);
}

TEST_F(VersionTest, VersionMatchesMacros) {
    const char* version = stcpp_version();
    ASSERT_NE(version, nullptr);

    // Expected format: "0.1.0"
    char expected[32];
    snprintf(expected, sizeof(expected), "%d.%d.%d",
             STCPP_VERSION_MAJOR,
             STCPP_VERSION_MINOR,
             STCPP_VERSION_PATCH);

    EXPECT_STREQ(version, expected);
}

TEST_F(VersionTest, AbiVersionMatchesMacro) {
    int32_t abi = stcpp_abi_version();
    EXPECT_EQ(abi, STCPP_ABI_VERSION);
}

TEST_F(VersionTest, AbiVersionIsPositive) {
    int32_t abi = stcpp_abi_version();
    EXPECT_GT(abi, 0);
}

TEST_F(VersionTest, VersionIsStableAcrossCalls) {
    const char* v1 = stcpp_version();
    const char* v2 = stcpp_version();

    // Should return the same pointer
    EXPECT_EQ(v1, v2);
}
