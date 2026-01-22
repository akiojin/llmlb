#include "core/inference_engine.h"

#include "core/llama_manager.h"
#include "core/text_manager.h"
#include "core/request_watchdog.h"
#include "core/vision_processor.h"
#include "include/llama.h"
#include "models/model_descriptor.h"
#include "models/model_resolver.h"
#include "models/model_storage.h"
#include "models/model_sync.h"
#include "mtmd.h"
#include "mtmd-helper.h"
#include "utils/stop_sequences.h"
#include "runtime/state.h"
#include "system/gpu_detector.h"

#include <spdlog/spdlog.h>

#include <algorithm>
#include <atomic>
#include <chrono>
#include <cctype>
#include <condition_variable>
#include <cstdlib>
#include <filesystem>
#include <mutex>
#include <sstream>
#include <thread>
#include <type_traits>

namespace xllm {

namespace {

// T182: トークン間タイムアウトのデフォルト値（5秒）
constexpr auto kDefaultInterTokenTimeout = std::chrono::milliseconds(5000);

#ifdef XLLM_TESTING
std::mutex g_inter_token_timeout_mutex;
std::chrono::milliseconds g_inter_token_timeout_override{0};
#endif

std::chrono::milliseconds get_inter_token_timeout() {
#ifdef XLLM_TESTING
    std::lock_guard<std::mutex> lock(g_inter_token_timeout_mutex);
    if (g_inter_token_timeout_override.count() > 0) {
        return g_inter_token_timeout_override;
    }
#endif
    return kDefaultInterTokenTimeout;
}

// T182: トークン間タイムアウト監視クラス
class InterTokenWatchdog {
public:
    explicit InterTokenWatchdog(std::chrono::milliseconds timeout)
        : timeout_(timeout)
        , last_reset_(std::chrono::steady_clock::now()) {
        if (timeout_.count() <= 0) {
            active_ = false;
            return;
        }
        thread_ = std::thread([this]() { run(); });
    }

    InterTokenWatchdog(const InterTokenWatchdog&) = delete;
    InterTokenWatchdog& operator=(const InterTokenWatchdog&) = delete;

    ~InterTokenWatchdog() {
        disarm();
        if (thread_.joinable()) {
            thread_.join();
        }
    }

    void reset() {
        std::lock_guard<std::mutex> lock(mutex_);
        last_reset_ = std::chrono::steady_clock::now();
    }

    bool hasTimedOut() const {
        return timed_out_.load(std::memory_order_acquire);
    }

    void disarm() {
        {
            std::lock_guard<std::mutex> lock(mutex_);
            if (!active_) return;
            active_ = false;
        }
        cv_.notify_all();
    }

private:
    void run() {
        std::unique_lock<std::mutex> lock(mutex_);
        while (active_) {
            auto now = std::chrono::steady_clock::now();
            auto elapsed = std::chrono::duration_cast<std::chrono::milliseconds>(now - last_reset_);
            if (elapsed >= timeout_) {
                timed_out_.store(true, std::memory_order_release);
                spdlog::warn("Inter-token timeout: {}ms since last token", elapsed.count());
                return;
            }
            auto remaining = timeout_ - elapsed;
            cv_.wait_for(lock, remaining, [this]() { return !active_; });
        }
    }

