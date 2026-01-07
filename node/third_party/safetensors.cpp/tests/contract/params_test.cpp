/**
 * @file params_test.cpp
 * @brief Contract tests for parameter functions
 */

#include <gtest/gtest.h>
#include "safetensors.h"

class ParamsTest : public ::testing::Test {
protected:
    void SetUp() override {
        stcpp_init();
    }

    void TearDown() override {
        stcpp_free();
    }
};

// Context params tests

TEST_F(ParamsTest, ContextDefaultParamsAreValid) {
    stcpp_context_params params = stcpp_context_default_params();

    EXPECT_GT(params.n_ctx, 0);
    EXPECT_GT(params.n_batch, 0);
}

TEST_F(ParamsTest, ContextDefaultContextSize) {
    stcpp_context_params params = stcpp_context_default_params();
    EXPECT_EQ(params.n_ctx, 2048);
}

TEST_F(ParamsTest, ContextDefaultBatchSize) {
    stcpp_context_params params = stcpp_context_default_params();
    EXPECT_EQ(params.n_batch, 512);
}

TEST_F(ParamsTest, ContextDefaultMmapEnabled) {
    stcpp_context_params params = stcpp_context_default_params();
    EXPECT_TRUE(params.use_mmap);
}

TEST_F(ParamsTest, ContextDefaultKvQuantDisabled) {
    stcpp_context_params params = stcpp_context_default_params();
    EXPECT_FALSE(params.kv_cache_quant);
}

TEST_F(ParamsTest, ContextDefaultGpuLayersAll) {
    stcpp_context_params params = stcpp_context_default_params();
    EXPECT_EQ(params.n_gpu_layers, -1);  // -1 means all layers
}

TEST_F(ParamsTest, ContextDefaultDeviceIdZero) {
    stcpp_context_params params = stcpp_context_default_params();
    EXPECT_EQ(params.device_id, 0);
}

// Sampling params tests

TEST_F(ParamsTest, SamplingDefaultParamsAreValid) {
    stcpp_sampling_params params = stcpp_sampling_default_params();

    EXPECT_GT(params.temperature, 0.0f);
    EXPECT_GT(params.top_p, 0.0f);
    EXPECT_LE(params.top_p, 1.0f);
}

TEST_F(ParamsTest, SamplingDefaultTemperature) {
    stcpp_sampling_params params = stcpp_sampling_default_params();
    EXPECT_FLOAT_EQ(params.temperature, 1.0f);
}

TEST_F(ParamsTest, SamplingDefaultTopP) {
    stcpp_sampling_params params = stcpp_sampling_default_params();
    EXPECT_FLOAT_EQ(params.top_p, 1.0f);
}

TEST_F(ParamsTest, SamplingDefaultTopKDisabled) {
    stcpp_sampling_params params = stcpp_sampling_default_params();
    EXPECT_EQ(params.top_k, -1);  // -1 means disabled
}

TEST_F(ParamsTest, SamplingDefaultMinP) {
    stcpp_sampling_params params = stcpp_sampling_default_params();
    EXPECT_FLOAT_EQ(params.min_p, 0.0f);
}

TEST_F(ParamsTest, SamplingDefaultRepeatPenalty) {
    stcpp_sampling_params params = stcpp_sampling_default_params();
    EXPECT_FLOAT_EQ(params.repeat_penalty, 1.0f);
}

TEST_F(ParamsTest, SamplingDefaultPresencePenalty) {
    stcpp_sampling_params params = stcpp_sampling_default_params();
    EXPECT_FLOAT_EQ(params.presence_penalty, 0.0f);
}

TEST_F(ParamsTest, SamplingDefaultFrequencyPenalty) {
    stcpp_sampling_params params = stcpp_sampling_default_params();
    EXPECT_FLOAT_EQ(params.frequency_penalty, 0.0f);
}

TEST_F(ParamsTest, SamplingDefaultSeedRandom) {
    stcpp_sampling_params params = stcpp_sampling_default_params();
    EXPECT_EQ(params.seed, -1);  // -1 means random
}

// Backend tests

TEST_F(ParamsTest, BackendCountIsPositive) {
    int32_t count = stcpp_n_backends();
    EXPECT_GT(count, 0);
}

TEST_F(ParamsTest, BackendNameNotNull) {
    const char* name = stcpp_backend_name(STCPP_BACKEND_METAL);
    EXPECT_NE(name, nullptr);
    EXPECT_STREQ(name, "Metal");
}

TEST_F(ParamsTest, BackendNameCuda) {
    const char* name = stcpp_backend_name(STCPP_BACKEND_CUDA);
    EXPECT_STREQ(name, "CUDA");
}

TEST_F(ParamsTest, BackendNameRocm) {
    const char* name = stcpp_backend_name(STCPP_BACKEND_ROCM);
    EXPECT_STREQ(name, "ROCm");
}

TEST_F(ParamsTest, BackendNameVulkan) {
    const char* name = stcpp_backend_name(STCPP_BACKEND_VULKAN);
    EXPECT_STREQ(name, "Vulkan");
}

TEST_F(ParamsTest, DeviceCountIsPositive) {
    stcpp_backend_type backend = stcpp_backend_type_at(0);
    int32_t count = stcpp_n_devices(backend);
    EXPECT_GT(count, 0);
}

TEST_F(ParamsTest, DeviceNameNotNull) {
    stcpp_backend_type backend = stcpp_backend_type_at(0);
    const char* name = stcpp_device_name(backend, 0);
    EXPECT_NE(name, nullptr);
}
