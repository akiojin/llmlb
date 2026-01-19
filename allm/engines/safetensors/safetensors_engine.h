/**
 * @file safetensors_engine.h
 * @brief SafetensorsEngine - Engine implementation for safetensors.cpp
 *
 * SPEC-69549000: safetensors.cpp Node integration
 */

#pragma once

#include <memory>
#include <mutex>
#include <string>
#include <unordered_map>
#include <vector>

#include "core/engine.h"
#include "core/engine_types.h"
#include "models/model_descriptor.h"

// Forward declarations for safetensors.cpp C API
extern "C" {
struct stcpp_model;
struct stcpp_context;
struct stcpp_tokenizer;
}

namespace llm_node {

/**
 * @class SafetensorsEngine
 * @brief Engine implementation wrapping safetensors.cpp for safetensors format models
 *
 * Provides text generation and embeddings using the safetensors.cpp library
 * with Metal/CUDA GPU acceleration.
 */
class SafetensorsEngine : public Engine {
public:
    /**
     * @brief Construct a new SafetensorsEngine
     * @param models_dir Base directory for model files
     */
    explicit SafetensorsEngine(const std::string& models_dir);

    ~SafetensorsEngine() override;

    // Engine interface implementation
    std::string runtime() const override;
    bool supportsTextGeneration() const override;
    bool supportsEmbeddings() const override;

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
    struct LoadedModel {
        stcpp_model* model{nullptr};
        stcpp_context* ctx{nullptr};
        stcpp_tokenizer* tokenizer{nullptr};
        size_t max_context{0};
        uint64_t vram_bytes{0};
        bool has_trained_chat_tokens{true};  // False for base models (not instruct)
    };

    /**
     * @brief Get or load a model for the given descriptor
     * @param descriptor Model descriptor
     * @return Pointer to the loaded model, or nullptr if not found
     */
    LoadedModel* getOrLoadModel(const ModelDescriptor& descriptor) const;

    /**
     * @brief Build a chat prompt from messages using the model's chat template
     * @param messages Chat messages
     * @param loaded Loaded model containing tokenizer and flags
     * @return Formatted prompt string
     * @note For base models (has_trained_chat_tokens=false), uses simple format
     */
    std::string buildChatPrompt(const std::vector<ChatMessage>& messages,
                                const LoadedModel* loaded) const;

    /**
     * @brief Convert InferenceParams to stcpp_sampling_params
     * @param params Inference parameters
     * @return safetensors.cpp sampling parameters
     */
    static void convertSamplingParams(const InferenceParams& params,
                                      void* out_params);

    std::string models_dir_;
    mutable std::mutex models_mutex_;
    mutable std::unordered_map<std::string, std::unique_ptr<LoadedModel>> loaded_models_;
};

}  // namespace llm_node
