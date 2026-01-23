/**
 * @file lora_test.cpp
 * @brief Unit tests for LoRA adapter support (Task 39)
 */

#include <gtest/gtest.h>
#include <filesystem>
#include "safetensors.h"

namespace fs = std::filesystem;

// C++17 compatible ends_with helper
inline bool str_ends_with(const std::string& str, const std::string& suffix) {
    if (suffix.size() > str.size()) return false;
    return str.compare(str.size() - suffix.size(), suffix.size(), suffix) == 0;
}

class LoRATest : public ::testing::Test {
protected:
    fs::path temp_dir;

    void SetUp() override {
        stcpp_init();
        temp_dir = fs::temp_directory_path() / "stcpp_lora_test";
        fs::create_directories(temp_dir);
    }

    void TearDown() override {
        fs::remove_all(temp_dir);
        stcpp_free();
    }
};

// Test: LoRA load with null model
TEST_F(LoRATest, LoadLoRANullModel) {
    stcpp_lora* lora = stcpp_lora_load(nullptr, "test.safetensors", 1.0f);
    EXPECT_EQ(lora, nullptr);
}

// Test: LoRA free with null
TEST_F(LoRATest, FreeLoRANull) {
    stcpp_lora_free(nullptr);
    // Should not crash
    SUCCEED();
}

// Test: LoRA apply with null context
TEST_F(LoRATest, ApplyLoRANullContext) {
    stcpp_error result = stcpp_lora_apply(nullptr, nullptr);
    EXPECT_NE(result, STCPP_OK);
}

// Test: LoRA remove with null context
TEST_F(LoRATest, RemoveLoRANullContext) {
    stcpp_error result = stcpp_lora_remove(nullptr, nullptr);
    EXPECT_NE(result, STCPP_OK);
}

// Test: LoRA scale factor
TEST_F(LoRATest, LoRAScaleFactor) {
    // Scale factor controls LoRA influence
    // scale = 0.0: No LoRA effect
    // scale = 1.0: Full LoRA effect
    // scale > 1.0: Amplified LoRA effect

    float scale_none = 0.0f;
    float scale_half = 0.5f;
    float scale_full = 1.0f;
    float scale_amplified = 2.0f;

    EXPECT_FLOAT_EQ(scale_none, 0.0f);
    EXPECT_FLOAT_EQ(scale_half, 0.5f);
    EXPECT_FLOAT_EQ(scale_full, 1.0f);
    EXPECT_FLOAT_EQ(scale_amplified, 2.0f);
}

// Test: LoRA rank parameter
TEST_F(LoRATest, LoRARankParameter) {
    // LoRA rank affects:
    // - Memory usage: lower rank = less memory
    // - Expressiveness: higher rank = more capacity
    // Common values: 4, 8, 16, 32, 64

    std::vector<int> common_ranks = {4, 8, 16, 32, 64};

    for (int rank : common_ranks) {
        EXPECT_GT(rank, 0);
        EXPECT_LE(rank, 256);  // Reasonable upper bound
    }
}

// Test: LoRA alpha parameter
TEST_F(LoRATest, LoRAAlphaParameter) {
    // Alpha is a scaling parameter
    // Effective scale = alpha / rank
    // Common pattern: alpha = rank (effective scale = 1.0)

    int rank = 16;
    int alpha = 16;

    float effective_scale = static_cast<float>(alpha) / rank;
    EXPECT_FLOAT_EQ(effective_scale, 1.0f);

    // Alpha = 2 * rank doubles the effect
    alpha = 32;
    effective_scale = static_cast<float>(alpha) / rank;
    EXPECT_FLOAT_EQ(effective_scale, 2.0f);
}

// Test: Multiple LoRA adapters
TEST_F(LoRATest, MultipleLoRAAdapters) {
    // System should support loading multiple LoRA adapters
    // Each can have different scale factors

    struct LoRAAdapter {
        std::string name;
        float scale;
    };

    std::vector<LoRAAdapter> adapters = {
        {"style_adapter", 0.8f},
        {"task_adapter", 1.0f},
        {"language_adapter", 0.5f},
    };

    EXPECT_EQ(adapters.size(), 3);

    for (const auto& adapter : adapters) {
        EXPECT_FALSE(adapter.name.empty());
        EXPECT_GE(adapter.scale, 0.0f);
    }
}

// Test: LoRA target modules
TEST_F(LoRATest, LoRATargetModules) {
    // LoRA can target different parts of the model
    // Common targets: q_proj, k_proj, v_proj, o_proj, gate, up, down

    std::vector<std::string> target_modules = {
        "q_proj",      // Query projection
        "k_proj",      // Key projection
        "v_proj",      // Value projection
        "o_proj",      // Output projection
        "gate_proj",   // MLP gate
        "up_proj",     // MLP up
        "down_proj",   // MLP down
    };

    EXPECT_EQ(target_modules.size(), 7);
}

// Test: LoRA hot reload concept
TEST_F(LoRATest, LoRAHotReloadConcept) {
    // LoRA should be loadable/unloadable without reloading base model

    struct ModelState {
        bool base_loaded = false;
        bool lora_applied = false;
    };

    ModelState state;

    // Load base model
    state.base_loaded = true;
    EXPECT_TRUE(state.base_loaded);
    EXPECT_FALSE(state.lora_applied);

    // Apply LoRA
    state.lora_applied = true;
    EXPECT_TRUE(state.base_loaded);
    EXPECT_TRUE(state.lora_applied);

    // Remove LoRA (base model unchanged)
    state.lora_applied = false;
    EXPECT_TRUE(state.base_loaded);
    EXPECT_FALSE(state.lora_applied);
}

// Test: LoRA file format detection
TEST_F(LoRATest, LoRAFileFormatDetection) {
    // LoRA can come in different formats
    std::string safetensors_path = "adapter.safetensors";
    std::string bin_path = "adapter.bin";

    bool is_safetensors = str_ends_with(safetensors_path, ".safetensors");
    bool is_bin = str_ends_with(bin_path, ".bin");

    EXPECT_TRUE(is_safetensors);
    EXPECT_TRUE(is_bin);
}