    std::chrono::milliseconds timeout_;
    std::chrono::steady_clock::time_point last_reset_;
    std::thread thread_;
    std::mutex mutex_;
    std::condition_variable cv_;
    bool active_{true};
    std::atomic<bool> timed_out_{false};
};

struct TokenMetricsState {
    uint64_t start_ns{0};
    uint64_t first_token_ns{0};
    uint64_t last_token_ns{0};
    size_t token_count{0};
};

// T182: InterTokenWatchdog用のコンテキスト
struct InterTokenWatchdogContext {
    InterTokenWatchdog* watchdog{nullptr};
    TokenMetricsState* metrics{nullptr};
};

// T182: トークン生成時にウォッチドッグをリセットするコールバック
void inter_token_watchdog_callback(void* ctx, uint32_t token_id, uint64_t timestamp_ns) {
    (void)token_id;  // unused
    auto* context = static_cast<InterTokenWatchdogContext*>(ctx);
    if (!context) return;
    // ウォッチドッグをリセット
    if (context->watchdog) {
        context->watchdog->reset();
    }
    // メトリクスも更新
    if (context->metrics) {
        context->metrics->token_count += 1;
        if (context->metrics->first_token_ns == 0) {
            context->metrics->first_token_ns = timestamp_ns;
        }
        context->metrics->last_token_ns = timestamp_ns;
    }
}

// T182: アボートチェック用コールバック
bool inter_token_abort_check(void* ctx) {
    auto* context = static_cast<InterTokenWatchdogContext*>(ctx);
    if (!context || !context->watchdog) return false;
    return context->watchdog->hasTimedOut();
}

uint64_t steady_now_ns() {
    return static_cast<uint64_t>(
        std::chrono::duration_cast<std::chrono::nanoseconds>(
            std::chrono::steady_clock::now().time_since_epoch()).count());
}

#ifdef XLLM_TESTING
std::mutex g_token_metrics_mutex;
std::function<void(const TokenMetrics&)> g_token_metrics_hook;
std::function<uint64_t()> g_token_metrics_clock;
#endif

uint64_t token_metrics_now_ns() {
#ifdef XLLM_TESTING
    std::lock_guard<std::mutex> lock(g_token_metrics_mutex);
    if (g_token_metrics_clock) {
        return g_token_metrics_clock();
    }
#endif
    return steady_now_ns();
}

// T136-T137: with_retry() and RetryConfig are now defined in inference_engine.h

void token_metrics_callback(void* ctx, uint32_t, uint64_t timestamp_ns) {
    auto* state = static_cast<TokenMetricsState*>(ctx);
    if (!state) return;
    state->token_count += 1;
    if (state->first_token_ns == 0) {
        state->first_token_ns = timestamp_ns;
    }
    state->last_token_ns = timestamp_ns;
}

TokenMetrics build_token_metrics(const TokenMetricsState& state) {
    TokenMetrics metrics;
    metrics.token_count = state.token_count;
    if (state.token_count == 0) {
        return metrics;
    }
    const uint64_t start = state.start_ns;
    const uint64_t first = state.first_token_ns > 0 ? state.first_token_ns : start;
    const uint64_t last = state.last_token_ns > 0 ? state.last_token_ns : first;
    metrics.ttft_ms = static_cast<double>(first - start) / 1'000'000.0;
    const double duration_s = last > start
        ? static_cast<double>(last - start) / 1'000'000'000.0
        : 0.0;
    metrics.tokens_per_second = duration_s > 0.0
        ? static_cast<double>(state.token_count) / duration_s
        : 0.0;
    return metrics;
}

void report_token_metrics(const TokenMetricsState& state, const std::string& model, const char* kind) {
    if (state.token_count == 0) return;
    TokenMetrics metrics = build_token_metrics(state);
    spdlog::info("Token metrics: model={} kind={} ttft_ms={:.2f} tokens={} tokens_per_sec={:.2f}",
        model,
        kind ? kind : "unknown",
        metrics.ttft_ms,
        metrics.token_count,
        metrics.tokens_per_second);
#ifdef XLLM_TESTING
    std::function<void(const TokenMetrics&)> hook;
    {
        std::lock_guard<std::mutex> lock(g_token_metrics_mutex);
        hook = g_token_metrics_hook;
    }
    if (hook) {
        hook(metrics);
    }
#endif
}

std::vector<std::string> split_tokens(const std::string& text, size_t max_tokens) {
    std::vector<std::string> tokens;
    std::string current;
    const size_t effective_max_tokens = max_tokens == 0 ? kDefaultMaxTokens : max_tokens;
    for (char c : text) {
        if (std::isspace(static_cast<unsigned char>(c))) {
            if (!current.empty()) {
                tokens.push_back(current);
                if (tokens.size() >= effective_max_tokens) break;
                current.clear();
            }
        } else {
            current.push_back(c);
        }
    }
    if (!current.empty() && tokens.size() < effective_max_tokens) {
        tokens.push_back(current);
    }
    return tokens;
}

std::string apply_stop_sequences_to_output(std::string output, const std::vector<std::string>& stop_sequences) {
    if (stop_sequences.empty()) return output;
    auto normalized = normalize_stop_sequences(stop_sequences);
    apply_stop_sequences_suffix(output, normalized);
    return output;
}

constexpr auto kDefaultRequestTimeout = std::chrono::seconds(30);
std::atomic<int64_t> g_watchdog_timeout_ms{
    std::chrono::duration_cast<std::chrono::milliseconds>(kDefaultRequestTimeout).count()};
std::mutex g_watchdog_mutex;
std::function<void()> g_watchdog_terminate;

void default_watchdog_terminate() {
    spdlog::critical("Request watchdog timeout exceeded; terminating process");
    std::abort();
}

class RequestWatchdog {
public:
    RequestWatchdog(std::chrono::milliseconds timeout, std::function<void()> on_timeout)
        : timeout_(timeout)
        , on_timeout_(std::move(on_timeout)) {
        if (timeout_.count() <= 0 || !on_timeout_) {
            active_ = false;
            return;
        }
        thread_ = std::thread([this]() { run(); });
    }

