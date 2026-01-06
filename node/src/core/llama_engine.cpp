#include "core/llama_engine.h"
#include "core/llama_manager.h"
#include "include/llama.h"
#include "utils/stop_sequences.h"

#include <spdlog/spdlog.h>
#include <random>
#include <sstream>
#include <chrono>
#include <cmath>
#include <filesystem>
#include <functional>
#include <utility>

namespace llm_node {

namespace fs = std::filesystem;

namespace {
#ifdef LLM_NODE_TESTING
std::function<void(const char*)> kv_cache_reset_hook;
#endif

void notify_kv_cache_reset(const char* reason) {
#ifdef LLM_NODE_TESTING
    if (kv_cache_reset_hook) {
        kv_cache_reset_hook(reason);
    }
#else
    (void)reason;
#endif
}

void reset_kv_cache(llama_context* ctx, const char* reason) {
    notify_kv_cache_reset(reason);
    if (!ctx) return;
    llama_memory_t mem = llama_get_memory(ctx);
    if (mem) {
        llama_memory_clear(mem, false);
    }
}

struct KvCacheScope {
    explicit KvCacheScope(llama_context* ctx) : ctx_(ctx) {
        reset_kv_cache(ctx_, "request_start");
    }
    ~KvCacheScope() {
        reset_kv_cache(ctx_, "request_end");
    }

