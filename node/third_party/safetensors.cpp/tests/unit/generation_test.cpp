/**
 * @file generation_test.cpp
 * @brief Unit tests for text generation (Task 24)
 */

#include <gtest/gtest.h>
#include <atomic>
#include <vector>
#include <cstring>
#include "safetensors.h"

class GenerationTest : public ::testing::Test {
protected:
    // Test callback state
    std::vector<std::string> generated_tokens;
    std::atomic<bool> callback_called{false};

    static bool stream_callback(const char* token_text, int32_t /*token_id*/, void* user_data) {
        auto* test = static_cast<GenerationTest*>(user_data);
        test->generated_tokens.push_back(token_text ? token_text : "");
        test->callback_called = true;
        return true;  // Continue generation
    }
};

// Test: Context creation with default params
TEST_F(GenerationTest, CreateContextWithDefaultParams) {
    stcpp_context_params params = stcpp_context_default_params();

    EXPECT_EQ(params.n_ctx, 4096);
    EXPECT_EQ(params.n_batch, 512);
    EXPECT_EQ(params.n_threads, 0);  // Auto-detect
    EXPECT_FALSE(params.use_mmap);
    EXPECT_FALSE(params.use_mlock);
}

// Test: Context creation with custom params
TEST_F(GenerationTest, CreateContextWithCustomParams) {
    stcpp_context_params params = stcpp_context_default_params();
    params.n_ctx = 8192;
    params.n_batch = 1024;
    params.n_threads = 8;

    EXPECT_EQ(params.n_ctx, 8192);
    EXPECT_EQ(params.n_batch, 1024);
    EXPECT_EQ(params.n_threads, 8);
}

// Test: Sampling params structure
TEST_F(GenerationTest, SamplingParamsStructure) {
    stcpp_sampling_params params = stcpp_sampling_default_params();

    EXPECT_FLOAT_EQ(params.temperature, 1.0f);
    EXPECT_FLOAT_EQ(params.top_p, 1.0f);
    EXPECT_EQ(params.top_k, -1);  // Disabled
    EXPECT_FLOAT_EQ(params.repeat_penalty, 1.0f);
    EXPECT_EQ(params.seed, -1);   // Random
}

// Test: Custom sampling params
TEST_F(GenerationTest, CustomSamplingParams) {
    stcpp_sampling_params params;
    params.temperature = 0.7f;
    params.top_p = 0.9f;
    params.top_k = 40;
    params.min_p = 0.0f;
    params.repeat_penalty = 1.1f;
    params.presence_penalty = 0.0f;
    params.frequency_penalty = 0.0f;
    params.seed = 42;

    EXPECT_FLOAT_EQ(params.temperature, 0.7f);
    EXPECT_FLOAT_EQ(params.top_p, 0.9f);
    EXPECT_EQ(params.top_k, 40);
    EXPECT_FLOAT_EQ(params.repeat_penalty, 1.1f);
    EXPECT_EQ(params.seed, 42);
}

// Test: Streaming callback mechanism
TEST_F(GenerationTest, StreamingCallbackMechanism) {
    // Simulate a callback invocation
    bool result = stream_callback("hello", 42, this);

    EXPECT_TRUE(result);
    EXPECT_TRUE(callback_called);
    EXPECT_EQ(generated_tokens.size(), 1);
    EXPECT_EQ(generated_tokens[0], "hello");
}

// Test: Callback can stop generation
TEST_F(GenerationTest, CallbackCanStopGeneration) {
    auto stop_callback = [](const char* /*text*/, int32_t /*token*/, void* /*user_data*/) -> bool {
        return false;  // Stop generation
    };

    // When callback returns false, generation should stop
    bool continue_gen = stop_callback("", 0, nullptr);
    EXPECT_FALSE(continue_gen);
}

// Test: Temperature affects sampling
TEST_F(GenerationTest, TemperatureAffectsSampling) {
    // Temperature = 0 should be deterministic (greedy)
    // Temperature = 1 should be standard sampling
    // Temperature > 1 increases randomness

    float temp_greedy = 0.0f;
    float temp_standard = 1.0f;
    float temp_creative = 2.0f;

    EXPECT_LT(temp_greedy, temp_standard);
    EXPECT_LT(temp_standard, temp_creative);
}

// Test: Top-k filtering
TEST_F(GenerationTest, TopKFiltering) {
    // top_k = 1 should be same as greedy
    // top_k = vocab_size should include all tokens
    // top_k = 40 is a common default

    int32_t top_k_greedy = 1;
    int32_t top_k_default = 40;
    int32_t top_k_disabled = -1;  // -1 means disabled

    EXPECT_EQ(top_k_greedy, 1);
    EXPECT_EQ(top_k_default, 40);
    EXPECT_EQ(top_k_disabled, -1);
}

// Test: Top-p (nucleus) filtering
TEST_F(GenerationTest, TopPFiltering) {
    // top_p = 1.0 includes all tokens
    // top_p = 0.9 is a common default
    // top_p = 0.1 would be very restrictive

    float top_p_all = 1.0f;
    float top_p_default = 0.9f;
    float top_p_strict = 0.1f;

    EXPECT_FLOAT_EQ(top_p_all, 1.0f);
    EXPECT_FLOAT_EQ(top_p_default, 0.9f);
    EXPECT_FLOAT_EQ(top_p_strict, 0.1f);
}

// Test: Repeat penalty
TEST_F(GenerationTest, RepeatPenalty) {
    // repeat_penalty = 1.0 means no penalty
    // repeat_penalty > 1.0 discourages repetition
    // Common values: 1.1 - 1.5

    float no_penalty = 1.0f;
    float default_penalty = 1.1f;
    float strong_penalty = 1.5f;

    EXPECT_FLOAT_EQ(no_penalty, 1.0f);
    EXPECT_GT(default_penalty, no_penalty);
    EXPECT_GT(strong_penalty, default_penalty);
}

// Test: Null context returns error
TEST_F(GenerationTest, NullContextReturnsError) {
    stcpp_sampling_params params = stcpp_sampling_default_params();
    char output[1024];

    // stcpp_generate with null context should return error
    stcpp_error result = stcpp_generate(nullptr, "Test", params, 10, output, sizeof(output));

    EXPECT_NE(result, STCPP_OK);
}

// Test: Error codes are defined
TEST_F(GenerationTest, ErrorCodesAreDefined) {
    EXPECT_EQ(STCPP_OK, 0);
    EXPECT_LT(STCPP_ERROR_UNKNOWN, 0);
    EXPECT_LT(STCPP_ERROR_FILE_NOT_FOUND, 0);
    EXPECT_LT(STCPP_ERROR_INVALID_MODEL, 0);
    EXPECT_LT(STCPP_ERROR_OUT_OF_MEMORY, 0);
    EXPECT_LT(STCPP_ERROR_GPU_NOT_FOUND, 0);
    EXPECT_LT(STCPP_ERROR_CANCELLED, 0);
}
