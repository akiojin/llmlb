/**
 * @file qwen_models_test.cpp
 * @brief Qwen系モデル定義の契約テスト
 *
 * SPEC-6cd7f960: 検証済みモデル一覧
 * Qwenモデルがsupported_models_json.hに正しく定義されていることを検証する。
 */

#include <gtest/gtest.h>
#include <nlohmann/json.hpp>
#include "models/supported_models_json.h"

namespace {

using json = nlohmann::json;

class QwenModelsTest : public ::testing::Test {
protected:
    json models_;

    void SetUp() override {
        models_ = json::parse(xllm::kSupportedModelsJson);
    }

    // ヘルパー: IDでモデルを検索
    json findModelById(const std::string& id) {
        for (const auto& model : models_) {
            if (model["id"] == id) {
                return model;
            }
        }
        return json();
    }
};

// T1: Qwen2.5 7B Instruct の定義検証
TEST_F(QwenModelsTest, Qwen25_7B_Instruct_DefinitionIsCorrect) {
    auto model = findModelById("qwen2.5-7b-instruct");
    ASSERT_FALSE(model.empty()) << "qwen2.5-7b-instruct should be defined";

    EXPECT_EQ(model["name"], "Qwen2.5 7B Instruct");
    EXPECT_EQ(model["repo"], "bartowski/Qwen2.5-7B-Instruct-GGUF");
    EXPECT_EQ(model["recommended_filename"], "Qwen2.5-7B-Instruct-Q4_K_M.gguf");
    EXPECT_EQ(model["format"], "gguf");
    EXPECT_EQ(model["engine"], "llama_cpp");
    EXPECT_EQ(model["parameter_count"], "7B");
    EXPECT_EQ(model["quantization"], "Q4_K_M");

    // Capabilities
    ASSERT_TRUE(model.contains("capabilities"));
    auto caps = model["capabilities"];
    EXPECT_TRUE(std::find(caps.begin(), caps.end(), "TextGeneration") != caps.end());

    // Platforms
    ASSERT_TRUE(model.contains("platforms"));
    auto platforms = model["platforms"];
    EXPECT_TRUE(std::find(platforms.begin(), platforms.end(), "macos-metal") != platforms.end());
    EXPECT_TRUE(std::find(platforms.begin(), platforms.end(), "windows-directml") != platforms.end());
    EXPECT_TRUE(std::find(platforms.begin(), platforms.end(), "linux-cuda") != platforms.end());
}

// T2: Qwen3 0.6B の定義検証
TEST_F(QwenModelsTest, Qwen3_06B_DefinitionIsCorrect) {
    auto model = findModelById("qwen3");
    ASSERT_FALSE(model.empty()) << "qwen3 should be defined";

    EXPECT_EQ(model["name"], "Qwen3 0.6B");
    EXPECT_EQ(model["repo"], "bartowski/Qwen_Qwen3-0.6B-GGUF");
    EXPECT_EQ(model["recommended_filename"], "Qwen_Qwen3-0.6B-Q4_K_M.gguf");
    EXPECT_EQ(model["format"], "gguf");
    EXPECT_EQ(model["engine"], "llama_cpp");
    EXPECT_EQ(model["parameter_count"], "0.6B");

    // Tags should include compact
    ASSERT_TRUE(model.contains("tags"));
    auto tags = model["tags"];
    EXPECT_TRUE(std::find(tags.begin(), tags.end(), "compact") != tags.end());
}

// T3: QwQ 32B の定義検証
TEST_F(QwenModelsTest, QwQ_32B_DefinitionIsCorrect) {
    auto model = findModelById("qwq");
    ASSERT_FALSE(model.empty()) << "qwq should be defined";

    EXPECT_EQ(model["name"], "QwQ 32B");
    EXPECT_EQ(model["repo"], "Qwen/QwQ-32B-GGUF");
    EXPECT_EQ(model["recommended_filename"], "qwq-32b-q4_k_m.gguf");
    EXPECT_EQ(model["format"], "gguf");
    EXPECT_EQ(model["engine"], "llama_cpp");
    EXPECT_EQ(model["parameter_count"], "32B");
}

// T4: Qwen3 Coder 30B の定義検証
TEST_F(QwenModelsTest, Qwen3Coder_30B_DefinitionIsCorrect) {
    auto model = findModelById("qwen3-coder");
    ASSERT_FALSE(model.empty()) << "qwen3-coder should be defined";

    EXPECT_EQ(model["name"], "Qwen3 Coder 30B A3B Instruct");
    EXPECT_EQ(model["repo"], "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF");
    EXPECT_EQ(model["recommended_filename"], "Qwen3-Coder-30B-A3B-Instruct-Q4_K_M.gguf");
    EXPECT_EQ(model["format"], "gguf");
    EXPECT_EQ(model["engine"], "llama_cpp");

    // Tags should include coding
    ASSERT_TRUE(model.contains("tags"));
    auto tags = model["tags"];
    EXPECT_TRUE(std::find(tags.begin(), tags.end(), "coding") != tags.end());
}

// T5: すべてのQwenモデルがllama_cppエンジンを使用することを検証
TEST_F(QwenModelsTest, AllQwenModelsUseLlamaCppEngine) {
    std::vector<std::string> qwen_ids = {
        "qwen2.5-7b-instruct",
        "qwen3",
        "qwq",
        "qwen3-coder"
    };

    for (const auto& id : qwen_ids) {
        auto model = findModelById(id);
        ASSERT_FALSE(model.empty()) << id << " should be defined";
        EXPECT_EQ(model["engine"], "llama_cpp")
            << id << " should use llama_cpp engine";
        EXPECT_EQ(model["format"], "gguf")
            << id << " should be in GGUF format";
    }
}

// T6: Qwenモデルのメモリ要件が妥当であることを検証
TEST_F(QwenModelsTest, QwenModelsHaveReasonableMemoryRequirements) {
    auto qwen3 = findModelById("qwen3");
    ASSERT_FALSE(qwen3.empty());
    // Qwen3 0.6B: 約500MB file, 約750MB memory
    EXPECT_LT(qwen3["size_bytes"].get<int64_t>(), 1000000000);  // < 1GB
    EXPECT_LT(qwen3["required_memory_bytes"].get<int64_t>(), 2000000000);  // < 2GB

    auto qwq = findModelById("qwq");
    ASSERT_FALSE(qwq.empty());
    // QwQ 32B: 約20GB file, 約30GB memory
    EXPECT_GT(qwq["size_bytes"].get<int64_t>(), 10000000000);  // > 10GB
    EXPECT_GT(qwq["required_memory_bytes"].get<int64_t>(), 20000000000);  // > 20GB
}

}  // namespace
