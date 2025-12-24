#pragma once

#include <string>
#include <vector>
#include <memory>
#include <chrono>
#include "config.h"
#include "model_config.h"
#include "safetensors_loader.h"
#include "cuda_memory.h"
#include "tokenizer.h"
#include "transformer.h"

namespace nemotron {

// Generation configuration
struct GenerationConfig {
    size_t max_tokens = 100;
    float temperature = 1.0f;
    int top_k = 50;
    bool greedy = true;  // Use greedy decoding for PoC
};

// Generation statistics
struct GenerationStats {
    size_t prompt_tokens = 0;
    size_t generated_tokens = 0;
    double load_time_ms = 0.0;
    double prompt_time_ms = 0.0;
    double generation_time_ms = 0.0;

    double tokensPerSecond() const {
        if (generation_time_ms <= 0) return 0;
        return (generated_tokens * 1000.0) / generation_time_ms;
    }
};

// Main inference engine
class InferenceEngine {
public:
    InferenceEngine() = default;
    ~InferenceEngine() = default;

    InferenceEngine(const InferenceEngine&) = delete;
    InferenceEngine& operator=(const InferenceEngine&) = delete;

    // Initialize engine with model path
    void loadModel(const std::string& model_dir, int device_id = 0);

    // Generate text from prompt
    std::string generate(
        const std::string& prompt,
        const GenerationConfig& config = GenerationConfig()
    );

    // Get generation statistics
    const GenerationStats& getStats() const { return stats_; }

    // Check if model is loaded
    bool isLoaded() const { return model_ != nullptr; }

    // Get model config
    const ModelConfig& getConfig() const { return config_; }

private:
    ModelConfig config_;
    SafetensorsLoader loader_;
    CudaModelManager cuda_manager_;
    std::unique_ptr<Tokenizer> tokenizer_;
    std::unique_ptr<TransformerModel> model_;
    GenerationStats stats_;

    // Generation buffers
    CudaBuffer<float> logits_;

    // Sample next token
    int32_t sampleToken(const float* logits, const GenerationConfig& config);
};

}  // namespace nemotron
