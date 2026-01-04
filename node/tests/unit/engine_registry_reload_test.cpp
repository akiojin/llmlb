#include <gtest/gtest.h>

#include "core/engine_registry.h"
#include "models/model_descriptor.h"

namespace llm_node {

class DummyEngine final : public Engine {
public:
    DummyEngine(std::string tag, std::string runtime)
        : tag_(std::move(tag)), runtime_(std::move(runtime)) {}

    const std::string& tag() const { return tag_; }

    std::string runtime() const override { return runtime_; }
    bool supportsTextGeneration() const override { return true; }
    bool supportsEmbeddings() const override { return false; }

    ModelLoadResult loadModel(const ModelDescriptor&) override {
        return {true, EngineErrorCode::kOk, ""};
    }

    std::string generateChat(const std::vector<ChatMessage>&,
                             const ModelDescriptor&,
                             const InferenceParams&) const override {
        return "";
    }

    std::string generateCompletion(const std::string&,
                                   const ModelDescriptor&,
                                   const InferenceParams&) const override {
        return "";
    }

    std::vector<std::string> generateChatStream(const std::vector<ChatMessage>&,
                                                const ModelDescriptor&,
                                                const InferenceParams&,
                                                const std::function<void(const std::string&)>&) const override {
        return {};
    }

    std::vector<std::vector<float>> generateEmbeddings(const std::vector<std::string>&,
                                                       const ModelDescriptor&) const override {
        return {};
    }

    size_t getModelMaxContext(const ModelDescriptor&) const override { return 0; }

private:
    std::string tag_;
    std::string runtime_;
};

TEST(EngineRegistryReloadTest, ReplaceEngineById) {
    EngineRegistry registry;

    EngineRegistration reg_v1;
    reg_v1.engine_id = "plugin_engine";
    reg_v1.engine_version = "v1";
    reg_v1.formats = {"gguf"};
    reg_v1.architectures = {"llama"};
    reg_v1.capabilities = {"text"};

    EngineRegistry::EngineHandle engine_v1(new DummyEngine("v1", "dummy_runtime"), EngineDeleter{});
    std::string error;
    EXPECT_TRUE(registry.registerEngine(std::move(engine_v1), reg_v1, &error));

    auto* resolved_v1 = dynamic_cast<DummyEngine*>(registry.resolve("dummy_runtime"));
    ASSERT_NE(resolved_v1, nullptr);
    EXPECT_EQ(resolved_v1->tag(), "v1");

    EngineRegistration reg_v2 = reg_v1;
    reg_v2.engine_version = "v2";
    EngineRegistry::EngineHandle engine_v2(new DummyEngine("v2", "dummy_runtime"), EngineDeleter{});
    EngineRegistry::EngineHandle replaced;
    EXPECT_TRUE(registry.replaceEngine(std::move(engine_v2), reg_v2, &replaced, &error));
    EXPECT_TRUE(static_cast<bool>(replaced));

    auto* resolved_v2 = dynamic_cast<DummyEngine*>(registry.resolve("dummy_runtime"));
    ASSERT_NE(resolved_v2, nullptr);
    EXPECT_EQ(resolved_v2->tag(), "v2");
}

TEST(EngineRegistryReloadTest, ReplaceRegistersWhenMissing) {
    EngineRegistry registry;

    EngineRegistration reg;
    reg.engine_id = "new_engine";
    reg.engine_version = "v1";
    reg.formats = {"gguf"};
    reg.architectures = {"llama"};
    reg.capabilities = {"text"};

    EngineRegistry::EngineHandle engine(new DummyEngine("v1", "dummy_runtime"), EngineDeleter{});
    EngineRegistry::EngineHandle replaced;
    std::string error;
    EXPECT_TRUE(registry.replaceEngine(std::move(engine), reg, &replaced, &error));
    EXPECT_FALSE(static_cast<bool>(replaced));

    auto* resolved = dynamic_cast<DummyEngine*>(registry.resolve("dummy_runtime"));
    ASSERT_NE(resolved, nullptr);
    EXPECT_EQ(resolved->tag(), "v1");
}

}  // namespace llm_node
