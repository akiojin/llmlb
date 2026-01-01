#include "core/inference_engine.h"

#include "core/engine_registry.h"
#include "core/gptoss_engine.h"
#include "core/llama_engine.h"
#include "core/llama_manager.h"
#include "core/nemotron_engine.h"
#include "core/vision_processor.h"
#include "include/llama.h"
#include "models/model_descriptor.h"
#include "models/model_resolver.h"
#include "models/model_storage.h"
#include "models/model_sync.h"
#include "mtmd.h"
#include "mtmd-helper.h"

#include <spdlog/spdlog.h>

#include <algorithm>
#include <chrono>
#include <cctype>
#include <filesystem>
#include <sstream>

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
    const std::string& model_name) {
    if (!storage) return std::nullopt;

    auto desc = storage->resolveDescriptor(model_name);
    if (desc) return desc;

    return std::nullopt;
}

// ChatML形式でプロンプトを構築するフォールバック関数
static std::string buildChatMLPrompt(const std::vector<ChatMessage>& messages) {
    std::ostringstream oss;
    for (const auto& msg : messages) {
        oss << "<|im_start|>" << msg.role << "\n" << msg.content << "<|im_end|>\n";
    }
    // アシスタント応答の開始
    oss << "<|im_start|>assistant\n";
    return oss.str();
}

// 制御トークンを除去してトリム
static std::string stripControlTokens(std::string text) {
    const std::vector<std::string> tokens = {
        "<|start|>", "<|end|>", "<|message|>", "<|channel|>",
        "<|im_start|>", "<|im_end|>", "<s>", "</s>", "<|endoftext|>", "<|eot_id|>"
    };
    for (const auto& t : tokens) {
        size_t pos = 0;
        while ((pos = text.find(t, pos)) != std::string::npos) {
            text.erase(pos, t.size());
        }
    }
    auto l = text.find_first_not_of(" \t\n\r");
    if (l == std::string::npos) return "";
    auto r = text.find_last_not_of(" \t\n\r");
    return text.substr(l, r - l + 1);
}

// gpt-ossテンプレート（モデル側にテンプレが無い場合のフォールバック）。ユーザー入力は改変しない。
static const char* GPT_OSS_TEMPLATE = R"tmpl({% for message in messages %}
{% if message['role'] == 'system' %}
<|start|>system<|message|>{{ message['content'] }}<|end|>
{% elif message['role'] == 'user' %}
<|start|>user<|message|>{{ message['content'] }}<|end|>
{% elif message['role'] == 'assistant' %}
<|start|>assistant<|channel|>final<|message|>{{ message['content'] }}<|end|>
{% endif %}
{% endfor %}
<|start|>assistant<|channel|>final<|message|>
)tmpl";

// gpt-oss: finalチャンネルだけを抽出して制御トークンを除去
static std::string extractGptOssFinalMessage(const std::string& output) {
    const std::string marker = "<|channel|>final<|message|>";
    const std::string end = "<|end|>";

    size_t mpos = output.rfind(marker);
    if (mpos == std::string::npos) return output;
    size_t start = mpos + marker.size();
    size_t endpos = output.find(end, start);
    std::string seg = endpos == std::string::npos ? output.substr(start) : output.substr(start, endpos - start);
    return stripControlTokens(seg);
}

// gpt-oss形式でプロンプトを構築する関数
// gpt-oss固有トークン: <|start|>, <|message|>, <|end|>, <|channel|>
// 応答形式: <|start|>assistant<|channel|>final<|message|>content<|end|>
// Reasoning: none を設定して推論チャンネルを無効化
static std::string buildGptOssPrompt(const std::vector<ChatMessage>& messages) {
    std::ostringstream oss;

    // システムメッセージの有無をチェック
    bool hasSystemMessage = false;
    for (const auto& msg : messages) {
        if (msg.role == "system") {
            hasSystemMessage = true;
            break;
        }
    }

    // システムメッセージがない場合、推論無効のシステムプロンプトを追加
    if (!hasSystemMessage) {
        oss << "<|start|>system<|message|>You are a helpful assistant.\n\nReasoning: none<|end|>";
    }

    for (const auto& msg : messages) {
        if (msg.role == "system") {
            // システムメッセージに推論設定を追加
            oss << "<|start|>system<|message|>" << msg.content << "\n\nReasoning: none<|end|>";
        } else {
            oss << "<|start|>" << msg.role << "<|message|>" << msg.content << "<|end|>";
        }
    }

    // アシスタント応答の開始（final チャンネルでコンテンツを直接生成）
    oss << "<|start|>assistant<|channel|>final<|message|>";
    return oss.str();
}

