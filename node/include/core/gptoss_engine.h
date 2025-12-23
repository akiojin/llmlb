#pragma once

#include <memory>
#include <mutex>
#include <string>
#include <unordered_map>
#include <vector>

#include "core/engine.h"

namespace llm_node {

class GptOssEngine : public Engine {
public:
    GptOssEngine() = default;
    ~GptOssEngine() override = default;

    std::string runtime() const override { return "gptoss_cpp"; }
    bool supportsTextGeneration() const override {
#ifdef USE_GPTOSS
        return true;
#else
        return false;
#endif
    }
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
    struct LoadedModel;

    std::shared_ptr<LoadedModel> ensureLoaded(const ModelDescriptor& descriptor,
                                              ModelLoadResult& result) const;

    mutable std::mutex mutex_;
    mutable std::unordered_map<std::string, std::shared_ptr<LoadedModel>> loaded_;
};

}  // namespace llm_node
