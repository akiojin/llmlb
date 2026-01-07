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
    registry.setGpuBackend(GpuBackend::Metal);

    // listExecutableModels uses the stored backend
    auto models = registry.listExecutableModels();
    ASSERT_EQ(models.size(), 3);
    EXPECT_EQ(models[0], "qwen-7b");
    EXPECT_EQ(models[1], "llama-3.1-8b");
    EXPECT_EQ(models[2], "mistral-7b");

    // CUDA バックエンドでも同様
    registry.setGpuBackend(GpuBackend::Cuda);
    models = registry.listExecutableModels();
    ASSERT_EQ(models.size(), 3);
}

TEST(ModelRegistryTest, ListExecutableModelsReturnsEmptyWhenNoModels) {
    ModelRegistry registry;
    registry.setGpuBackend(GpuBackend::Cpu);

    auto models = registry.listExecutableModels();
    EXPECT_TRUE(models.empty());
}

// T2.4: hasModel テスト (isCompatible is private, test via hasModel)

TEST(ModelRegistryTest, HasModelReturnsTrueForLoadedModelsOnAllBackends) {
    ModelRegistry registry;
    registry.setModels({"qwen-7b", "llama-3.1-8b"});

    // 登録済みモデルはhasModelでtrue
    registry.setGpuBackend(GpuBackend::Metal);
    EXPECT_TRUE(registry.hasModel("qwen-7b"));
    registry.setGpuBackend(GpuBackend::Cuda);
    EXPECT_TRUE(registry.hasModel("llama-3.1-8b"));
    registry.setGpuBackend(GpuBackend::Rocm);
    EXPECT_TRUE(registry.hasModel("qwen-7b"));
    registry.setGpuBackend(GpuBackend::Cpu);
    EXPECT_TRUE(registry.hasModel("qwen-7b"));
}

TEST(ModelRegistryTest, HasModelReturnsFalseForUnknownModels) {
    ModelRegistry registry;
    registry.setModels({"qwen-7b"});

    // 未登録モデルはhasModelでfalse
    EXPECT_FALSE(registry.hasModel("unknown-model"));
    EXPECT_FALSE(registry.hasModel("not-loaded"));
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
