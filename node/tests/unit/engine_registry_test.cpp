#include <gtest/gtest.h>

#include <memory>

#include "core/engine_registry.h"
#include "models/model_descriptor.h"
#include <nlohmann/json.hpp>

namespace {

class FakeEngine : public llm_node::Engine {
public:
    explicit FakeEngine(std::string label) : label_(std::move(label)) {}

    std::string runtime() const override { return "fake"; }
    bool supportsTextGeneration() const override { return true; }
    bool supportsEmbeddings() const override { return false; }

    const std::string& label() const { return label_; }

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

private:
    std::string label_;
};

}  // namespace

using llm_node::EngineRegistry;
using llm_node::EngineRegistration;
using llm_node::ModelDescriptor;

TEST(EngineRegistryTest, ResolvesByRuntime) {
    EngineRegistry registry;
    auto engine = std::make_unique<FakeEngine>("primary");
    auto* engine_ptr = engine.get();
    EngineRegistration reg;
    reg.engine_id = "engine_primary";
    reg.engine_version = "0.1.0";
    ASSERT_TRUE(registry.registerEngine(std::move(engine), reg, nullptr));

    ModelDescriptor desc;
    desc.runtime = "fake";

    EXPECT_EQ(registry.resolve(desc), engine_ptr);
    EXPECT_EQ(registry.resolve("missing"), nullptr);
}

TEST(EngineRegistryTest, ResolvesByBenchmarkScore) {
    EngineRegistry registry;

    auto engine_a = std::make_unique<FakeEngine>("a");
    auto* engine_a_ptr = engine_a.get();
    EngineRegistration reg_a;
    reg_a.engine_id = "engine_a";
    reg_a.engine_version = "0.1.0";
    ASSERT_TRUE(registry.registerEngine(std::move(engine_a), reg_a, nullptr));

    auto engine_b = std::make_unique<FakeEngine>("b");
    auto* engine_b_ptr = engine_b.get();
    EngineRegistration reg_b;
    reg_b.engine_id = "engine_b";
    reg_b.engine_version = "0.1.0";
    ASSERT_TRUE(registry.registerEngine(std::move(engine_b), reg_b, nullptr));

    ModelDescriptor desc;
    desc.runtime = "fake";
    nlohmann::json meta;
    meta["benchmarks"]["engine_scores"] = {{"engine_a", 1.0}, {"engine_b", 5.0}};
    desc.metadata = meta;

    EXPECT_EQ(registry.resolve(desc), engine_b_ptr);
}

TEST(EngineRegistryTest, FallsBackToFirstEngineWhenNoBenchmarks) {
    EngineRegistry registry;

    auto engine_a = std::make_unique<FakeEngine>("a");
    auto* engine_a_ptr = engine_a.get();
    EngineRegistration reg_a;
    reg_a.engine_id = "engine_a";
    reg_a.engine_version = "0.1.0";
    ASSERT_TRUE(registry.registerEngine(std::move(engine_a), reg_a, nullptr));

    auto engine_b = std::make_unique<FakeEngine>("b");
    EngineRegistration reg_b;
    reg_b.engine_id = "engine_b";
    reg_b.engine_version = "0.1.0";
    ASSERT_TRUE(registry.registerEngine(std::move(engine_b), reg_b, nullptr));

    ModelDescriptor desc;
    desc.runtime = "fake";

    EXPECT_EQ(registry.resolve(desc), engine_a_ptr);
}
