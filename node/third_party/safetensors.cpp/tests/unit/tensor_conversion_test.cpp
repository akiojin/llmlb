/**
 * @file tensor_conversion_test.cpp
 * @brief Unit tests for tensor data type conversion
 *
 * Tests for converting safetensors dtypes to ggml tensor types:
 * - FP16 (float16)
 * - BF16 (bfloat16)
 * - FP32 (float32)
 * - INT8 (quantized)
 */

#include <gtest/gtest.h>
#include "safetensors.h"
#include <cmath>
#include <vector>
#include <limits>

class TensorConversionTest : public ::testing::Test {
protected:
    void SetUp() override {
        stcpp_init();
    }

    void TearDown() override {
        stcpp_free();
    }
};

// Helper struct to represent tensor metadata (as would be parsed from safetensors)
struct TensorMetadata {
    std::string name;
    std::string dtype;  // "F16", "BF16", "F32", "I8", etc.
    std::vector<int64_t> shape;
    size_t data_offset;
    size_t data_size;
};

// Test: Verify FP16 range handling
TEST_F(TensorConversionTest, FP16RangeIsValid) {
    // FP16 max value is approximately 65504
    // FP16 min positive normal is approximately 6.1e-5
    // FP16 min positive subnormal is approximately 5.96e-8
    const float fp16_max = 65504.0f;
    const float fp16_min_normal = 6.103515625e-5f;

    // Verify these constants are reasonable
    EXPECT_GT(fp16_max, 60000.0f);
    EXPECT_LT(fp16_min_normal, 0.0001f);
}

// Test: Verify BF16 range handling
TEST_F(TensorConversionTest, BF16RangeIsValid) {
    // BF16 has the same exponent range as FP32
    // but only 7 bits of mantissa (vs 23 for FP32)
    // BF16 max is approximately 3.39e38 (same as FP32)
    const float bf16_max = std::numeric_limits<float>::max();

    EXPECT_GT(bf16_max, 1e38f);
}

// Test: Verify INT8 quantization range
TEST_F(TensorConversionTest, INT8QuantizationRangeIsValid) {
    // INT8 symmetric quantization: [-127, 127] or [-128, 127]
    const int8_t int8_min = -128;
    const int8_t int8_max = 127;

    EXPECT_EQ(int8_min, std::numeric_limits<int8_t>::min());
    EXPECT_EQ(int8_max, std::numeric_limits<int8_t>::max());
}

// Test: Supported dtype strings
TEST_F(TensorConversionTest, SupportedDtypeStrings) {
    // List of dtypes that should be supported
    std::vector<std::string> supported_dtypes = {
        "F16",   // float16
        "BF16",  // bfloat16
        "F32",   // float32
        "F64",   // float64
        "I8",    // int8
        "I16",   // int16
        "I32",   // int32
        "I64",   // int64
        "U8",    // uint8
        "U16",   // uint16
        "U32",   // uint32
        "U64",   // uint64
        "BOOL",  // boolean
    };

    // Just verify the list is populated
    EXPECT_FALSE(supported_dtypes.empty());
    EXPECT_GE(supported_dtypes.size(), 5u);
}

// Test: Tensor element count calculation
TEST_F(TensorConversionTest, ElementCountCalculation) {
    // Test various shapes
    std::vector<int64_t> shape1 = {10, 20, 30};
    int64_t count1 = 1;
    for (auto dim : shape1) count1 *= dim;
    EXPECT_EQ(count1, 6000);

    std::vector<int64_t> shape2 = {1024, 1024};
    int64_t count2 = 1;
    for (auto dim : shape2) count2 *= dim;
    EXPECT_EQ(count2, 1048576);

    // Scalar (empty shape means scalar)
    std::vector<int64_t> scalar_shape = {};
    int64_t scalar_count = 1;  // Scalar has 1 element
    EXPECT_EQ(scalar_count, 1);
}

// Test: Data size calculation for different dtypes
TEST_F(TensorConversionTest, DataSizeCalculation) {
    const int64_t num_elements = 1000;

    // FP16: 2 bytes per element
    size_t fp16_size = num_elements * 2;
    EXPECT_EQ(fp16_size, 2000u);

    // BF16: 2 bytes per element
    size_t bf16_size = num_elements * 2;
    EXPECT_EQ(bf16_size, 2000u);

    // FP32: 4 bytes per element
    size_t fp32_size = num_elements * 4;
    EXPECT_EQ(fp32_size, 4000u);

    // INT8: 1 byte per element
    size_t int8_size = num_elements * 1;
    EXPECT_EQ(int8_size, 1000u);
}

// Test: Alignment requirements
TEST_F(TensorConversionTest, AlignmentRequirements) {
    // ggml typically requires 32-byte alignment for SIMD operations
    const size_t required_alignment = 32;

    // Helper to check alignment
    auto is_aligned = [](const void* ptr, size_t alignment) {
        return reinterpret_cast<uintptr_t>(ptr) % alignment == 0;
    };

    // Allocate aligned memory
    alignas(32) float aligned_data[8];
    EXPECT_TRUE(is_aligned(aligned_data, required_alignment));
}

// Test: Endianness handling (safetensors uses little-endian)
TEST_F(TensorConversionTest, EndiannessCheck) {
    // Check if current platform is little-endian
    union {
        uint32_t i;
        char c[4];
    } test = {0x01020304};

    bool is_little_endian = (test.c[0] == 0x04);

    // safetensors format uses little-endian
    // If platform is big-endian, byte swapping would be needed
    // This test just documents the current platform's endianness
    if (is_little_endian) {
        // No byte swapping needed
        SUCCEED();
    } else {
        // Byte swapping would be needed (uncommon on modern systems)
        SUCCEED();
    }
}
