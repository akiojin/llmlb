#pragma once

#include <functional>
#include <memory>
#include <mutex>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <vector>

#include "core/engine.h"

namespace llm_node {

class NemotronEngine : public Engine {
public:
    NemotronEngine() = default;
    ~NemotronEngine() override;

    std::string runtime() const override { return "nemotron_cpp"; }
    bool supportsTextGeneration() const override {
#if defined(_WIN32) && defined(USE_GPTOSS) && (defined(USE_DIRECTML) || defined(USE_CUDA))
        return true;
#elif defined(USE_CUDA)
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
    uint64_t getModelVramBytes(const ModelDescriptor& descriptor) const override;

private:
    struct LoadedModel;
    std::shared_ptr<LoadedModel> ensureLoaded(const ModelDescriptor& descriptor,
                                              ModelLoadResult& result) const;
    std::string generateCompletionInternal(
        const std::string& prompt,
        const ModelDescriptor& descriptor,
        const InferenceParams& params,
        const std::vector<ChatMessage>* chat_messages,
        const std::function<void(const std::string&)>& on_token) const;
    mutable std::mutex mutex_;
    std::unordered_set<std::string> loaded_;
    std::unordered_map<std::string, std::shared_ptr<LoadedModel>> loaded_models_;
#ifdef USE_CUDA
    struct CudaBuffer {
        void* device_ptr{nullptr};
        size_t bytes{0};
    };
    std::unordered_map<std::string, CudaBuffer> cuda_buffers_;
#endif
};

}  // namespace llm_node
