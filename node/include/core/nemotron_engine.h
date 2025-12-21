#pragma once

#include <mutex>
#include <string>
#include <unordered_set>
#include <vector>

#include "core/engine.h"

namespace llm_node {

class NemotronEngine : public Engine {
public:
    NemotronEngine() = default;

    std::string runtime() const override { return "nemotron_cpp"; }
    bool supportsTextGeneration() const override { return false; }
    bool supportsEmbeddings() const override { return false; }

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

private:
    mutable std::mutex mutex_;
    std::unordered_set<std::string> loaded_;
};

}  // namespace llm_node
