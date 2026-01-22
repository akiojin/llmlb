#include <gtest/gtest.h>

#include <memory>

#include "core/engine_registry.h"
#include "models/model_descriptor.h"
#include <nlohmann/json.hpp>

namespace {

class FakeEngine : public xllm::Engine {
public:
    explicit FakeEngine(std::string label) : label_(std::move(label)) {}

    std::string runtime() const override { return "fake"; }
    bool supportsTextGeneration() const override { return true; }
    bool supportsEmbeddings() const override { return false; }

    const std::string& label() const { return label_; }

    xllm::ModelLoadResult loadModel(const xllm::ModelDescriptor&) override {
        xllm::ModelLoadResult result;
        result.success = true;
        result.error_code = xllm::EngineErrorCode::kOk;
        return result;
    }

    std::string generateChat(const std::vector<xllm::ChatMessage>&,
                             const xllm::ModelDescriptor&,
                             const xllm::InferenceParams&) const override {
        return "ok";
    }

    std::string generateCompletion(const std::string&,
                                   const xllm::ModelDescriptor&,
                                   const xllm::InferenceParams&) const override {
        return "ok";
    }

    std::vector<std::string> generateChatStream(
        const std::vector<xllm::ChatMessage>&,
        const xllm::ModelDescriptor&,
        const xllm::InferenceParams&,
        const std::function<void(const std::string&)>&) const override {
        return {};
    }

    std::vector<std::vector<float>> generateEmbeddings(
        const std::vector<std::string>&,
        const xllm::ModelDescriptor&) const override {
        return {};
    }

    size_t getModelMaxContext(const xllm::ModelDescriptor&) const override {
        return 0;
    }

private:
    std::string label_;
};

}  // namespace

using xllm::EngineRegistry;
using xllm::EngineRegistration;
using xllm::ModelDescriptor;

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

TEST(EngineRegistryTest, ResolvesByFormat) {
    EngineRegistry registry;

    auto engine_a = std::make_unique<FakeEngine>("safetensors");
    auto* engine_a_ptr = engine_a.get();
    EngineRegistration reg_a;
    reg_a.engine_id = "engine_safetensors";
    reg_a.engine_version = "0.1.0";
    reg_a.formats = {"safetensors"};
    ASSERT_TRUE(registry.registerEngine(std::move(engine_a), reg_a, nullptr));

    auto engine_b = std::make_unique<FakeEngine>("gguf");
    auto* engine_b_ptr = engine_b.get();
    EngineRegistration reg_b;
    reg_b.engine_id = "engine_gguf";
    reg_b.engine_version = "0.1.0";
    reg_b.formats = {"gguf"};
    ASSERT_TRUE(registry.registerEngine(std::move(engine_b), reg_b, nullptr));

    ModelDescriptor desc;
    desc.runtime = "fake";
    desc.format = "gguf";

    EXPECT_EQ(registry.resolve(desc), engine_b_ptr);
    EXPECT_NE(registry.resolve(desc), engine_a_ptr);
}

TEST(EngineRegistryTest, ReturnsNullWhenFormatMismatch) {
    EngineRegistry registry;

    auto engine_a = std::make_unique<FakeEngine>("safetensors");
    EngineRegistration reg_a;
    reg_a.engine_id = "engine_safetensors";
    reg_a.engine_version = "0.1.0";
    reg_a.formats = {"safetensors"};
    ASSERT_TRUE(registry.registerEngine(std::move(engine_a), reg_a, nullptr));

    ModelDescriptor desc;
    desc.runtime = "fake";
    desc.format = "gguf";

    EXPECT_EQ(registry.resolve(desc), nullptr);
}

TEST(EngineRegistryTest, ResolvesByCapability) {
    EngineRegistry registry;

    auto engine_text = std::make_unique<FakeEngine>("text");
    auto* engine_text_ptr = engine_text.get();
    EngineRegistration reg_text;
    reg_text.engine_id = "engine_text";
    reg_text.engine_version = "0.1.0";
    reg_text.capabilities = {"text"};
    ASSERT_TRUE(registry.registerEngine(std::move(engine_text), reg_text, nullptr));

    auto engine_embed = std::make_unique<FakeEngine>("embeddings");
    auto* engine_embed_ptr = engine_embed.get();
    EngineRegistration reg_embed;
    reg_embed.engine_id = "engine_embeddings";
    reg_embed.engine_version = "0.1.0";
    reg_embed.capabilities = {"embeddings"};
    ASSERT_TRUE(registry.registerEngine(std::move(engine_embed), reg_embed, nullptr));

    ModelDescriptor desc;
    desc.runtime = "fake";

    EXPECT_EQ(registry.resolve(desc, "embeddings"), engine_embed_ptr);
    EXPECT_NE(registry.resolve(desc, "embeddings"), engine_text_ptr);
}

TEST(EngineRegistryTest, RejectsUnsupportedArchitecture) {
    EngineRegistry registry;

    auto engine = std::make_unique<FakeEngine>("arch");
    auto* engine_ptr = engine.get();
    EngineRegistration reg;
    reg.engine_id = "engine_arch";
    reg.engine_version = "0.1.0";
    reg.architectures = {"custom_arch", "mamba"};
    ASSERT_TRUE(registry.registerEngine(std::move(engine), reg, nullptr));

    ModelDescriptor desc;
    desc.runtime = "fake";
    desc.architectures = {"llama"};

    std::string error;
    EXPECT_EQ(registry.resolve(desc, "", &error), nullptr);
    EXPECT_NE(error.find("architecture"), std::string::npos);
    EXPECT_NE(error.find("custom_arch"), std::string::npos);

    desc.architectures = {"mamba"};
    error.clear();
    EXPECT_EQ(registry.resolve(desc, "", &error), engine_ptr);
    EXPECT_TRUE(error.empty());
}

TEST(EngineRegistryTest, ReturnsNullWhenCapabilityMismatch) {
    EngineRegistry registry;

    auto engine_text = std::make_unique<FakeEngine>("text");
    EngineRegistration reg_text;
    reg_text.engine_id = "engine_text";
    reg_text.engine_version = "0.1.0";
    reg_text.capabilities = {"text"};
    ASSERT_TRUE(registry.registerEngine(std::move(engine_text), reg_text, nullptr));

    ModelDescriptor desc;
    desc.runtime = "fake";

    EXPECT_EQ(registry.resolve(desc, "embeddings"), nullptr);
}