    KvCacheScope(const KvCacheScope&) = delete;
    KvCacheScope& operator=(const KvCacheScope&) = delete;

private:
    llama_context* ctx_{nullptr};
};

uint64_t steady_now_ns() {
    return static_cast<uint64_t>(
        std::chrono::duration_cast<std::chrono::nanoseconds>(
            std::chrono::steady_clock::now().time_since_epoch()).count());
}

void emit_token_metrics(const InferenceParams& params, uint32_t token_id) {
    if (!params.on_token_callback) return;
    params.on_token_callback(params.on_token_callback_ctx, token_id, steady_now_ns());
}
}  // namespace

static const std::vector<std::string> kDefaultStopSequences = {
    "<|im_end|>",       // ChatML (Qwen3, etc.)
    "<|end|>",          // gpt-oss, Some models
    "<|start|>",        // gpt-oss (新しいメッセージの開始を検出)
    "<|eot_id|>",       // Llama 3
    "</s>",             // Llama 2, Mistral
    "<|endoftext|>",    // GPT-style
};

// 前方宣言
static std::string stripControlTokens(std::string text);

// コンストラクタ
LlamaEngine::LlamaEngine(LlamaManager& manager)
    : manager_(manager) {}

#ifdef LLM_NODE_TESTING
void LlamaEngine::setKvCacheResetHookForTest(KvCacheResetHook hook) {
    kv_cache_reset_hook = std::move(hook);
}

void LlamaEngine::runKvCacheScopeForTest() {
    KvCacheScope scope(nullptr);
}
#endif

// チャットメッセージからプロンプトを構築（llama_chat_apply_template使用）
std::string LlamaEngine::buildChatPrompt(const std::vector<ChatMessage>& messages) const {
    // この関数はモデルなしで呼ばれる互換性維持用のフォールバック
    // 実際の推論では generateChat/generateChatStream 内で直接テンプレートを適用
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

    // アシスタント応答の開始を示す
    oss << "Assistant: ";
    return oss.str();
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

// テスト用の後処理関数
std::string postProcessGeneratedTextForTest(const std::string& output) {
    std::string processed = output;

    // Keep in sync with LlamaEngine::generateChat() post-processing.
    auto stop_sequences = merge_stop_sequences(kDefaultStopSequences, {});
    apply_stop_sequences_suffix(processed, stop_sequences);

    return processed;
}

// モデル固有のチャットテンプレートを適用してプロンプトを構築
static std::string applyModelChatTemplate(
    llama_model* model,
    const std::vector<ChatMessage>& messages) {

    // llama_chat_message 配列を構築
    std::vector<llama_chat_message> llama_messages;
    llama_messages.reserve(messages.size());
    for (const auto& msg : messages) {
        llama_messages.push_back({msg.role.c_str(), msg.content.c_str()});
    }

    // モデルからチャットテンプレートを取得
    const char* tmpl = llama_model_chat_template(model, nullptr);

    // テンプレートがない場合はChatMLにフォールバック
    if (tmpl == nullptr || tmpl[0] == '\0') {
        spdlog::info("Model has no chat template, using ChatML format");
        return buildChatMLPrompt(messages);
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

// チャット生成（llama.cpp API使用）
std::string LlamaEngine::generateChat(
    const std::vector<ChatMessage>& messages,
    const ModelDescriptor& descriptor,
    const InferenceParams& params) const {

    const std::string gguf_path = descriptor.primary_path;
    if (gguf_path.empty()) {
        throw std::runtime_error("GGUF path is empty for model: " + descriptor.name);
    }

    if (!manager_.loadModelIfNeeded(gguf_path)) {
        throw std::runtime_error("Failed to load model: " + gguf_path);
    }

    llama_context* ctx = manager_.getContext(gguf_path);
    llama_model* model = manager_.getModel(gguf_path);

    if (!ctx || !model) {
        throw std::runtime_error("Failed to get context/model for: " + gguf_path);
    }
    KvCacheScope kv_scope(ctx);

    // 4. プロンプト構築（モデル固有のチャットテンプレートを使用）
    std::string prompt = applyModelChatTemplate(model, messages);
    spdlog::debug("Prompt: {}", prompt);

    // 5. vocab取得
    const llama_vocab* vocab = llama_model_get_vocab(model);
    if (!vocab) {
        throw std::runtime_error("Failed to get vocab from model");
    }

    // 6. トークン化
    // add_special=true: BOSトークンを追加
    // parse_special=true: 特殊トークンをパース
    std::vector<llama_token> tokens(prompt.size() + 128);
    int32_t n_tokens = llama_tokenize(
        vocab,
        prompt.c_str(),
        static_cast<int32_t>(prompt.size()),
        tokens.data(),
        static_cast<int32_t>(tokens.size()),
        true,   // add_special
        true    // parse_special
    );

    if (n_tokens < 0) {
        // バッファが小さすぎる場合、再割り当て
        tokens.resize(static_cast<size_t>(-n_tokens));
        n_tokens = llama_tokenize(
            vocab,
            prompt.c_str(),
            static_cast<int32_t>(prompt.size()),
            tokens.data(),
            static_cast<int32_t>(tokens.size()),
            true,   // add_special
            true    // parse_special
        );
    }

    if (n_tokens < 0) {
        throw std::runtime_error("Failed to tokenize prompt");
    }

    tokens.resize(static_cast<size_t>(n_tokens));
    spdlog::debug("Tokenized prompt: {} tokens", n_tokens);

    // 7. バッチ分割処理でプロンプトをデコード
    const int32_t batch_size = llama_n_batch(ctx);
    spdlog::debug("Decoding prompt with {} tokens in batches of {}", n_tokens, batch_size);

    for (int32_t i = 0; i < n_tokens; i += batch_size) {
        int32_t current_batch_size = std::min(batch_size, n_tokens - i);
        llama_batch batch = llama_batch_get_one(tokens.data() + i, current_batch_size);

        int32_t decode_result = llama_decode(ctx, batch);
        if (decode_result != 0) {
            spdlog::error("llama_decode failed at batch {}/{}: n_tokens={}, batch_size={}, error={}",
                i / batch_size + 1, (n_tokens + batch_size - 1) / batch_size,
                n_tokens, batch_size, decode_result);
            throw std::runtime_error("llama_decode failed");
        }
    }

    // 8. サンプラーチェーン初期化
    llama_sampler_chain_params sparams = llama_sampler_chain_default_params();
    llama_sampler* sampler = llama_sampler_chain_init(sparams);

    // サンプリング戦略を追加
    llama_sampler_chain_add(sampler, llama_sampler_init_top_k(params.top_k));
    llama_sampler_chain_add(sampler, llama_sampler_init_top_p(params.top_p, 1));
    llama_sampler_chain_add(sampler, llama_sampler_init_temp(params.temperature));

    // 繰り返し抑制ペナルティを追加（重要：反復出力を防ぐ）
    llama_sampler_chain_add(sampler, llama_sampler_init_penalties(
        64,                      // last_n: 直近64トークンを考慮
        params.repeat_penalty,   // repeat_penalty: 1.1
        0.0f,                    // frequency_penalty
        0.0f                     // presence_penalty
    ));

    // シード設定
    uint32_t seed = params.seed;
    if (seed == 0) {
        seed = static_cast<uint32_t>(
            std::chrono::steady_clock::now().time_since_epoch().count() & 0xFFFFFFFF);
    }
    llama_sampler_chain_add(sampler, llama_sampler_init_dist(seed));

    // 9. トークン生成ループ
    std::string output;
    auto stop_sequences = merge_stop_sequences(kDefaultStopSequences, params.stop_sequences);
    // int32_t n_cur = n_tokens; // unused

    // 動的max_tokens計算: モデルの最大コンテキストからプロンプト分を差し引く
    size_t effective_max_tokens = params.max_tokens;
    int32_t model_n_ctx = llama_model_n_ctx_train(model);
    if (model_n_ctx > 0) {
        size_t available = 0;
        if (static_cast<size_t>(n_tokens) < static_cast<size_t>(model_n_ctx)) {
            available = static_cast<size_t>(model_n_ctx) - static_cast<size_t>(n_tokens);
        }
        // デフォルト値(2048)の場合は利用可能な全容量を使用、
        // ユーザー指定がある場合はその値と利用可能な残り容量の小さい方を使用
        effective_max_tokens = resolve_effective_max_tokens(params.max_tokens, n_tokens, model_n_ctx);
        spdlog::info("Dynamic max_tokens: model_ctx={}, prompt_tokens={}, available={}, effective={}",
            model_n_ctx, n_tokens, available, effective_max_tokens);
    }

    for (size_t i = 0; i < effective_max_tokens; i++) {
        // トークンサンプリング
        llama_token new_token = llama_sampler_sample(sampler, ctx, -1);

        // EOG（End of Generation）チェック
        if (llama_vocab_is_eog(vocab, new_token)) {
            spdlog::debug("EOG token received at position {}", i);
            break;
        }

        emit_token_metrics(params, static_cast<uint32_t>(new_token));

        // トークンをテキストに変換
        char buf[256];
        int32_t len = llama_token_to_piece(vocab, new_token, buf, sizeof(buf), 0, false);
        if (len > 0) {
            // Debug: log token ID and raw bytes
            std::string hex_bytes;
            for (int32_t j = 0; j < len; j++) {
                char hex[8];
                snprintf(hex, sizeof(hex), "%02X ", static_cast<unsigned char>(buf[j]));
                hex_bytes += hex;
            }
            spdlog::debug("Token {}: id={}, len={}, bytes=[{}]", i, new_token, len, hex_bytes);
            output.append(buf, static_cast<size_t>(len));
            if (apply_stop_sequences_suffix(output, stop_sequences)) {
                break;
            }
        }

        // サンプラーにトークンを通知
        llama_sampler_accept(sampler, new_token);

        // 次のトークン用にバッチを準備
        llama_batch next_batch = llama_batch_get_one(&new_token, 1);
        int32_t gen_decode_result = llama_decode(ctx, next_batch);
        if (gen_decode_result != 0) {
            spdlog::warn("llama_decode failed during generation: {}", gen_decode_result);
            break;
        }

        // n_cur++; // unused
    }

    // 10. クリーンアップ
    llama_sampler_free(sampler);

    // Debug: log final output hex dump (first 100 bytes)
    std::string hex_output;
    for (size_t j = 0; j < std::min(output.size(), size_t(100)); j++) {
        char hex[8];
        snprintf(hex, sizeof(hex), "%02X ", static_cast<unsigned char>(output[j]));
        hex_output += hex;
    }
    spdlog::info("Generated {} bytes for model {}, first 100 bytes: [{}]", output.size(), descriptor.name, hex_output);
    return output;
}

// テキスト補完
std::string LlamaEngine::generateCompletion(
    const std::string& prompt,
    const ModelDescriptor& descriptor,
    const InferenceParams& params) const {

    // チャットメッセージとして処理
    std::vector<ChatMessage> messages = {{"user", prompt}};
    return generateChat(messages, descriptor, params);
}

// ストリーミングチャット生成
std::vector<std::string> LlamaEngine::generateChatStream(
    const std::vector<ChatMessage>& messages,
    const ModelDescriptor& descriptor,
    const InferenceParams& params,
    const std::function<void(const std::string&)>& on_token) const {

    std::vector<std::string> all_tokens;

    const std::string gguf_path = descriptor.primary_path;
    if (gguf_path.empty()) {
        throw std::runtime_error("GGUF path is empty for model: " + descriptor.name);
    }

    if (!manager_.loadModelIfNeeded(gguf_path)) {
        throw std::runtime_error("Failed to load model: " + gguf_path);
    }

    llama_context* ctx = manager_.getContext(gguf_path);
    llama_model* model = manager_.getModel(gguf_path);

    if (!ctx || !model) {
        throw std::runtime_error("Failed to get context/model");
    }
    KvCacheScope kv_scope(ctx);

    // 3. vocab取得とプロンプト処理（モデル固有のチャットテンプレートを使用）
    const llama_vocab* vocab = llama_model_get_vocab(model);
    std::string prompt = applyModelChatTemplate(model, messages);

    // add_special=true: BOSトークンを追加
    // parse_special=true: 特殊トークンをパース
    std::vector<llama_token> tokens(prompt.size() + 128);
    int32_t n_tokens = llama_tokenize(
        vocab, prompt.c_str(), static_cast<int32_t>(prompt.size()),
        tokens.data(), static_cast<int32_t>(tokens.size()), true, true);

    if (n_tokens < 0) {
        tokens.resize(static_cast<size_t>(-n_tokens));
        n_tokens = llama_tokenize(
            vocab, prompt.c_str(), static_cast<int32_t>(prompt.size()),
            tokens.data(), static_cast<int32_t>(tokens.size()), true, true);
    }

    tokens.resize(static_cast<size_t>(n_tokens));

    // 4. バッチ分割処理でプロンプトをデコード
    const int32_t batch_size = llama_n_batch(ctx);
    spdlog::debug("Streaming: Decoding prompt with {} tokens in batches of {}", n_tokens, batch_size);

    for (int32_t i = 0; i < n_tokens; i += batch_size) {
        int32_t current_batch_size = std::min(batch_size, n_tokens - i);
        llama_batch batch = llama_batch_get_one(tokens.data() + i, current_batch_size);

        if (llama_decode(ctx, batch) != 0) {
            spdlog::error("llama_decode failed at batch {}/{}: n_tokens={}, batch_size={}",
                i / batch_size + 1, (n_tokens + batch_size - 1) / batch_size,
                n_tokens, batch_size);
            throw std::runtime_error("llama_decode failed for prompt");
        }
    }

    // 5. サンプラー初期化
    llama_sampler_chain_params sparams = llama_sampler_chain_default_params();
    llama_sampler* sampler = llama_sampler_chain_init(sparams);

    llama_sampler_chain_add(sampler, llama_sampler_init_top_k(params.top_k));
    llama_sampler_chain_add(sampler, llama_sampler_init_top_p(params.top_p, 1));
    llama_sampler_chain_add(sampler, llama_sampler_init_temp(params.temperature));

    // 繰り返し抑制ペナルティを追加（重要：反復出力を防ぐ）
    llama_sampler_chain_add(sampler, llama_sampler_init_penalties(
        64,                      // last_n: 直近64トークンを考慮
        params.repeat_penalty,   // repeat_penalty: 1.1
        0.0f,                    // frequency_penalty
        0.0f                     // presence_penalty
    ));

    uint32_t seed = params.seed;
    if (seed == 0) {
        seed = static_cast<uint32_t>(
            std::chrono::steady_clock::now().time_since_epoch().count() & 0xFFFFFFFF);
    }
    llama_sampler_chain_add(sampler, llama_sampler_init_dist(seed));

    // 6. ストリーミング生成ループ
    auto stop_sequences = merge_stop_sequences(kDefaultStopSequences, params.stop_sequences);
    StopSequenceStream stop_stream(stop_sequences);
    auto emit_chunk = [&](const std::string& chunk) {
        if (chunk.empty()) return;
        all_tokens.push_back(chunk);
        if (on_token) {
            on_token(chunk);
        }
    };

    // 動的max_tokens計算: モデルの最大コンテキストからプロンプト分を差し引く
    size_t effective_max_tokens = params.max_tokens;
    int32_t model_n_ctx = llama_model_n_ctx_train(model);
    if (model_n_ctx > 0) {
        size_t available = 0;
        if (static_cast<size_t>(n_tokens) < static_cast<size_t>(model_n_ctx)) {
            available = static_cast<size_t>(model_n_ctx) - static_cast<size_t>(n_tokens);
        }
        // デフォルト値(2048)の場合は利用可能な全容量を使用、
        // ユーザー指定がある場合はその値と利用可能な残り容量の小さい方を使用
        effective_max_tokens = resolve_effective_max_tokens(params.max_tokens, n_tokens, model_n_ctx);
        spdlog::info("Streaming: Dynamic max_tokens: model_ctx={}, prompt_tokens={}, available={}, effective={}",
            model_n_ctx, n_tokens, available, effective_max_tokens);
    }

    for (size_t i = 0; i < effective_max_tokens && !stop_stream.stopped(); i++) {
        llama_token new_token = llama_sampler_sample(sampler, ctx, -1);

        if (llama_vocab_is_eog(vocab, new_token)) {
            break;
        }

        emit_token_metrics(params, static_cast<uint32_t>(new_token));

        char buf[256];
        int32_t len = llama_token_to_piece(vocab, new_token, buf, sizeof(buf), 0, false);
        if (len > 0) {
            std::string piece(buf, static_cast<size_t>(len));
            stop_stream.push(piece, emit_chunk);
        }

        if (!stop_stream.stopped()) {
            llama_sampler_accept(sampler, new_token);

            llama_batch next_batch = llama_batch_get_one(&new_token, 1);
            if (llama_decode(ctx, next_batch) != 0) {
                break;
            }
        }
    }

    stop_stream.flush(emit_chunk);

    // 完了を通知
    if (on_token) {
        on_token("[DONE]");
    }

    llama_sampler_free(sampler);
    return all_tokens;
}

ModelLoadResult LlamaEngine::loadModel(const ModelDescriptor& descriptor) {
    ModelLoadResult result;
    const std::string gguf_path = descriptor.primary_path;
    if (gguf_path.empty()) {
        result.error_message = "GGUF path is empty for model: " + descriptor.name;
        result.error_code = EngineErrorCode::kLoadFailed;
        return result;
    }

    if (manager_.isLoaded(gguf_path)) {
        result.success = true;
        result.error_code = EngineErrorCode::kOk;
        return result;
    }

    std::error_code ec;
    if (!fs::exists(gguf_path, ec) || ec) {
        result.error_message = "GGUF file not found: " + gguf_path;
        result.error_code = EngineErrorCode::kLoadFailed;
        return result;
    }

    if (!manager_.loadModelIfNeeded(gguf_path)) {
        result.error_message = "Failed to load model: " + gguf_path;
        result.error_code = EngineErrorCode::kLoadFailed;
        return result;
    }

    llama_model* model = manager_.getModel(gguf_path);
    if (model) {
        int32_t n_ctx_train = llama_model_n_ctx_train(model);
        if (n_ctx_train > 0) {
            model_max_ctx_ = static_cast<size_t>(n_ctx_train);
            spdlog::info("Model max context size: {}", model_max_ctx_);
        }
    }

    result.success = true;
    result.error_code = EngineErrorCode::kOk;
    return result;
}

size_t LlamaEngine::getModelMaxContext(const ModelDescriptor& descriptor) const {
    const std::string gguf_path = descriptor.primary_path;
    if (!gguf_path.empty()) {
        llama_model* model = manager_.getModel(gguf_path);
        if (model) {
            int32_t n_ctx_train = llama_model_n_ctx_train(model);
            if (n_ctx_train > 0) {
                return static_cast<size_t>(n_ctx_train);
            }
        }
    }
    return model_max_ctx_;
}

uint64_t LlamaEngine::getModelVramBytes(const ModelDescriptor& descriptor) const {
    if (descriptor.primary_path.empty()) {
        return 0;
    }
    std::error_code ec;
    auto size = fs::file_size(descriptor.primary_path, ec);
    if (ec) {
        return 0;
    }
    return static_cast<uint64_t>(size);
}

// Embedding生成
std::vector<std::vector<float>> LlamaEngine::generateEmbeddings(
    const std::vector<std::string>& inputs,
    const ModelDescriptor& descriptor) const {

    std::vector<std::vector<float>> results;

    // 依存関係が注入されていない場合はスタブモード（ダミーembedding）
    const std::string gguf_path = descriptor.primary_path;
    if (gguf_path.empty()) {
        throw std::runtime_error("GGUF path is empty for model: " + descriptor.name);
    }

    if (!manager_.loadModelIfNeeded(gguf_path)) {
        throw std::runtime_error("Failed to load model: " + gguf_path);
    }

    llama_context* ctx = manager_.getContext(gguf_path);
    llama_model* model = manager_.getModel(gguf_path);

    if (!ctx || !model) {
        throw std::runtime_error("Failed to get context/model for: " + gguf_path);
    }

    // 4. embeddingモードを有効化
    llama_set_embeddings(ctx, true);

    // 5. vocab取得
    const llama_vocab* vocab = llama_model_get_vocab(model);
    if (!vocab) {
        throw std::runtime_error("Failed to get vocab from model");
    }

    // 6. embedding次元を取得
    const int32_t n_embd = llama_model_n_embd(model);

    // 7. 各入力に対してembeddingを生成
    for (const auto& input : inputs) {
        // トークン化
        std::vector<llama_token> tokens(input.size() + 128);
        int32_t n_tokens = llama_tokenize(
            vocab,
            input.c_str(),
            static_cast<int32_t>(input.size()),
            tokens.data(),
            static_cast<int32_t>(tokens.size()),
            true,   // add_special (BOS)
            false   // parse_special
        );

        if (n_tokens < 0) {
            tokens.resize(static_cast<size_t>(-n_tokens));
            n_tokens = llama_tokenize(
                vocab,
                input.c_str(),
                static_cast<int32_t>(input.size()),
                tokens.data(),
                static_cast<int32_t>(tokens.size()),
                true,
                false
            );
        }

        if (n_tokens <= 0) {
            throw std::runtime_error("Failed to tokenize input for embedding");
        }

        tokens.resize(static_cast<size_t>(n_tokens));

        // メモリをクリア（新しい入力をエンコードする前に）
        reset_kv_cache(ctx, "embed_input");

        // バッチを作成（全トークンのembeddingを出力）
        llama_batch batch = llama_batch_init(static_cast<int32_t>(tokens.size()), 0, 1);
        for (int32_t i = 0; i < n_tokens; ++i) {
            batch.token[i] = tokens[static_cast<size_t>(i)];
            batch.pos[i] = i;
            batch.n_seq_id[i] = 1;
            batch.seq_id[i][0] = 0;
            batch.logits[i] = (i == n_tokens - 1) ? 1 : 0;  // 最後のトークンのみ出力
        }
        batch.n_tokens = n_tokens;

        // エンコード（embedding生成）
        int32_t encode_result = llama_encode(ctx, batch);
        if (encode_result != 0) {
            llama_batch_free(batch);
            // llama_encodeが失敗した場合、llama_decodeを試す（一部モデル用）
            spdlog::debug("llama_encode failed, trying llama_decode for embeddings");
            llama_batch batch2 = llama_batch_get_one(tokens.data(), n_tokens);
            if (llama_decode(ctx, batch2) != 0) {
                throw std::runtime_error("Failed to encode/decode for embeddings");
            }
        } else {
            llama_batch_free(batch);
        }

        // embeddingを取得（最後のトークンのembedding）
        const float* embd = llama_get_embeddings_ith(ctx, -1);
        if (embd == nullptr) {
            // pooling_typeがnone以外の場合はseqから取得
            embd = llama_get_embeddings_seq(ctx, 0);
        }

        if (embd == nullptr) {
            spdlog::error("Failed to get embeddings for input");
            // ダミーembeddingを返す
            results.push_back(std::vector<float>(static_cast<size_t>(n_embd), 0.0f));
            continue;
        }

        // embeddingをコピーして正規化
        std::vector<float> embedding(embd, embd + n_embd);

        // L2正規化
        float norm = 0.0f;
        for (float v : embedding) {
            norm += v * v;
        }
        norm = std::sqrt(norm);
        if (norm > 0.0f) {
            for (float& v : embedding) {
                v /= norm;
            }
        }

        results.push_back(std::move(embedding));
    }

    // embeddingモードを無効化（通常のテキスト生成に戻す）
    llama_set_embeddings(ctx, false);

    return results;
}

}  // namespace llm_node
