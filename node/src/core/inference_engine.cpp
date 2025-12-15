#include "core/inference_engine.h"

#include "core/onnx_llm_manager.h"
#include "models/model_storage.h"
#include "models/model_sync.h"

#include <spdlog/spdlog.h>

#include <cctype>
#include <sstream>

namespace llm_node {

namespace {

std::string stripControlTokens(std::string text) {
    const std::vector<std::string> tokens = {
        "<|start|>", "<|end|>", "<|message|>", "<|channel|>",
        "<|im_start|>", "<|im_end|>", "<s>", "</s>", "<|endoftext|>", "<|eot_id|>",
    };
    for (const auto& t : tokens) {
        size_t pos = 0;
        while ((pos = text.find(t, pos)) != std::string::npos) {
            text.erase(pos, t.size());
        }
    }
    const auto l = text.find_first_not_of(" \t\n\r");
    if (l == std::string::npos) return "";
    const auto r = text.find_last_not_of(" \t\n\r");
    return text.substr(l, r - l + 1);
}

std::string extractGptOssFinalMessage(const std::string& output) {
    const std::string marker = "<|channel|>final<|message|>";
    const std::string end = "<|end|>";

    const size_t mpos = output.rfind(marker);
    if (mpos == std::string::npos) return stripControlTokens(output);
    const size_t start = mpos + marker.size();
    const size_t endpos = output.find(end, start);
    const std::string seg = endpos == std::string::npos ? output.substr(start) : output.substr(start, endpos - start);
    return stripControlTokens(seg);
}

}  // namespace

// テスト用に公開する薄いラッパー（本番コードには影響なし）
std::string extractGptOssFinalMessageForTest(const std::string& output) {
    return extractGptOssFinalMessage(output);
}

InferenceEngine::InferenceEngine(OnnxLlmManager& manager, ModelStorage& model_storage, ModelSync* model_sync)
    : manager_(&manager)
    , model_storage_(&model_storage)
    , model_sync_(model_sync) {}

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
    const std::string& model_name,
    const InferenceParams&) const {

    if (messages.empty()) return "";

    // Stub mode (tests) when dependencies are not injected.
    if (!isInitialized()) {
        spdlog::warn("InferenceEngine not initialized, using stub mode");
        return "Response to: " + messages.back().content;
    }

    // Ensure the model is loadable (PoC: session creation only).
    auto lr = const_cast<InferenceEngine*>(this)->loadModel(model_name);
    if (!lr.success) {
        throw std::runtime_error(lr.error_message.empty() ? "Failed to load model" : lr.error_message);
    }

    // NOTE: Full ONNX chat inference (tokenization + kv-cache generation loop) is not implemented yet.
    return "Response to: " + messages.back().content;
}

std::string InferenceEngine::generateCompletion(
    const std::string& prompt,
    const std::string& model,
    const InferenceParams& params) const {
    std::vector<ChatMessage> msgs;
    msgs.push_back({"user", prompt});
    return generateChat(msgs, model, params);
}

std::vector<std::string> InferenceEngine::generateChatStream(
    const std::vector<ChatMessage>& messages,
    const std::string& model,
    const InferenceParams& params,
    const std::function<void(const std::string&)>& on_token) const {
    const std::string text = generateChat(messages, model, params);
    auto tokens = generateTokens(text, params.max_tokens);

    for (const auto& t : tokens) {
        if (on_token) on_token(t);
    }
    if (on_token) on_token("[DONE]");
    return tokens;
}

std::vector<std::string> InferenceEngine::generateChatStream(
    const std::vector<ChatMessage>& messages,
    size_t max_tokens,
    const std::function<void(const std::string&)>& on_token) const {
    const std::string text = generateChat(messages, "");
    auto tokens = generateTokens(text, max_tokens);
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
        outputs.push_back(generateTokens(p, max_tokens));
    }
    return outputs;
}

std::vector<std::string> InferenceEngine::generateTokens(
    const std::string& prompt,
    size_t max_tokens) const {
    std::vector<std::string> tokens;
    std::string current;

    for (char c : prompt) {
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

    // 1) Local storage (SPEC-dcaeaec4) - prefer ONNX (model.onnx), fallback to legacy GGUF.
    std::string model_path = model_storage_->resolveOnnx(model_name);
    if (model_path.empty()) {
        model_path = model_storage_->resolveGguf(model_name);
    }

    // 2) Remote path from router (if configured).
    if (model_path.empty() && model_sync_ != nullptr) {
        model_path = model_sync_->getRemotePath(model_name);
        if (!model_path.empty()) {
            spdlog::info("Using remote path for model {}: {}", model_name, model_path);
        }
    }

    if (model_path.empty()) {
        result.error_message = "Model not found: " + model_name;
        return result;
    }

    if (!manager_->loadModelIfNeeded(model_path)) {
        result.error_message = "Failed to load model: " + model_path;
        return result;
    }

    result.success = true;
    return result;
}

std::vector<std::vector<float>> InferenceEngine::generateEmbeddings(
    const std::vector<std::string>& inputs,
    const std::string& model) const {
    std::vector<std::vector<float>> results;

    // Stub mode: keep existing contract tests stable.
    if (!isInitialized()) {
        spdlog::warn("InferenceEngine not initialized, returning dummy embeddings");
        for (size_t i = 0; i < inputs.size(); ++i) {
            results.push_back({1.0f, 0.0f, -1.0f});
        }
        return results;
    }

    auto lr = const_cast<InferenceEngine*>(this)->loadModel(model);
    if (!lr.success) {
        throw std::runtime_error(lr.error_message.empty() ? "Failed to load model" : lr.error_message);
    }

    // NOTE: Full ONNX embedding inference is not implemented yet.
    for (size_t i = 0; i < inputs.size(); ++i) {
        results.push_back({1.0f, 0.0f, -1.0f});
    }
    return results;
}

}  // namespace llm_node