// gpt-ossモデルの出力から特殊トークンを除去する後処理関数
static std::string cleanGptOssOutput(const std::string& output) {
    const std::string marker = "<|channel|>final<|message|>";
    if (output.find(marker) != std::string::npos) {
        return extractGptOssFinalMessage(output);
    }

    std::string result = output;

    // gpt-ossおよびChatMLの特殊トークンリスト
    const std::vector<std::string> tokens_to_remove = {
        // gpt-oss tokens
        "<|start|>", "<|end|>", "<|message|>", "<|channel|>",
        "<|startoftext|>", "<|endoftext|>", "<|return|>", "<|call|>",
        "<|constrain|>", "<|endofprompt|>",
        // ChatML tokens
        "<|im_start|>", "<|im_end|>", "<|assistant>", "<|user>", "<|system>",
        // Common control tokens
        "<|eot_id|>", "</s>", "<s>", "<|begin_of_text|>", "<|end_of_text|>"
    };

    // 特殊トークンを除去
    for (const auto& token : tokens_to_remove) {
        size_t pos = 0;
        while ((pos = result.find(token, pos)) != std::string::npos) {
            result.erase(pos, token.length());
        }
    }

    // "to=" パターンを全て除去（例: "to=assistant", "to=You", "to=user"）
    // 正規表現的に "to=" + 英数字列 を除去
    {
        size_t pos = 0;
        while ((pos = result.find("to=", pos)) != std::string::npos) {
            size_t end_pos = pos + 3;  // "to=" の後ろ
            // 英数字とアンダースコアが続く間は除去対象
            while (end_pos < result.size() &&
                   (std::isalnum(static_cast<unsigned char>(result[end_pos])) ||
                    result[end_pos] == '_')) {
                end_pos++;
            }
            result.erase(pos, end_pos - pos);
        }
    }

    // チャンネル名やロール名を含むパターンを除去
    // 例: "assistantanalysis:", "analysis:", "final:", "assistantfinal:", etc.
    const std::vector<std::string> channel_patterns = {
        // 連結パターン（優先度高）
        "assistantanalysis:", "assistantfinal:", "assistantcommentary:",
        "useranalysis:", "userfinal:", "usercommentary:",
        "systemanalysis:", "systemfinal:", "systemcommentary:",
        // 単独パターン
        "analysis:", "final:", "commentary:",
        "assistant:", "user:", "system:", "developer:",
        // "=name" パターン
        "=assistant", "=analysis", "=final", "=commentary",
        "=user", "=system", "=developer"
    };
    for (const auto& pattern : channel_patterns) {
        size_t pos = 0;
        while ((pos = result.find(pattern, pos)) != std::string::npos) {
            result.erase(pos, pattern.length());
        }
    }

    // 行頭のチャンネル名（コロンなし）を除去
    const std::vector<std::string> channel_names = {
        "assistant", "analysis", "final", "commentary", "user", "system", "developer"
    };
    for (const auto& name : channel_names) {
        // 行頭の "name\n" パターン
        std::string line_pattern = "\n" + name + "\n";
        size_t pos = 0;
        while ((pos = result.find(line_pattern, pos)) != std::string::npos) {
            result.erase(pos + 1, name.length() + 1);  // 最初の\nは残す
        }
        // 文字列先頭の場合
        if (result.find(name + "\n") == 0) {
            result.erase(0, name.length() + 1);
        }
    }

    // 先頭と末尾の空白を除去
    size_t start = result.find_first_not_of(" \t\n\r");
    if (start == std::string::npos) {
        return "";
    }
    size_t end = result.find_last_not_of(" \t\n\r");
    return result.substr(start, end - start + 1);
}

