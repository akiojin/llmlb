#include "inference.h"
#include "cuda_utils.h"
#include "kernels.h"
#include <filesystem>
#include <iostream>

namespace nemotron {

namespace fs = std::filesystem;

void InferenceEngine::loadModel(const std::string& model_dir, int device_id) {
    auto start_time = std::chrono::high_resolution_clock::now();

    LOG_INFO("Loading model from: " << model_dir);

    // Verify model directory exists
    if (!fs::exists(model_dir)) {
        throw FileError("Model directory does not exist: " + model_dir);
    }

    // Load config.json
    std::string config_path = model_dir + "/config.json";
    if (!fs::exists(config_path)) {
        throw FileError("config.json not found in: " + model_dir);
    }
    config_ = loadModelConfig(config_path);

    // Load tokenizer
    std::string tokenizer_path = model_dir + "/tokenizer.json";
    if (!fs::exists(tokenizer_path)) {
        throw FileError("tokenizer.json not found in: " + model_dir);
    }
    tokenizer_ = std::make_unique<Tokenizer>();
    tokenizer_->load(tokenizer_path);

    // Initialize CUDA
    cuda_manager_.initDevice(device_id);

    // Load safetensors weights
    loader_.loadModel(model_dir);

    // Transfer weights to GPU
    cuda_manager_.loadWeights(loader_, config_);

    // Create transformer model
    model_ = std::make_unique<TransformerModel>(config_, cuda_manager_);

    // Allocate logits buffer
    logits_ = CudaBuffer<float>(config_.vocab_size);

    auto end_time = std::chrono::high_resolution_clock::now();
    stats_.load_time_ms = std::chrono::duration<double, std::milli>(
        end_time - start_time).count();

    LOG_INFO("Model loaded in " << stats_.load_time_ms << " ms");
}

int32_t InferenceEngine::sampleToken(const float* logits, const GenerationConfig& config) {
    if (config.greedy) {
        return kernels::argmax(logits, config_.vocab_size);
    } else {
        return kernels::topKSample(
            logits, config_.vocab_size,
            config.top_k, config.temperature
        );
    }
}

std::string InferenceEngine::generate(
    const std::string& prompt,
    const GenerationConfig& config
) {
    if (!isLoaded()) {
        throw ModelError("Model not loaded");
    }

    LOG_INFO("Generating with prompt: \"" << prompt << "\"");

    // Reset stats
    stats_.prompt_tokens = 0;
    stats_.generated_tokens = 0;
    stats_.prompt_time_ms = 0;
    stats_.generation_time_ms = 0;

    // Tokenize prompt
    std::vector<int32_t> input_ids = tokenizer_->encode(prompt);
    stats_.prompt_tokens = input_ids.size();
    LOG_INFO("Prompt tokens: " << stats_.prompt_tokens);

    // Reset KV cache
    model_->resetCache();

    // Process prompt (prefill)
    auto prompt_start = std::chrono::high_resolution_clock::now();

    model_->forward(
        logits_.get(),
        input_ids.data(),
        input_ids.size(),
        0  // position_offset
    );
    CUDA_CHECK(cudaDeviceSynchronize());

    auto prompt_end = std::chrono::high_resolution_clock::now();
    stats_.prompt_time_ms = std::chrono::duration<double, std::milli>(
        prompt_end - prompt_start).count();

    // Sample first token
    std::vector<float> logits_host(config_.vocab_size);
    logits_.copyToHost(logits_host.data(), config_.vocab_size);

    int32_t next_token = sampleToken(logits_host.data(), config);

    // Generated tokens
    std::vector<int32_t> generated_ids;
    generated_ids.push_back(next_token);

    // Autoregressive generation
    auto gen_start = std::chrono::high_resolution_clock::now();

    size_t position = input_ids.size();
    for (size_t i = 1; i < config.max_tokens; ++i) {
        // Check for EOS
        if (next_token == tokenizer_->getEosTokenId()) {
            LOG_INFO("EOS token generated");
            break;
        }

        // Forward single token
        model_->forward(
            logits_.get(),
            &next_token,
            1,
            position
        );
        CUDA_CHECK(cudaDeviceSynchronize());

        // Sample next token
        logits_.copyToHost(logits_host.data(), config_.vocab_size);
        next_token = sampleToken(logits_host.data(), config);
        generated_ids.push_back(next_token);
        position++;

        // Print token for visibility
        std::string token_str = tokenizer_->decodeToken(next_token);
        std::cout << token_str << std::flush;
    }
    std::cout << std::endl;

    auto gen_end = std::chrono::high_resolution_clock::now();
    stats_.generation_time_ms = std::chrono::duration<double, std::milli>(
        gen_end - gen_start).count();
    stats_.generated_tokens = generated_ids.size();

    // Decode generated tokens
    std::string output = tokenizer_->decode(generated_ids);

    LOG_INFO("Generated " << stats_.generated_tokens << " tokens in "
             << stats_.generation_time_ms << " ms ("
             << stats_.tokensPerSecond() << " tokens/sec)");

    return output;
}

}  // namespace nemotron