    RequestWatchdog(const RequestWatchdog&) = delete;
    RequestWatchdog& operator=(const RequestWatchdog&) = delete;

    void disarm() {
        {
            std::lock_guard<std::mutex> lock(mutex_);
            if (!active_) return;
            active_ = false;
        }
        cv_.notify_all();
    }

    ~RequestWatchdog() {
        disarm();
        if (thread_.joinable()) {
            thread_.join();
        }
    }

private:
    void run() {
        std::unique_lock<std::mutex> lock(mutex_);
        if (!active_) return;
        const bool cancelled = cv_.wait_for(lock, timeout_, [this]() { return !active_; });
        if (cancelled || !active_) return;
        lock.unlock();
        on_timeout_();
    }

    std::chrono::milliseconds timeout_{0};
    std::function<void()> on_timeout_;
    std::mutex mutex_;
    std::condition_variable cv_;
    bool active_{true};
    std::thread thread_;
};

std::chrono::milliseconds get_watchdog_timeout() {
    return std::chrono::milliseconds(g_watchdog_timeout_ms.load());
}

std::function<void()> get_watchdog_terminate_hook() {
    std::lock_guard<std::mutex> lock(g_watchdog_mutex);
    if (!g_watchdog_terminate) {
        g_watchdog_terminate = default_watchdog_terminate;
    }
    return g_watchdog_terminate;
}

template <typename Fn>
auto run_with_watchdog(Fn&& fn) {
    const auto timeout = get_watchdog_timeout();
    if (timeout.count() <= 0) {
        return fn();
    }
    RequestWatchdog watchdog(timeout, get_watchdog_terminate_hook());
    if constexpr (std::is_void_v<decltype(fn())>) {
        fn();
        watchdog.disarm();
    } else {
        auto result = fn();
        watchdog.disarm();
        return result;
    }
}

std::optional<ModelDescriptor> resolve_descriptor(
    const ModelStorage* storage,
    const std::string& model_name) {
    if (!storage) return std::nullopt;

    auto desc = storage->resolveDescriptor(model_name);
    if (desc) return desc;

    return std::nullopt;
}

}  // namespace

// ChatML形式でプロンプトを構築するフォールバック関数（ヘッダーからエクスポート）
std::string buildChatMLPrompt(const std::vector<ChatMessage>& messages) {
    std::ostringstream oss;
    for (const auto& msg : messages) {
        oss << "<|im_start|>" << msg.role << "\n" << msg.content << "<|im_end|>\n";
    }
    // アシスタント応答の開始
    oss << "<|im_start|>assistant\n";
    return oss.str();
}

namespace {

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
    , model_resolver_(model_resolver)
    , resource_usage_provider_(ResourceMonitor::sampleSystemUsage) {
    text_manager_ = std::make_unique<TextManager>(manager, model_storage.modelsDir());
    vision_processor_ = std::make_unique<VisionProcessor>(model_storage);
}

InferenceEngine::InferenceEngine() = default;

InferenceEngine::~InferenceEngine() = default;





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

    auto parsed = ModelStorage::parseModelName(model_name);
    if (!parsed) {
        if (error_message) {
            *error_message = "Invalid model name (invalid quantization format): " + model_name;
        }
        return "";
    }
    const std::string& lookup_name = parsed->base;

    if (model_resolver_ != nullptr) {
        auto resolved = model_resolver_->resolve(lookup_name);
        if (resolved.success) {
            return resolved.path;
        }
        if (error_message) *error_message = resolved.error_message;
        return "";
    }

    std::string gguf_path = model_storage_->resolveGguf(lookup_name);
    if (!gguf_path.empty()) {
        return gguf_path;
    }

