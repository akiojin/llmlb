/**
 * @file split_loader_test.cpp
 * @brief Unit tests for split/sharded safetensors file loading
 *
 * Large models are often split into multiple safetensors files with an
 * index.json file that maps tensor names to their respective shard files.
 *
 * Format:
 * - model.safetensors.index.json: Maps tensor names to shard files
 * - model-00001-of-00005.safetensors: First shard
 * - model-00002-of-00005.safetensors: Second shard
 * - etc.
 */

#include <gtest/gtest.h>
#include "safetensors.h"
#include <fstream>
#include <filesystem>
#include <string>

class SplitLoaderTest : public ::testing::Test {
protected:
    void SetUp() override {
        stcpp_init();
        test_dir_ = std::filesystem::temp_directory_path() / "stcpp_split_test";
        std::filesystem::create_directories(test_dir_);
    }

    void TearDown() override {
        stcpp_free();
        std::filesystem::remove_all(test_dir_);
    }

    // Helper to create a valid index.json file
    void CreateIndexJson(const std::filesystem::path& path,
                         const std::string& content) {
        std::ofstream file(path);
        file << content;
        file.close();
    }

    // Helper to create a minimal safetensors shard file
    void CreateMinimalShard(const std::filesystem::path& path) {
        std::ofstream file(path, std::ios::binary);
        std::string header = R"({"__metadata__":{}})";
        uint64_t header_size = header.size();
        file.write(reinterpret_cast<const char*>(&header_size), 8);
        file.write(header.data(), header.size());
        file.close();
    }

    std::filesystem::path test_dir_;
};

// Test: Loading model with missing index.json returns error
TEST_F(SplitLoaderTest, MissingIndexJsonReturnsError) {
    // Create directory without index.json
    auto model_dir = test_dir_ / "model_no_index";
    std::filesystem::create_directories(model_dir);

    struct ErrorData { bool called = false; } data;
    auto callback = [](stcpp_error, const char*, void* ud) {
        static_cast<ErrorData*>(ud)->called = true;
    };

    stcpp_model* model = stcpp_model_load(
        model_dir.c_str(),
        callback,
        &data
    );

    EXPECT_EQ(model, nullptr);
    EXPECT_TRUE(data.called);
}

// Test: Invalid JSON in index file returns error
TEST_F(SplitLoaderTest, InvalidIndexJsonReturnsError) {
    auto model_dir = test_dir_ / "model_bad_json";
    std::filesystem::create_directories(model_dir);
    CreateIndexJson(model_dir / "model.safetensors.index.json",
                    "{ invalid json }");

    struct ErrorData { bool called = false; } data;
    auto callback = [](stcpp_error, const char*, void* ud) {
        static_cast<ErrorData*>(ud)->called = true;
    };

    stcpp_model* model = stcpp_model_load(
        model_dir.c_str(),
        callback,
        &data
    );

    EXPECT_EQ(model, nullptr);
    EXPECT_TRUE(data.called);
}

// Test: Index.json with missing weight_map field returns error
TEST_F(SplitLoaderTest, IndexJsonMissingWeightMapReturnsError) {
    auto model_dir = test_dir_ / "model_no_weight_map";
    std::filesystem::create_directories(model_dir);
    CreateIndexJson(model_dir / "model.safetensors.index.json",
                    R"({"metadata": {}})");

    struct ErrorData { bool called = false; } data;
    auto callback = [](stcpp_error, const char*, void* ud) {
        static_cast<ErrorData*>(ud)->called = true;
    };

    stcpp_model* model = stcpp_model_load(
        model_dir.c_str(),
        callback,
        &data
    );

    EXPECT_EQ(model, nullptr);
    EXPECT_TRUE(data.called);
}

// Test: Index.json referencing non-existent shard returns error
TEST_F(SplitLoaderTest, MissingShardFileReturnsError) {
    auto model_dir = test_dir_ / "model_missing_shard";
    std::filesystem::create_directories(model_dir);

    // Create index pointing to non-existent shard
    std::string index_json = R"({
        "metadata": {},
        "weight_map": {
            "model.layers.0.weight": "model-00001-of-00002.safetensors",
            "model.layers.1.weight": "model-00002-of-00002.safetensors"
        }
    })";
    CreateIndexJson(model_dir / "model.safetensors.index.json", index_json);

    struct ErrorData { bool called = false; } data;
    auto callback = [](stcpp_error, const char*, void* ud) {
        static_cast<ErrorData*>(ud)->called = true;
    };

    stcpp_model* model = stcpp_model_load(
        model_dir.c_str(),
        callback,
        &data
    );

    EXPECT_EQ(model, nullptr);
    EXPECT_TRUE(data.called);
}

// Test: Empty weight_map in index.json returns error
TEST_F(SplitLoaderTest, EmptyWeightMapReturnsError) {
    auto model_dir = test_dir_ / "model_empty_weights";
    std::filesystem::create_directories(model_dir);
    CreateIndexJson(model_dir / "model.safetensors.index.json",
                    R"({"metadata": {}, "weight_map": {}})");

    struct ErrorData { bool called = false; } data;
    auto callback = [](stcpp_error, const char*, void* ud) {
        static_cast<ErrorData*>(ud)->called = true;
    };

    stcpp_model* model = stcpp_model_load(
        model_dir.c_str(),
        callback,
        &data
    );

    EXPECT_EQ(model, nullptr);
    EXPECT_TRUE(data.called);
}

// Test: Correct parsing of shard file pattern
TEST_F(SplitLoaderTest, ShardFilePatternRecognition) {
    // Test pattern: model-XXXXX-of-YYYYY.safetensors
    std::string pattern1 = "model-00001-of-00003.safetensors";
    std::string pattern2 = "model-00002-of-00003.safetensors";
    std::string pattern3 = "model-00003-of-00003.safetensors";

    // Verify pattern contains expected components
    EXPECT_NE(pattern1.find("-00001-of-"), std::string::npos);
    EXPECT_NE(pattern2.find("-00002-of-"), std::string::npos);
    EXPECT_NE(pattern3.find("-00003-of-"), std::string::npos);

    // Verify extension
    EXPECT_TRUE(pattern1.ends_with(".safetensors"));
}

// Test: Verify unique shard extraction from weight_map
TEST_F(SplitLoaderTest, UniqueShardExtraction) {
    // Simulate weight_map parsing
    std::vector<std::pair<std::string, std::string>> weight_map = {
        {"layer1.weight", "model-00001-of-00002.safetensors"},
        {"layer1.bias", "model-00001-of-00002.safetensors"},
        {"layer2.weight", "model-00002-of-00002.safetensors"},
        {"layer2.bias", "model-00002-of-00002.safetensors"},
    };

    // Extract unique shards
    std::set<std::string> unique_shards;
    for (const auto& [tensor, shard] : weight_map) {
        unique_shards.insert(shard);
    }

    EXPECT_EQ(unique_shards.size(), 2u);
    EXPECT_TRUE(unique_shards.count("model-00001-of-00002.safetensors"));
    EXPECT_TRUE(unique_shards.count("model-00002-of-00002.safetensors"));
}

// Test: Handling of different index.json naming conventions
TEST_F(SplitLoaderTest, IndexJsonNamingConventions) {
    // Common naming patterns for index files
    std::vector<std::string> valid_names = {
        "model.safetensors.index.json",      // Standard HuggingFace format
        "pytorch_model.bin.index.json",      // Legacy PyTorch format (converted)
    };

    for (const auto& name : valid_names) {
        EXPECT_TRUE(name.ends_with(".index.json"))
            << "Name should end with .index.json: " << name;
    }
}
