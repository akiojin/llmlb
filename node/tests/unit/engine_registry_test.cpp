#include <gtest/gtest.h>

#include <memory>

#include "core/engine_registry.h"
#include "models/model_descriptor.h"

namespace {

class FakeEngine : public llm_node::Engine {
public:
    std::string runtime() const override { return "fake"; }
    bool supportsTextGeneration() const override { return true; }
    bool supportsEmbeddings() const override { return false; }

    llm_node::ModelLoadResult loadModel(const llm_node::ModelDescriptor&) override {
        llm_node::ModelLoadResult result;
        result.success = true;
        return result;
    }

    std::string generateChat(const std::vector<llm_node::ChatMessage>&,
                             const llm_node::ModelDescriptor&,
                             const llm_node::InferenceParams&) const override {
        return "ok";
    }

    std::string generateCompletion(const std::string&,
                                   const llm_node::ModelDescriptor&,
                                   const llm_node::InferenceParams&) const override {
        return "ok";
    }

    std::vector<std::string> generateChatStream(
        const std::vector<llm_node::ChatMessage>&,
        const llm_node::ModelDescriptor&,
        const llm_node::InferenceParams&,
        const std::function<void(const std::string&)>&) const override {
        return {};
    }

    std::vector<std::vector<float>> generateEmbeddings(
        const std::vector<std::string>&,
        const llm_node::ModelDescriptor&) const override {
        return {};
    }

    size_t getModelMaxContext(const llm_node::ModelDescriptor&) const override {
        return 0;
    }
};

}  // namespace

using llm_node::EngineRegistry;

TEST(EngineRegistryTest, ResolvesByRuntime) {
    EngineRegistry registry;
    registry.registerEngine(std::make_unique<FakeEngine>());

    EXPECT_NE(registry.resolve("fake"), nullptr);
    EXPECT_EQ(registry.resolve("missing"), nullptr);
}
