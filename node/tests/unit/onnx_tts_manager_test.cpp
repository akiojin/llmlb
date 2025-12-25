#include "core/onnx_tts_manager.h"

#include <gtest/gtest.h>

namespace {

TEST(OnnxTtsManagerTest, RuntimeAvailabilityReflectsCompileConfig) {
#ifdef USE_ONNX_RUNTIME
    EXPECT_TRUE(llm_node::OnnxTtsManager::isRuntimeAvailable());
#else
    EXPECT_FALSE(llm_node::OnnxTtsManager::isRuntimeAvailable());
#endif
}

TEST(OnnxTtsManagerTest, DefaultIdleTimeoutIs30Minutes) {
    llm_node::OnnxTtsManager manager("/tmp/models");
    auto timeout = manager.getIdleTimeout();
    EXPECT_EQ(timeout.count(), std::chrono::minutes(30).count());
}

TEST(OnnxTtsManagerTest, MaxLoadedModelsDefaultsToUnlimited) {
    llm_node::OnnxTtsManager manager("/tmp/models");
    EXPECT_EQ(manager.getMaxLoadedModels(), 0u);
}

TEST(OnnxTtsManagerTest, LoadedCountIsZeroOnInit) {
    llm_node::OnnxTtsManager manager("/tmp/models");
    EXPECT_EQ(manager.loadedCount(), 0u);
}

TEST(OnnxTtsManagerTest, GetLoadedModelsReturnsEmptyOnInit) {
    llm_node::OnnxTtsManager manager("/tmp/models");
    EXPECT_TRUE(manager.getLoadedModels().empty());
}

TEST(OnnxTtsManagerTest, IsLoadedReturnsFalseForUnloadedModel) {
    llm_node::OnnxTtsManager manager("/tmp/models");
    EXPECT_FALSE(manager.isLoaded("nonexistent_model.onnx"));
}

TEST(OnnxTtsManagerTest, SetIdleTimeoutUpdatesValue) {
    llm_node::OnnxTtsManager manager("/tmp/models");
    manager.setIdleTimeout(std::chrono::minutes(60));
    EXPECT_EQ(manager.getIdleTimeout().count(), std::chrono::minutes(60).count());
}

TEST(OnnxTtsManagerTest, SetMaxLoadedModelsUpdatesValue) {
    llm_node::OnnxTtsManager manager("/tmp/models");
    manager.setMaxLoadedModels(5);
    EXPECT_EQ(manager.getMaxLoadedModels(), 5u);
}

}  // namespace