// モデルがgpt-oss形式かどうかを判定
// モデルのテンプレートやアーキテクチャから判定する
static bool isGptOssModel(llama_model* model) {
    // 1. アーキテクチャ名で判定（最も確実）
    char arch_buf[64] = {0};
    int arch_len = llama_model_meta_val_str(model, "general.architecture", arch_buf, sizeof(arch_buf));
    spdlog::info("isGptOssModel: arch_len={}, arch_buf='{}'", arch_len, arch_buf);
    if (arch_len > 0) {
        std::string arch(arch_buf);
        spdlog::info("isGptOssModel: checking architecture '{}'", arch);
        if (arch == "gpt-oss") {
            spdlog::info("Detected gpt-oss model by architecture: {}", arch);
            return true;
        }
    }

    // 2. チャットテンプレートにgpt-oss固有トークンが含まれているかチェック
    const char* tmpl = llama_model_chat_template(model, nullptr);
    spdlog::info("isGptOssModel: chat_template={}", tmpl != nullptr ? tmpl : "(null)");
    if (tmpl != nullptr && tmpl[0] != '\0') {
        std::string template_str(tmpl);
        if (template_str.find("<|start|>") != std::string::npos ||
            template_str.find("<|message|>") != std::string::npos) {
            spdlog::info("Detected gpt-oss model by chat template tokens");
            return true;
        }
    }

    spdlog::info("isGptOssModel: not detected as gpt-oss");
    return false;
}

// モデル固有のチャットテンプレートを適用してプロンプトを構築
static std::string applyModelChatTemplate(
    llama_model* model,
    const std::vector<ChatMessage>& messages) {

    // gpt-ossモデルの場合はgpt-oss専用形式を使用
    if (isGptOssModel(model)) {
        spdlog::info("Detected gpt-oss model, using gpt-oss chat format");
        return buildGptOssPrompt(messages);
    }

    // llama_chat_message 配列を構築
    std::vector<llama_chat_message> llama_messages;
    llama_messages.reserve(messages.size());
    for (const auto& msg : messages) {
        llama_messages.push_back({msg.role.c_str(), msg.content.c_str()});
    }

    // モデルからチャットテンプレートを取得
    const char* tmpl = llama_model_chat_template(model, nullptr);

    // テンプレートがない場合はgpt-oss用テンプレかChatMLにフォールバック
    if (tmpl == nullptr || tmpl[0] == '\0') {
        if (isGptOssModel(model)) {
            spdlog::info("Model has no chat template, using built-in gpt-oss template");
            tmpl = GPT_OSS_TEMPLATE;
        } else {
            spdlog::info("Model has no chat template, using ChatML format");
            return buildChatMLPrompt(messages);
        }
    }

    spdlog::debug("Model chat template found: {}", tmpl);

    // 初回呼び出しで必要なバッファサイズを取得
    int32_t required_size = llama_chat_apply_template(
        tmpl,
        llama_messages.data(),
        llama_messages.size(),
        true,  // add_ass: アシスタント応答の開始を追加
        nullptr,
        0);

    if (required_size < 0) {
        // テンプレート適用に失敗した場合、ChatML形式にフォールバック
        spdlog::warn("llama_chat_apply_template failed (size={}), using ChatML fallback", required_size);
        return buildChatMLPrompt(messages);
    }

    // バッファを確保してテンプレートを適用
    std::vector<char> buf(static_cast<size_t>(required_size) + 1);
    int32_t actual_size = llama_chat_apply_template(
        tmpl,
        llama_messages.data(),
        llama_messages.size(),
        true,
        buf.data(),
        static_cast<int32_t>(buf.size()));

    if (actual_size < 0 || actual_size > static_cast<int32_t>(buf.size())) {
        spdlog::error("llama_chat_apply_template failed on second call");
        // ChatML形式にフォールバック
        return buildChatMLPrompt(messages);
    }

    std::string prompt(buf.data(), static_cast<size_t>(actual_size));
    spdlog::debug("Applied chat template: {} chars", prompt.size());
    return prompt;
}
}  // namespace

InferenceEngine::InferenceEngine(LlamaManager& manager, ModelStorage& model_storage, ModelSync* model_sync,
                                 ModelResolver* model_resolver)
    : manager_(&manager)
    , model_storage_(&model_storage)
    , model_sync_(model_sync)
    , model_resolver_(model_resolver) {
    engines_ = std::make_unique<EngineRegistry>();
    EngineRegistration llama_reg;
    llama_reg.engine_id = "builtin_llama_cpp";
    llama_reg.engine_version = "builtin";
    llama_reg.formats = {"gguf"};
    llama_reg.capabilities = {"text", "embeddings"};
    engines_->registerEngine(std::make_unique<LlamaEngine>(manager), llama_reg, nullptr);

    EngineRegistration gptoss_reg;
    gptoss_reg.engine_id = "builtin_gptoss_cpp";
    gptoss_reg.engine_version = "builtin";
    gptoss_reg.formats = {"safetensors"};
    gptoss_reg.capabilities = {"text"};
    engines_->registerEngine(std::make_unique<GptOssEngine>(), gptoss_reg, nullptr);

    EngineRegistration nemotron_reg;
    nemotron_reg.engine_id = "builtin_nemotron_cpp";
    nemotron_reg.engine_version = "builtin";
    nemotron_reg.formats = {"safetensors"};
    nemotron_reg.capabilities = {"text"};
    engines_->registerEngine(std::make_unique<NemotronEngine>(), nemotron_reg, nullptr);
    vision_processor_ = std::make_unique<VisionProcessor>(model_storage);
}