    if (error_message) *error_message = "Model not found: " + lookup_name;
    return "";
}






std::vector<std::string> InferenceEngine::getRegisteredRuntimes() const {
    if (!text_manager_) {
        return {};
    }
    return text_manager_->getRegisteredRuntimes();
}

std::string InferenceEngine::generateChat(
    const std::vector<ChatMessage>& messages,
    const std::string& model,
    const InferenceParams& params) const {

    if (!isInitialized()) {
        spdlog::warn("InferenceEngine not initialized, using stub mode");
        if (messages.empty()) return "";
        return apply_stop_sequences_to_output("Response to: " + messages.back().content, params.stop_sequences);
    }

    return run_with_watchdog([&]() {
        auto desc = resolve_descriptor(model_storage_, model);
        if (!desc) {
            throw std::runtime_error("Model not found: " + model);
        }

        std::string resolve_error;
        Engine* engine = text_manager_ ? text_manager_->resolve(*desc, "text", &resolve_error) : nullptr;
        if (!engine) {
            throw std::runtime_error(resolve_error.empty()
                                         ? "No engine registered for runtime: " + desc->runtime
                                         : resolve_error);
        }

        TokenMetricsState metrics;
        metrics.start_ns = token_metrics_now_ns();
        InferenceParams params_with_metrics = params;
        params_with_metrics.on_token_callback = &token_metrics_callback;
        params_with_metrics.on_token_callback_ctx = &metrics;

        RetryConfig retry_config;
        auto output = with_retry([&]() { return engine->generateChat(messages, *desc, params_with_metrics); },
                                 retry_config);
        report_token_metrics(metrics, desc->name, "chat");
        return output;
    });
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
        return apply_stop_sequences_to_output("Response to: " + messages.back().content, params.stop_sequences);
    }

