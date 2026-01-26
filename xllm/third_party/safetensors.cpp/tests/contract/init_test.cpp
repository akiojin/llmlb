/**
 * @file init_test.cpp
 * @brief Contract tests for initialization/cleanup functions
 */

#include <gtest/gtest.h>
#include "safetensors.h"

class InitializationTest : public ::testing::Test {
protected:
    void SetUp() override {
        // Ensure clean state before each test
    }

    void TearDown() override {
        // Cleanup after each test
        stcpp_free();
    }
};

TEST_F(InitializationTest, InitSucceeds) {
    // Test that stcpp_init() can be called without errors
    EXPECT_NO_THROW(stcpp_init());
}

TEST_F(InitializationTest, InitIsIdempotent) {
    // Multiple init calls should not cause issues
    stcpp_init();
    EXPECT_NO_THROW(stcpp_init());
}

TEST_F(InitializationTest, FreeSucceeds) {
    stcpp_init();
    EXPECT_NO_THROW(stcpp_free());
}

TEST_F(InitializationTest, FreeIsIdempotent) {
    stcpp_init();
    stcpp_free();
    // Second free should not crash
    EXPECT_NO_THROW(stcpp_free());
}

TEST_F(InitializationTest, FreeWithoutInitSucceeds) {
    // Free without init should not crash
    EXPECT_NO_THROW(stcpp_free());
}

TEST_F(InitializationTest, ReinitializationWorks) {
    stcpp_init();
    stcpp_free();
    // Should be able to reinitialize
    EXPECT_NO_THROW(stcpp_init());
}

TEST_F(InitializationTest, LogCallbackCanBeSet) {
    stcpp_init();

    bool callback_called = false;
    auto callback = [](stcpp_log_level level, const char* msg, void* data) {
        (void)level;
        (void)msg;
        *static_cast<bool*>(data) = true;
    };

    EXPECT_NO_THROW(stcpp_set_log_callback(callback, &callback_called));
}

TEST_F(InitializationTest, LogCallbackCanBeCleared) {
    stcpp_init();
    stcpp_set_log_callback(nullptr, nullptr);
    // Should not crash
}

TEST_F(InitializationTest, LogLevelCanBeSet) {
    stcpp_init();

    EXPECT_NO_THROW(stcpp_set_log_level(STCPP_LOG_DEBUG));
    EXPECT_NO_THROW(stcpp_set_log_level(STCPP_LOG_INFO));
    EXPECT_NO_THROW(stcpp_set_log_level(STCPP_LOG_WARN));
    EXPECT_NO_THROW(stcpp_set_log_level(STCPP_LOG_ERROR));
}