InferenceEngine::InferenceEngine() = default;

InferenceEngine::~InferenceEngine() = default;

bool InferenceEngine::loadEnginePlugins(const std::filesystem::path& directory, std::string& error) {
    if (!engines_) {
        error = "EngineRegistry not initialized";
        return false;
    }

    EngineHostContext context;
    context.abi_version = EngineHost::kAbiVersion;
    context.models_dir = model_storage_ ? model_storage_->modelsDir().c_str() : nullptr;
    context.llama_manager = manager_;

    return engine_host_.loadPluginsFromDir(directory, *engines_, context, error);
}

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

std::string InferenceEngine::resolveModelPath(const std::string& model_name, std::string* error_message) const {
    if (!isInitialized()) {
        if (error_message) *error_message = "InferenceEngine not initialized";
        return "";
    }

    if (model_resolver_ != nullptr) {
        auto resolved = model_resolver_->resolve(model_name);
        if (resolved.success) {
            return resolved.path;
        }
        if (error_message) *error_message = resolved.error_message;
        return "";
    }

    std::string gguf_path = model_storage_->resolveGguf(model_name);
    if (!gguf_path.empty()) {
        return gguf_path;
    }

    if (error_message) *error_message = "Model not found: " + model_name;
    return "";
}

std::string InferenceEngine::generateChat(
    const std::vector<ChatMessage>& messages,
    const std::string& model,
    const InferenceParams& params) const {

    if (!isInitialized()) {
        spdlog::warn("InferenceEngine not initialized, using stub mode");
        if (messages.empty()) return "";
        return "Response to: " + messages.back().content;
    }

    auto desc = resolve_descriptor(model_storage_, model);
    if (!desc) {
        throw std::runtime_error("Model not found: " + model);
    }

    Engine* engine = engines_ ? engines_->resolve(*desc, "text") : nullptr;
    if (!engine) {
        throw std::runtime_error("No engine registered for runtime: " + desc->runtime);
    }

    return engine->generateChat(messages, *desc, params);
}