    return run_with_watchdog([&]() {
        std::string error;
        TokenMetricsState metrics;
        metrics.start_ns = token_metrics_now_ns();
        InferenceParams params_with_metrics = params;
        params_with_metrics.on_token_callback = &token_metrics_callback;
        params_with_metrics.on_token_callback_ctx = &metrics;

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
        // T028-T029: OpenAI互換のpresence_penalty/frequency_penaltyを適用
        llama_sampler_chain_add(sampler, llama_sampler_init_penalties(
            64,                        // last_n: 直近64トークンを考慮
            params.repeat_penalty,     // repeat_penalty
            params.frequency_penalty,  // frequency_penalty: OpenAI互換
            params.presence_penalty    // presence_penalty: OpenAI互換
        ));

        uint32_t seed = params.seed;
        if (seed == 0) {
            seed = static_cast<uint32_t>(
                std::chrono::steady_clock::now().time_since_epoch().count() & 0xFFFFFFFF);
        }
        llama_sampler_chain_add(sampler, llama_sampler_init_dist(seed));

        std::string output;
        static const std::vector<std::string> kDefaultStopSequences = {
            "<|im_end|>",
            "<|end|>",
            "<|start|>",
            "<|eot_id|>",
            "</s>",
            "<|endoftext|>",
        };
        auto stop_sequences = merge_stop_sequences(kDefaultStopSequences, params_with_metrics.stop_sequences);

        size_t effective_max_tokens = params_with_metrics.max_tokens;
        int32_t model_n_ctx = llama_model_n_ctx_train(model);
        if (model_n_ctx > 0) {
            size_t available = 0;
            if (prompt_positions < static_cast<size_t>(model_n_ctx)) {
                available = static_cast<size_t>(model_n_ctx) - prompt_positions;
            }
            effective_max_tokens = resolve_effective_max_tokens(
                params_with_metrics.max_tokens,
                prompt_positions,
                model_n_ctx);
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

            if (params_with_metrics.on_token_callback) {
                params_with_metrics.on_token_callback(
                    params_with_metrics.on_token_callback_ctx,
                    static_cast<uint32_t>(new_token),
                    steady_now_ns());
            }

            char buf[256];
            int32_t len = llama_token_to_piece(vocab, new_token, buf, sizeof(buf), 0, false);
            if (len > 0) {
                output.append(buf, static_cast<size_t>(len));
                if (apply_stop_sequences_suffix(output, stop_sequences)) {
                    break;
                }
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

        llama_memory_t end_mem = llama_get_memory(ctx);
        if (end_mem) {
            llama_memory_clear(end_mem, false);
        }

        apply_stop_sequences_suffix(output, stop_sequences);

        if (isGptOssModel(model)) {
            spdlog::info("Vision: Applying gpt-oss output cleanup, before: {} chars", output.size());
            output = cleanGptOssOutput(output);
            spdlog::info("Vision: After cleanup: {} chars", output.size());
        }

        spdlog::info("Vision: Generated {} bytes for model {}", output.size(), model_name);
        report_token_metrics(metrics, model_name, "vision");
        return output;
    });
}

std::string InferenceEngine::generateCompletion(
    const std::string& prompt,
    const std::string& model,
    const InferenceParams& params) const {

    if (!isInitialized()) {
        spdlog::warn("InferenceEngine not initialized, using stub mode");
        if (prompt.empty()) return "";
        return apply_stop_sequences_to_output("Response to: " + prompt, params.stop_sequences);
    }

    return run_with_watchdog([&]() {
        auto desc = resolve_descriptor(model_storage_, model);
        if (!desc) {
            throw std::runtime_error("Model not found: " + model);
        }

        std::string resolve_error;
        Engine* engine = text_manager_ ? text_manager_->resolve(*desc, "text", &resolve_error) : nullptr;
        if (!engine) {
            throw std::runtime_error(resolve_error.empty()
                                         ? "No engine registered for runtime: " + desc->runtime
                                         : resolve_error);
        }

        TokenMetricsState metrics;
        metrics.start_ns = token_metrics_now_ns();
        InferenceParams params_with_metrics = params;
        params_with_metrics.on_token_callback = &token_metrics_callback;
        params_with_metrics.on_token_callback_ctx = &metrics;

        RetryConfig retry_config;
        auto output = with_retry([&]() { return engine->generateCompletion(prompt, *desc, params_with_metrics); },
                                 retry_config);
        report_token_metrics(metrics, desc->name, "completion");
        return output;
    });
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

    return run_with_watchdog([&]() {
        auto desc = resolve_descriptor(model_storage_, model);
        if (!desc) {
            throw std::runtime_error("Model not found: " + model);
        }

        std::string resolve_error;
        Engine* engine = text_manager_ ? text_manager_->resolve(*desc, "text", &resolve_error) : nullptr;
        if (!engine) {
            throw std::runtime_error(resolve_error.empty()
                                         ? "No engine registered for runtime: " + desc->runtime
                                         : resolve_error);
        }

        auto inter_token_timeout = get_inter_token_timeout();
        InterTokenWatchdog inter_token_watchdog(inter_token_timeout);

        TokenMetricsState metrics;
        metrics.start_ns = token_metrics_now_ns();

        InterTokenWatchdogContext watchdog_context;
        watchdog_context.watchdog = &inter_token_watchdog;
        watchdog_context.metrics = &metrics;

        InferenceParams params_with_watchdog = params;
        params_with_watchdog.on_token_callback = &inter_token_watchdog_callback;
        params_with_watchdog.on_token_callback_ctx = &watchdog_context;
        params_with_watchdog.abort_callback = &inter_token_abort_check;
        params_with_watchdog.abort_callback_ctx = &watchdog_context;

        try {
            auto output = engine->generateChatStream(messages, *desc, params_with_watchdog, on_token);
            inter_token_watchdog.disarm();

            if (inter_token_watchdog.hasTimedOut()) {
                throw TokenTimeoutError("Inter-token timeout: no token generated within " +
                    std::to_string(inter_token_timeout.count()) + "ms");
            }

            report_token_metrics(metrics, desc->name, "stream");
            return output;
        } catch (const TokenTimeoutError&) {
            inter_token_watchdog.disarm();
            throw;
        } catch (...) {
            inter_token_watchdog.disarm();
            throw;
        }
    });
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
        result.error_code = EngineErrorCode::kInternal;
        return result;
    }

    if (!ModelStorage::parseModelName(model_name).has_value()) {
        result.error_message = "Invalid model name (invalid quantization format): " + model_name;
        result.error_code = EngineErrorCode::kUnsupported;
        return result;
    }

    auto desc = resolve_descriptor(model_storage_, model_name);
    if (!desc) {
        result.error_message = "Model not found: " + model_name;
        result.error_code = EngineErrorCode::kLoadFailed;
        return result;
    }

    if (!capability.empty() && !desc->capabilities.empty()) {
        if (std::find(desc->capabilities.begin(), desc->capabilities.end(), capability) == desc->capabilities.end()) {
            result.error_message = "Model does not support capability: " + capability;
            result.error_code = EngineErrorCode::kUnsupported;
            return result;
        }
    }

    if (!desc->architectures.empty() && text_manager_ &&
        !text_manager_->supportsArchitecture(desc->runtime, desc->architectures)) {
        result.error_code = EngineErrorCode::kUnsupported;
        result.error_message = "Model architecture is not supported by any engine";
        return result;
    }

    std::string resolve_error;
    Engine* engine = text_manager_ ? text_manager_->resolve(*desc, capability, &resolve_error) : nullptr;
    if (!engine) {
        result.error_message = !resolve_error.empty()
                                   ? resolve_error
                                   : "No engine registered for runtime: " + desc->runtime;
        result.error_code = EngineErrorCode::kUnsupported;
        return result;
    }
    const std::string engine_id = text_manager_ ? text_manager_->engineIdFor(engine) : "";

    if (resource_usage_provider_) {
        const auto usage = resource_usage_provider_();
        const uint64_t required = engine->getModelVramBytes(*desc);
        uint64_t vram_total_bytes = usage.vram_total_bytes;
        uint64_t vram_available_bytes =
            usage.vram_total_bytes > usage.vram_used_bytes
                ? usage.vram_total_bytes - usage.vram_used_bytes
                : 0;

#ifndef XLLM_TESTING
        if (required > 0) {
            GpuDetector detector;
            const auto devices = detector.detect();
            uint64_t max_total = 0;
            uint64_t max_free = 0;
            for (const auto& device : devices) {
                if (!device.is_available) continue;
                max_total = std::max<uint64_t>(max_total, device.memory_bytes);
                max_free = std::max<uint64_t>(max_free, device.free_memory_bytes);
            }
            if (max_total > 0) {
                vram_total_bytes = max_total;
            }
            if (max_free > 0) {
                vram_available_bytes = max_free;
            }
        }
#endif

        if (vram_total_bytes > 0 && required > 0 && !engine_id.empty()) {
            const size_t engine_count = text_manager_ ? text_manager_->engineIdCount() : 0;
            if (engine_count > 0) {
                const uint64_t budget = vram_total_bytes / engine_count;
                if (budget > 0 && required > budget) {
                    spdlog::warn(
                        "VRAM budget exceeded for engine {} (required={} budget={})",
                        engine_id,
                        required,
                        budget);
                    result.error_code = EngineErrorCode::kOomVram;
                    result.error_message = "Insufficient VRAM budget available";
                    return result;
                }
            }
        }
        if (vram_total_bytes > 0 && required > 0) {
            if (required > vram_available_bytes) {
                result.error_code = EngineErrorCode::kOomVram;
                result.error_message = "Insufficient VRAM available";
                return result;
            }
        }
    }

    result = engine->loadModel(*desc);
    if (result.success) {
        result.error_code = EngineErrorCode::kOk;
        model_max_ctx_ = engine->getModelMaxContext(*desc);
    } else if (result.error_code == EngineErrorCode::kLoadFailed && result.error_message.empty()) {
        result.error_message = "Failed to load model: " + model_name;
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

    return run_with_watchdog([&]() {
        auto desc = resolve_descriptor(model_storage_, model_name);
        if (!desc) {
            throw std::runtime_error("Model not found: " + model_name);
        }

        std::string resolve_error;
        Engine* engine = text_manager_ ? text_manager_->resolve(*desc, "embeddings", &resolve_error) : nullptr;
        if (!engine) {
            throw std::runtime_error(resolve_error.empty()
                                         ? "No engine registered for runtime: " + desc->runtime
                                         : resolve_error);
        }

        RetryConfig retry_config;
        return with_retry([&]() { return engine->generateEmbeddings(inputs, *desc); }, retry_config);
    });
}

#ifdef XLLM_TESTING
void InferenceEngine::setEngineRegistryForTest(std::unique_ptr<EngineRegistry> registry) {
    if (!text_manager_) {
        return;
    }
    text_manager_->setEngineRegistryForTest(std::move(registry));
}

void InferenceEngine::setResourceUsageProviderForTest(std::function<ResourceUsage()> provider) {
    resource_usage_provider_ = std::move(provider);
}

void InferenceEngine::setWatchdogTimeoutForTest(std::chrono::milliseconds timeout) {
    g_watchdog_timeout_ms.store(timeout.count());
}

void InferenceEngine::setWatchdogTerminateHookForTest(std::function<void()> hook) {
    std::lock_guard<std::mutex> lock(g_watchdog_mutex);
    g_watchdog_terminate = std::move(hook);
}

void InferenceEngine::setTokenMetricsHookForTest(std::function<void(const TokenMetrics&)> hook) {
    std::lock_guard<std::mutex> lock(g_token_metrics_mutex);
    g_token_metrics_hook = std::move(hook);
}

void InferenceEngine::setTokenMetricsClockForTest(std::function<uint64_t()> clock) {
    std::lock_guard<std::mutex> lock(g_token_metrics_mutex);
    g_token_metrics_clock = std::move(clock);
}



void InferenceEngine::setInterTokenTimeoutForTest(std::chrono::milliseconds timeout) {
    std::lock_guard<std::mutex> lock(g_inter_token_timeout_mutex);
    g_inter_token_timeout_override = timeout;
}
#endif

bool InferenceEngine::isModelSupported(const ModelDescriptor& descriptor) const {
    Engine* engine = text_manager_ ? text_manager_->resolve(descriptor) : nullptr;
    if (!engine) {
        return false;
    }
    if (!engine->supportsTextGeneration()) return false;

    if (descriptor.runtime == "safetensors_cpp") {
#if defined(USE_METAL) || defined(USE_CUDA) || defined(USE_ROCM) || defined(USE_VULKAN)
        return true;
#else
        return false;
#endif
    }

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

// T168: Format tool definitions for prompt embedding
std::string formatToolsForPrompt(const std::vector<ToolDefinition>& tools) {
    if (tools.empty()) {
        return "";
    }

    std::ostringstream oss;
    oss << "\n\nYou have access to the following tools:\n\n";

    for (const auto& tool : tools) {
        oss << "### " << tool.name << "\n";
        if (!tool.description.empty()) {
            oss << tool.description << "\n";
        }
        if (!tool.parameters_json.empty()) {
            oss << "Parameters: " << tool.parameters_json << "\n";
        }
        oss << "\n";
    }

    oss << "To use a tool, respond with a JSON object in this format:\n";
    oss << R"({"name": "tool_name", "arguments": {...}})";
    oss << "\n\nRespond with regular text if no tool is needed.\n";

    return oss.str();
}

// T168: Detect tool calls from model output
std::vector<ToolCall> detectToolCalls(const std::string& output) {
    std::vector<ToolCall> calls;

    // Look for JSON objects that might be tool calls
    // Pattern: {"name": "...", "arguments": {...}}
    size_t pos = 0;
    while (pos < output.size()) {
        // Find start of potential JSON object
        size_t start = output.find('{', pos);
        if (start == std::string::npos) break;

        // Try to find matching closing brace
        int depth = 1;
        size_t end = start + 1;
        bool in_string = false;
        char prev_char = 0;

        while (end < output.size() && depth > 0) {
            char c = output[end];
            if (c == '"' && prev_char != '\\') {
                in_string = !in_string;
            } else if (!in_string) {
                if (c == '{') depth++;
                else if (c == '}') depth--;
            }
            prev_char = c;
            end++;
        }

        if (depth != 0) {
            pos = start + 1;
            continue;
        }

        std::string json_str = output.substr(start, end - start);

        // Try to parse as JSON
        try {
            auto j = nlohmann::json::parse(json_str);

            // Check if it looks like a tool call
            if (j.contains("name") && j["name"].is_string()) {
                ToolCall call;
                call.function_name = j["name"].get<std::string>();

                if (j.contains("arguments")) {
                    if (j["arguments"].is_object()) {
                        call.arguments_json = j["arguments"].dump();
                    } else if (j["arguments"].is_string()) {
                        call.arguments_json = j["arguments"].get<std::string>();
                    }
                }

                // Generate a unique call ID
                static std::atomic<uint64_t> call_counter{0};
                call.id = "call_" + std::to_string(call_counter.fetch_add(1));

                calls.push_back(std::move(call));
                spdlog::debug("Detected tool call: {}", call.function_name);
            }
        } catch (const std::exception&) {
            // Not valid JSON, continue searching
        }

        pos = end;
    }

    return calls;
}

}  // namespace xllm
