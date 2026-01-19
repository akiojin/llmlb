#pragma once

#include <cstdint>
#include <functional>
#include <memory>
#include <string>
#include <vector>

#include "core/engine_types.h"
#include "models/model_descriptor.h"

namespace allm {

class Engine {
public:
    virtual ~Engine() = default;

    virtual std::string runtime() const = 0;
    virtual bool supportsTextGeneration() const = 0;
    virtual bool supportsEmbeddings() const = 0;

    virtual ModelLoadResult loadModel(const ModelDescriptor& descriptor) = 0;

    virtual std::string generateChat(const std::vector<ChatMessage>& messages,
                                     const ModelDescriptor& descriptor,
                                     const InferenceParams& params) const = 0;

    virtual std::string generateCompletion(const std::string& prompt,
                                           const ModelDescriptor& descriptor,
                                           const InferenceParams& params) const = 0;

    virtual std::vector<std::string> generateChatStream(
        const std::vector<ChatMessage>& messages,
        const ModelDescriptor& descriptor,
        const InferenceParams& params,
        const std::function<void(const std::string&)>& on_token) const = 0;

    virtual std::vector<std::vector<float>> generateEmbeddings(
        const std::vector<std::string>& inputs,
        const ModelDescriptor& descriptor) const = 0;

    virtual size_t getModelMaxContext(const ModelDescriptor& descriptor) const = 0;

    virtual uint64_t getModelVramBytes(const ModelDescriptor&) const { return 0; }
};

}  // namespace allm
