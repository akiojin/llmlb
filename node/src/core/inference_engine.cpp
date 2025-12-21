#include "core/inference_engine.h"

#include "core/engine_registry.h"
#include "core/llama_engine.h"
#include "core/nemotron_engine.h"
#include "models/model_descriptor.h"
#include "models/model_storage.h"
#include "models/model_sync.h"

#include <spdlog/spdlog.h>
#include <sstream>
#include <cctype>

namespace llm_node {

namespace {
std::vector<std::string> split_tokens(const std::string& text, size_t max_tokens) {
    std::vector<std::string> tokens;
    std::string current;
    for (char c : text) {
        if (std::isspace(static_cast<unsigned char>(c))) {
            if (!current.empty()) {
                tokens.push_back(current);
                if (tokens.size() >= max_tokens) break;
                current.clear();
            }
        } else {
            current.push_back(c);
        }
    }
    if (!current.empty() && tokens.size() < max_tokens) {
        tokens.push_back(current);
    }
    return tokens;
}

std::optional<ModelDescriptor> resolve_descriptor(
    const ModelStorage* storage,
    const ModelSync* sync,
    const std::string& model_name) {
    if (!storage) return std::nullopt;

    auto desc = storage->resolveDescriptor(model_name);
    if (desc) return desc;

    if (sync) {
        auto remote = sync->getRemotePath(model_name);
        if (!remote.empty()) {
            ModelDescriptor fallback;
            fallback.name = model_name;
            fallback.runtime = "llama_cpp";
            fallback.format = "gguf";
            fallback.primary_path = remote;
            fallback.model_dir = "";
            return fallback;
        }
    }

    return std::nullopt;
}
}  // namespace

InferenceEngine::InferenceEngine(LlamaManager& manager, ModelStorage& model_storage, ModelSync* model_sync)
    : manager_(&manager)
    , model_storage_(&model_storage)
    , model_sync_(model_sync) {
    engines_ = std::make_unique<EngineRegistry>();
    engines_->registerEngine(std::make_unique<LlamaEngine>(manager));
    engines_->registerEngine(std::make_unique<NemotronEngine>());
}

InferenceEngine::~InferenceEngine() noexcept = default;

std::string InferenceEngine::buildChatPrompt(const std::vector<ChatMessage>& messages) const {
    std::ostringstream oss;
    for (const auto& msg : messages) {
        if (msg.role == "system") {
            oss << "System: " << msg.content << "\n\n";
        } else if (msg.role == "user") {
            oss << "User: " << msg.content << "\n\n";
        } else if (msg.role == "assistant") {
            oss << "Assistant: " << msg.content << "\n\n";
        }
    }
    oss << "Assistant: ";
    return oss.str();
}

std::string InferenceEngine::generateChat(
    const std::vector<ChatMessage>& messages,
    const std::string& model,
    const InferenceParams& params) const {

    if (!isInitialized()) {
        if (messages.empty()) return "";
        return "Response to: " + messages.back().content;
    }

    auto desc = resolve_descriptor(model_storage_, model_sync_, model);
    if (!desc) {
        throw std::runtime_error("Model not found: " + model);
    }

    Engine* engine = engines_ ? engines_->resolve(desc->runtime) : nullptr;
    if (!engine) {
        throw std::runtime_error("No engine registered for runtime: " + desc->runtime);
    }

    return engine->generateChat(messages, *desc, params);
}

std::string InferenceEngine::generateCompletion(
    const std::string& prompt,
    const std::string& model,
    const InferenceParams& params) const {
    if (!isInitialized()) {
        return "Response to: " + prompt;
    }

    auto desc = resolve_descriptor(model_storage_, model_sync_, model);
    if (!desc) {
        throw std::runtime_error("Model not found: " + model);
    }

    Engine* engine = engines_ ? engines_->resolve(desc->runtime) : nullptr;
    if (!engine) {
        throw std::runtime_error("No engine registered for runtime: " + desc->runtime);
    }

    return engine->generateCompletion(prompt, *desc, params);
}

std::vector<std::string> InferenceEngine::generateChatStream(
    const std::vector<ChatMessage>& messages,
    const std::string& model,
    const InferenceParams& params,
    const std::function<void(const std::string&)>& on_token) const {

    if (!isInitialized()) {
        std::string text = messages.empty() ? "" : "Response to: " + messages.back().content;
        auto tokens = split_tokens(text, params.max_tokens);
        for (const auto& t : tokens) {
            if (on_token) on_token(t);
        }
        if (on_token) on_token("[DONE]");
        return tokens;
    }

    auto desc = resolve_descriptor(model_storage_, model_sync_, model);
    if (!desc) {
        throw std::runtime_error("Model not found: " + model);
    }

    Engine* engine = engines_ ? engines_->resolve(desc->runtime) : nullptr;
    if (!engine) {
        throw std::runtime_error("No engine registered for runtime: " + desc->runtime);
    }

    return engine->generateChatStream(messages, *desc, params, on_token);
}

std::vector<std::string> InferenceEngine::generateChatStream(
    const std::vector<ChatMessage>& messages,
    size_t max_tokens,
    const std::function<void(const std::string&)>& on_token) const {
    std::string text = generateChat(messages, "");
    auto tokens = split_tokens(text, max_tokens);
    for (const auto& t : tokens) {
        if (on_token) on_token(t);
    }
    return tokens;
}

std::vector<std::vector<std::string>> InferenceEngine::generateBatch(
    const std::vector<std::string>& prompts,
    size_t max_tokens) const {
    std::vector<std::vector<std::string>> outputs;
    outputs.reserve(prompts.size());
    for (const auto& p : prompts) {
        outputs.push_back(split_tokens(p, max_tokens));
    }
    return outputs;
}

std::vector<std::string> InferenceEngine::generateTokens(
    const std::string& prompt,
    size_t max_tokens) const {
    return split_tokens(prompt, max_tokens);
}

std::string InferenceEngine::sampleNextToken(const std::vector<std::string>& tokens) const {
    if (tokens.empty()) return "";
    return tokens.back();
}

ModelLoadResult InferenceEngine::loadModel(const std::string& model_name) {
    ModelLoadResult result;

    if (!isInitialized()) {
        result.error_message = "InferenceEngine not initialized";
        return result;
    }

    auto desc = resolve_descriptor(model_storage_, model_sync_, model_name);
    if (!desc) {
        result.error_message = "Model not found: " + model_name;
        return result;
    }

    Engine* engine = engines_ ? engines_->resolve(desc->runtime) : nullptr;
    if (!engine) {
        result.error_message = "No engine registered for runtime: " + desc->runtime;
        return result;
    }

    result = engine->loadModel(*desc);
    if (result.success) {
        model_max_ctx_ = engine->getModelMaxContext(*desc);
    }
    return result;
}

std::vector<std::vector<float>> InferenceEngine::generateEmbeddings(
    const std::vector<std::string>& inputs,
    const std::string& model_name) const {

    if (!isInitialized()) {
        std::vector<std::vector<float>> results;
        results.reserve(inputs.size());
        for (size_t i = 0; i < inputs.size(); ++i) {
            results.push_back({1.0f, 0.0f, -1.0f});
        }
        return results;
    }

    auto desc = resolve_descriptor(model_storage_, model_sync_, model_name);
    if (!desc) {
        throw std::runtime_error("Model not found: " + model_name);
    }

    Engine* engine = engines_ ? engines_->resolve(desc->runtime) : nullptr;
    if (!engine) {
        throw std::runtime_error("No engine registered for runtime: " + desc->runtime);
    }

    return engine->generateEmbeddings(inputs, *desc);
}

bool InferenceEngine::isModelSupported(const ModelDescriptor& descriptor) const {
    Engine* engine = engines_ ? engines_->resolve(descriptor.runtime) : nullptr;
    if (!engine) return false;
    return engine->supportsTextGeneration();
}

}  // namespace llm_node