std::string InferenceEngine::generateChatWithImages(
    const std::vector<ChatMessage>& messages,
    const std::vector<std::string>& image_urls,
    const std::string& model_name,
    const InferenceParams& params) const {

    if (image_urls.empty()) {
        return generateChat(messages, model_name, params);
    }

    if (!isInitialized()) {
        spdlog::warn("InferenceEngine not initialized, using stub mode for vision");
        if (messages.empty()) return "";
        return "Response to: " + messages.back().content;
    }

    std::string error;
    std::string gguf_path = resolveModelPath(model_name, &error);
    if (gguf_path.empty()) {
        std::string msg = error.empty() ? "Model not found: " + model_name : error;
        spdlog::error("{}", msg);
        throw std::runtime_error(msg);
    }

    if (!manager_->loadModelIfNeeded(gguf_path)) {
        throw std::runtime_error("Failed to load model: " + gguf_path);
    }

    llama_context* ctx = manager_->getContext(gguf_path);
    llama_model* model = manager_->getModel(gguf_path);

    if (!ctx || !model) {
        throw std::runtime_error("Failed to get context/model for: " + gguf_path);
    }

    if (!vision_processor_) {
        vision_processor_ = std::make_unique<VisionProcessor>(*model_storage_);
    }

    std::string vision_error;
    mtmd_context* mctx = vision_processor_->getOrCreateContext(model_name, gguf_path, model, vision_error);
    if (!mctx) {
        throw std::runtime_error(vision_error.empty() ? "Vision model not available" : vision_error);
    }

    mtmd::bitmaps bitmaps;
    if (!vision_processor_->prepareBitmaps(mctx, image_urls, bitmaps, vision_error)) {
        throw std::runtime_error(vision_error.empty() ? "Failed to prepare images" : vision_error);
    }

    std::string prompt = applyModelChatTemplate(model, messages);
    spdlog::debug("Vision prompt: {}", prompt);

    bool is_gptoss = isGptOssModel(model);
    bool add_special = !is_gptoss;
    bool parse_special = is_gptoss;

    mtmd_input_text text;
    text.text = prompt.c_str();
    text.add_special = add_special;
    text.parse_special = parse_special;

    mtmd::input_chunks chunks(mtmd_input_chunks_init());
    auto bitmaps_c_ptr = bitmaps.c_ptr();
    int32_t res = mtmd_tokenize(mctx,
                                chunks.ptr.get(),
                                &text,
                                bitmaps_c_ptr.data(),
                                bitmaps_c_ptr.size());
    if (res != 0) {
        throw std::runtime_error("Failed to tokenize vision prompt");
    }

    llama_memory_t mem = llama_get_memory(ctx);
    if (mem) {
        // Reset sequence positions to avoid KV cache position mismatches across requests.
        llama_memory_clear(mem, false);
    }

    llama_pos new_n_past = 0;
    const int32_t n_batch = llama_n_batch(ctx);
    if (mtmd_helper_eval_chunks(mctx,
                                ctx,
                                chunks.ptr.get(),
                                0,
                                0,
                                n_batch,
                                true,
                                &new_n_past) != 0) {
        throw std::runtime_error("Failed to evaluate vision prompt");
    }

    size_t prompt_positions = new_n_past < 0 ? 0 : static_cast<size_t>(new_n_past);
    spdlog::debug("Vision prompt positions: {}", prompt_positions);

    llama_sampler_chain_params sparams = llama_sampler_chain_default_params();
    llama_sampler* sampler = llama_sampler_chain_init(sparams);

    llama_sampler_chain_add(sampler, llama_sampler_init_top_k(params.top_k));
    llama_sampler_chain_add(sampler, llama_sampler_init_top_p(params.top_p, 1));
    llama_sampler_chain_add(sampler, llama_sampler_init_temp(params.temperature));
    llama_sampler_chain_add(sampler, llama_sampler_init_penalties(
        64,
        params.repeat_penalty,
        0.0f,
        0.0f
    ));

    uint32_t seed = params.seed;
    if (seed == 0) {
        seed = static_cast<uint32_t>(
            std::chrono::steady_clock::now().time_since_epoch().count() & 0xFFFFFFFF);
    }
    llama_sampler_chain_add(sampler, llama_sampler_init_dist(seed));

    std::string output;

    size_t effective_max_tokens = params.max_tokens;
    int32_t model_n_ctx = llama_model_n_ctx_train(model);
    if (model_n_ctx > 0 && prompt_positions < static_cast<size_t>(model_n_ctx)) {
        size_t available = static_cast<size_t>(model_n_ctx) - prompt_positions;
        constexpr size_t DEFAULT_MAX_TOKENS = 2048;
        if (params.max_tokens == DEFAULT_MAX_TOKENS || params.max_tokens == 0) {
            effective_max_tokens = available;
        } else {
            effective_max_tokens = std::min(params.max_tokens, available);
        }
        spdlog::info("Vision: Dynamic max_tokens: model_ctx={}, prompt_pos={}, available={}, effective={}",
            model_n_ctx, prompt_positions, available, effective_max_tokens);
    }

    const llama_vocab* vocab = llama_model_get_vocab(model);

    for (size_t i = 0; i < effective_max_tokens; i++) {
        llama_token new_token = llama_sampler_sample(sampler, ctx, -1);

        if (llama_vocab_is_eog(vocab, new_token)) {
            spdlog::debug("Vision: EOG token received at position {}", i);
            break;
        }

        char buf[256];
        int32_t len = llama_token_to_piece(vocab, new_token, buf, sizeof(buf), 0, false);
        if (len > 0) {
            output.append(buf, static_cast<size_t>(len));
        }

        llama_sampler_accept(sampler, new_token);

        llama_batch next_batch = llama_batch_get_one(&new_token, 1);
        int32_t gen_decode_result = llama_decode(ctx, next_batch);
        if (gen_decode_result != 0) {
            spdlog::warn("Vision: llama_decode failed during generation: {}", gen_decode_result);
            break;
        }
    }

    llama_sampler_free(sampler);

    static const std::vector<std::string> stop_sequences = {
        "<|im_end|>",
        "<|end|>",
        "<|start|>",
        "<|eot_id|>",
        "</s>",
        "<|endoftext|>",
    };

    for (const auto& stop : stop_sequences) {
        size_t pos = output.find(stop);
        if (pos != std::string::npos) {
            spdlog::debug("Vision: Truncating output at stop sequence '{}' at position {}", stop, pos);
            output = output.substr(0, pos);
            break;
        }
    }

    if (isGptOssModel(model)) {
        spdlog::info("Vision: Applying gpt-oss output cleanup, before: {} chars", output.size());
        output = cleanGptOssOutput(output);
        spdlog::info("Vision: After cleanup: {} chars", output.size());
    }

    spdlog::info("Vision: Generated {} bytes for model {}", output.size(), model_name);
    return output;
}

