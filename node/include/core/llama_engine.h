#pragma once

#include <memory>
#include <string>
#include <vector>

#include "core/engine.h"

namespace llm_node {

class LlamaManager;
class LlamaEngine : public Engine {
public:
    explicit LlamaEngine(LlamaManager& manager);

    std::string runtime() const override { return "llama_cpp"; }
    bool supportsTextGeneration() const override { return true; }
    bool supportsEmbeddings() const override { return true; }

    ModelLoadResult loadModel(const ModelDescriptor& descriptor) override;

    std::string generateChat(const std::vector<ChatMessage>& messages,
                             const ModelDescriptor& descriptor,
                             const InferenceParams& params) const override;

    std::string generateCompletion(const std::string& prompt,
                                   const ModelDescriptor& descriptor,
                                   const InferenceParams& params) const override;

    std::vector<std::string> generateChatStream(
        const std::vector<ChatMessage>& messages,
        const ModelDescriptor& descriptor,
        const InferenceParams& params,
        const std::function<void(const std::string&)>& on_token) const override;

    std::vector<std::vector<float>> generateEmbeddings(
        const std::vector<std::string>& inputs,
        const ModelDescriptor& descriptor) const override;

    size_t getModelMaxContext(const ModelDescriptor& descriptor) const override;
    uint64_t getModelVramBytes(const ModelDescriptor& descriptor) const override;

private:
    LlamaManager& manager_;
    mutable size_t model_max_ctx_{4096};

    std::string buildChatPrompt(const std::vector<ChatMessage>& messages) const;
};

}  // namespace llm_node
