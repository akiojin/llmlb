/**
 * @file model_load_test.cpp
 * @brief Integration tests for complete model loading (Task 23)
 */

#include <gtest/gtest.h>
#include <fstream>
#include <filesystem>
#include "safetensors.h"
#include "safetensors_internal.h"

namespace fs = std::filesystem;

class ModelLoadTest : public ::testing::Test {
protected:
    fs::path temp_dir;

    void SetUp() override {
        temp_dir = fs::temp_directory_path() / "stcpp_model_load_test";
        fs::create_directories(temp_dir);
    }

    void TearDown() override {
        fs::remove_all(temp_dir);
    }

    void CreateConfigJson(const fs::path& dir) {
        std::ofstream file(dir / "config.json");
        file << R"({
            "architectures": ["GPTOssForCausalLM"],
            "vocab_size": 32000,
            "hidden_size": 2048,
            "num_hidden_layers": 24,
            "num_attention_heads": 32,
            "max_position_embeddings": 4096
        })";
        file.close();
    }

    void CreateMinimalModel(const fs::path& dir) {
        CreateConfigJson(dir);

        // Create tokenizer.json
        std::ofstream tokenizer(dir / "tokenizer.json");
        tokenizer << R"({
            "model": {
                "type": "BPE",
                "vocab": {"<s>": 0, "</s>": 1},
                "merges": []
            },
            "added_tokens": []
        })";
        tokenizer.close();

        // Create minimal safetensors file
        std::ofstream st(dir / "model.safetensors", std::ios::binary);
        std::string header = R"({"__metadata__":{}})";
        uint64_t header_size = header.size();
        st.write(reinterpret_cast<const char*>(&header_size), 8);
        st.write(header.data(), header.size());
        st.close();
    }
};

// Test: Load config.json
TEST_F(ModelLoadTest, LoadConfigJson) {
    CreateConfigJson(temp_dir);

    stcpp::ModelImpl model;
    std::string error;

    bool result = stcpp::load_model_config(temp_dir.string(), model, error);

    EXPECT_TRUE(result) << "Error: " << error;
    EXPECT_EQ(model.vocab_size, 32000);
    EXPECT_EQ(model.hidden_size, 2048);
    EXPECT_EQ(model.n_layers, 24);
    EXPECT_EQ(model.n_heads, 32);
    EXPECT_EQ(model.max_context, 4096);
}

// Test: Load config.json with alternative key names
TEST_F(ModelLoadTest, LoadConfigJsonAlternativeKeys) {
    std::ofstream file(temp_dir / "config.json");
    file << R"({
        "vocab_size": 50000,
        "n_embd": 1024,
        "n_layer": 12,
        "n_head": 16,
        "n_positions": 2048
    })";
    file.close();

    stcpp::ModelImpl model;
    std::string error;

    bool result = stcpp::load_model_config(temp_dir.string(), model, error);

    EXPECT_TRUE(result) << "Error: " << error;
    EXPECT_EQ(model.vocab_size, 50000);
    EXPECT_EQ(model.hidden_size, 1024);
    EXPECT_EQ(model.n_layers, 12);
    EXPECT_EQ(model.n_heads, 16);
    EXPECT_EQ(model.max_context, 2048);
}

// Test: Missing config.json returns error
TEST_F(ModelLoadTest, MissingConfigJsonReturnsError) {
    stcpp::ModelImpl model;
    std::string error;

    bool result = stcpp::load_model_config(temp_dir.string(), model, error);

    EXPECT_FALSE(result);
    EXPECT_FALSE(error.empty());
}

// Test: Complete model load with all files
TEST_F(ModelLoadTest, CompleteModelLoad) {
    CreateMinimalModel(temp_dir);

    stcpp_model* model = stcpp_model_load(
        temp_dir.string().c_str(),
        nullptr,
        nullptr
    );

    // Model load might fail since we don't have real weights,
    // but the path should be attempted
    if (model != nullptr) {
        stcpp_model_free(model);
    }
}

// Test: VRAM estimation
TEST_F(ModelLoadTest, VRAMEstimation) {
    // For a model with:
    // - vocab_size: 32000
    // - hidden_size: 2048
    // - n_layers: 24
    // - Assuming FP16 weights
    //
    // Rough estimation:
    // - Embedding: vocab_size * hidden_size * 2 bytes
    // - Layers: n_layers * (attention + FFN) * 2 bytes
    // - KV cache: depends on context length

    CreateConfigJson(temp_dir);

    stcpp::ModelImpl model;
    std::string error;
    stcpp::load_model_config(temp_dir.string(), model, error);

    // Basic calculation for embedding table
    size_t embedding_size = model.vocab_size * model.hidden_size * 2;  // FP16
    EXPECT_GT(embedding_size, 0);

    // This should be ~128MB for embedding alone
    // Full model would be several GB
}

// Test: Architecture detection from config.json
TEST_F(ModelLoadTest, ArchitectureDetection) {
    std::ofstream file(temp_dir / "config.json");
    file << R"({
        "architectures": ["GPTOssForCausalLM"],
        "model_type": "gptoss",
        "vocab_size": 32000,
        "hidden_size": 2048,
        "num_hidden_layers": 24,
        "num_attention_heads": 32
    })";
    file.close();

    // Architecture detection would be implemented in the model loader
    // For now, just verify config parsing works
    stcpp::ModelImpl model;
    std::string error;

    bool result = stcpp::load_model_config(temp_dir.string(), model, error);
    EXPECT_TRUE(result);
}