std::string InferenceEngine::generateCompletion(
    const std::string& prompt,
    const std::string& model,
    const InferenceParams& params) const {
    if (!isInitialized()) {
        return "Response to: " + prompt;
    }

    auto desc = resolve_descriptor(model_storage_, model);
    if (!desc) {
        throw std::runtime_error("Model not found: " + model);
    }

    Engine* engine = engines_ ? engines_->resolve(*desc, "text") : nullptr;
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
        spdlog::warn("InferenceEngine not initialized, using stub mode for streaming");
        std::string text = messages.empty() ? "" : "Response to: " + messages.back().content;
        auto tokens = split_tokens(text, params.max_tokens);
        for (const auto& t : tokens) {
            if (on_token) on_token(t);
        }
        if (on_token) on_token("[DONE]");
        return tokens;
    }

    auto desc = resolve_descriptor(model_storage_, model);
    if (!desc) {
        throw std::runtime_error("Model not found: " + model);
    }

    Engine* engine = engines_ ? engines_->resolve(*desc, "text") : nullptr;
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

ModelLoadResult InferenceEngine::loadModel(const std::string& model_name, const std::string& capability) {
    ModelLoadResult result;

    if (!isInitialized()) {
        result.error_message = "InferenceEngine not initialized";
        return result;
    }

    auto desc = resolve_descriptor(model_storage_, model_name);
    if (!desc) {
        result.error_message = "Model not found: " + model_name;
        return result;
    }

    if (!capability.empty() && !desc->capabilities.empty()) {
        if (std::find(desc->capabilities.begin(), desc->capabilities.end(), capability) == desc->capabilities.end()) {
            result.error_message = "Model does not support capability: " + capability;
            return result;
        }
    }

    Engine* engine = engines_ ? engines_->resolve(*desc, capability) : nullptr;
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

    auto desc = resolve_descriptor(model_storage_, model_name);
    if (!desc) {
        throw std::runtime_error("Model not found: " + model_name);
    }

    Engine* engine = engines_ ? engines_->resolve(*desc, "embeddings") : nullptr;
    if (!engine) {
        throw std::runtime_error("No engine registered for runtime: " + desc->runtime);
    }

    return engine->generateEmbeddings(inputs, *desc);
}

#ifdef LLM_NODE_TESTING
void InferenceEngine::setEngineRegistryForTest(std::unique_ptr<EngineRegistry> registry) {
    engines_ = std::move(registry);
}
#endif

bool InferenceEngine::isModelSupported(const ModelDescriptor& descriptor) const {
    Engine* engine = engines_ ? engines_->resolve(descriptor) : nullptr;
    if (!engine) return false;
    if (!engine->supportsTextGeneration()) return false;

    if (descriptor.runtime == "gptoss_cpp") {
        namespace fs = std::filesystem;
        fs::path model_dir = descriptor.model_dir.empty()
                                 ? fs::path(descriptor.primary_path).parent_path()
                                 : fs::path(descriptor.model_dir);
        if (model_dir.empty()) return false;
#if defined(_WIN32)
        if (fs::exists(model_dir / "model.directml.bin")) return true;
        if (fs::exists(model_dir / "model.dml.bin")) return true;
        return false;
#elif defined(__APPLE__)
        if (fs::exists(model_dir / "model.metal.bin")) return true;
        if (fs::exists(model_dir / "metal" / "model.bin")) return true;
        if (fs::exists(model_dir / "model.bin")) return true;
        return false;
#else
        return false;
#endif
    }

    if (descriptor.runtime == "nemotron_cpp") {
#ifndef USE_CUDA
        return false;
#else
        return true;
#endif
    }

    return true;
}

}  // namespace llm_node
