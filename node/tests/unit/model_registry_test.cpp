/**
 * @file model_registry_test.cpp
 * @brief SPEC-93536000: ModelRegistry テスト
 */

#include <gtest/gtest.h>
#include "models/model_registry.h"
#include "system/gpu_detector.h"

namespace {

using llm_node::ModelRegistry;
using llm_node::GpuBackend;

// T2.3: listExecutableModels テスト

TEST(ModelRegistryTest, ListExecutableModelsReturnsAllModelsForCompatibleBackend) {
    ModelRegistry registry;
    registry.setModels({"qwen-7b", "llama-3.1-8b", "mistral-7b"});

    // Metal バックエンドでも全モデルを返す（ロード可能なモデルは互換性あり）
    auto models = registry.listExecutableModels(GpuBackend::kMetal);
    ASSERT_EQ(models.size(), 3);
    EXPECT_EQ(models[0], "qwen-7b");
    EXPECT_EQ(models[1], "llama-3.1-8b");
    EXPECT_EQ(models[2], "mistral-7b");

    // CUDA バックエンドでも同様
    models = registry.listExecutableModels(GpuBackend::kCuda);
    ASSERT_EQ(models.size(), 3);
}

TEST(ModelRegistryTest, ListExecutableModelsReturnsEmptyWhenNoModels) {
    ModelRegistry registry;

    auto models = registry.listExecutableModels(GpuBackend::kCpu);
    EXPECT_TRUE(models.empty());
}

// T2.4: isCompatible テスト

TEST(ModelRegistryTest, IsCompatibleReturnsTrueForLoadedModels) {
    ModelRegistry registry;
    registry.setModels({"qwen-7b", "llama-3.1-8b"});

    // 登録済みモデルは現在のバックエンドと互換性あり
    EXPECT_TRUE(registry.isCompatible("qwen-7b", GpuBackend::kMetal));
    EXPECT_TRUE(registry.isCompatible("llama-3.1-8b", GpuBackend::kCuda));
    EXPECT_TRUE(registry.isCompatible("qwen-7b", GpuBackend::kRocm));
    EXPECT_TRUE(registry.isCompatible("qwen-7b", GpuBackend::kCpu));
}

TEST(ModelRegistryTest, IsCompatibleReturnsFalseForUnknownModels) {
    ModelRegistry registry;
    registry.setModels({"qwen-7b"});

    // 未登録モデルは非互換
    EXPECT_FALSE(registry.isCompatible("unknown-model", GpuBackend::kMetal));
    EXPECT_FALSE(registry.isCompatible("not-loaded", GpuBackend::kCuda));
}

// 既存機能テスト

TEST(ModelRegistryTest, ListModelsReturnsAllRegisteredModels) {
    ModelRegistry registry;
    registry.setModels({"model-a", "model-b", "model-c"});

    auto models = registry.listModels();
    ASSERT_EQ(models.size(), 3);
    EXPECT_EQ(models[0], "model-a");
    EXPECT_EQ(models[1], "model-b");
    EXPECT_EQ(models[2], "model-c");
}

TEST(ModelRegistryTest, HasModelReturnsTrueForExistingModel) {
    ModelRegistry registry;
    registry.setModels({"qwen-7b", "llama-3.1-8b"});

    EXPECT_TRUE(registry.hasModel("qwen-7b"));
    EXPECT_TRUE(registry.hasModel("llama-3.1-8b"));
}

TEST(ModelRegistryTest, HasModelReturnsFalseForMissingModel) {
    ModelRegistry registry;
    registry.setModels({"qwen-7b"});

    EXPECT_FALSE(registry.hasModel("unknown-model"));
    EXPECT_FALSE(registry.hasModel(""));
}

}  // namespace
