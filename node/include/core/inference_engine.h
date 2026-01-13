#pragma once

#include <chrono>
#include <string>
#include <vector>
#include <functional>
#include <memory>
#include <stdexcept>
#include <filesystem>
#include <mutex>

#include "core/engine_types.h"
#include "core/engine_host.h"
#include "core/engine_registry.h"
#include "system/resource_monitor.h"

namespace llm_node {

// T181: クラッシュ後503エラー
class ServiceUnavailableError : public std::runtime_error {
public:
    explicit ServiceUnavailableError(const std::string& message)
        : std::runtime_error(message) {}
};

// T182: トークン間タイムアウトエラー
class TokenTimeoutError : public std::runtime_error {
public:
    explicit TokenTimeoutError(const std::string& message)
        : std::runtime_error(message) {}
};

// 前方宣言
class LlamaManager;
class ModelStorage;
class ModelSync;
class ModelResolver;
class VisionProcessor;
struct ModelDescriptor;

struct TokenMetrics {
    double ttft_ms{0.0};
    double tokens_per_second{0.0};
    size_t token_count{0};
};

class InferenceEngine {
public:
    /// コンストラクタ: LlamaManager, ModelStorage, ModelSync/ModelResolver への参照を注入
    InferenceEngine(LlamaManager& manager, ModelStorage& model_storage, ModelSync* model_sync = nullptr,
                    ModelResolver* model_resolver = nullptr);

    /// デフォルトコンストラクタ（互換性維持、スタブモード）
    /// VisionProcessor完全型のために.cppで定義
    InferenceEngine();

    /// デストラクタ（VisionProcessor完全型のために.cppで定義）
    ~InferenceEngine();

    /// チャット生成（llama.cpp API使用）
    std::string generateChat(const std::vector<ChatMessage>& messages,
                            const std::string& model,
                            const InferenceParams& params = {}) const;

    /// 画像付きチャット生成（mtmd使用）
    std::string generateChatWithImages(const std::vector<ChatMessage>& messages,
                                       const std::vector<std::string>& image_urls,
                                       const std::string& model,
                                       const InferenceParams& params = {}) const;

    /// テキスト補完
    std::string generateCompletion(const std::string& prompt,
                                   const std::string& model,
                                   const InferenceParams& params = {}) const;

    /// ストリーミングチャット生成
    /// on_token コールバックは各トークン生成時に呼ばれる
    /// 完了時は "[DONE]" を送信
    std::vector<std::string> generateChatStream(
        const std::vector<ChatMessage>& messages,
        const std::string& model,
        const InferenceParams& params,
        const std::function<void(const std::string&)>& on_token) const;

    /// 旧API互換（max_tokens のみ指定）
    std::vector<std::string> generateChatStream(
        const std::vector<ChatMessage>& messages,
        size_t max_tokens,
        const std::function<void(const std::string&)>& on_token) const;

    /// バッチ推論（複数プロンプトを処理）
    std::vector<std::vector<std::string>> generateBatch(
        const std::vector<std::string>& prompts,
        size_t max_tokens) const;

    /// 簡易トークン生成（スペース区切り、互換性維持）
    std::vector<std::string> generateTokens(const std::string& prompt,
                                            size_t max_tokens = 5) const;

    /// サンプリング（互換性維持）
    std::string sampleNextToken(const std::vector<std::string>& tokens) const;

    /// Embedding生成
    /// @param input テキスト入力（単一または複数）
    /// @param model モデル名
    /// @return 各入力に対するembeddingベクトル
    std::vector<std::vector<float>> generateEmbeddings(
        const std::vector<std::string>& inputs,
        const std::string& model) const;

    /// 依存関係が注入されているか確認
    bool isInitialized() const { return manager_ != nullptr && model_storage_ != nullptr; }

    /// モデルをロード（ローカルまたは外部/プロキシ解決）
    /// @return ロード結果（成功/失敗）
    ModelLoadResult loadModel(const std::string& model_name, const std::string& capability = "text");

    /// モデルの最大コンテキストサイズを取得
    size_t getModelMaxContext() const { return model_max_ctx_; }

    /// モデルが利用可能かを判定（エンジン/メタデータに基づく）
    bool isModelSupported(const ModelDescriptor& descriptor) const;

    /// エンジンプラグインをロードする
    bool loadEnginePlugins(const std::filesystem::path& directory, std::string& error);
    /// エンジンプラグインをシャドウロードして差し替える
    bool reloadEnginePlugins(const std::filesystem::path& directory, std::string& error);
    /// リクエストがアイドルなら保留中のプラグイン差し替えを適用
    void applyPendingEnginePluginsIfIdle(std::string* error = nullptr) const;
    /// プラグイン再起動ポリシーを設定
    void setPluginRestartPolicy(std::chrono::seconds interval, uint64_t request_limit);

    /// T181: プラグインがリカバリモード中かどうかを返す
    bool isInRecoveryMode() const;
    /// T181: リカバリモードを解除
    void clearRecoveryMode();

    /// 登録済みのランタイム一覧を取得（プラグインからロードしたものを含む）
    std::vector<std::string> getRegisteredRuntimes() const;

#ifdef LLM_NODE_TESTING
    /// テスト専用: EngineRegistry を差し替える
    void setEngineRegistryForTest(std::unique_ptr<EngineRegistry> registry);
    /// テスト専用: リソース使用量のプロバイダを差し替える
    void setResourceUsageProviderForTest(std::function<ResourceUsage()> provider);
    /// テスト専用: ウォッチドッグのタイムアウトを差し替える
    static void setWatchdogTimeoutForTest(std::chrono::milliseconds timeout);
    /// テスト専用: タイムアウト時の終了処理を差し替える
    static void setWatchdogTerminateHookForTest(std::function<void()> hook);
    /// テスト専用: トークンメトリクスのフックを差し替える
    static void setTokenMetricsHookForTest(std::function<void(const TokenMetrics&)> hook);
    /// テスト専用: トークンメトリクス用の時刻取得を差し替える
    static void setTokenMetricsClockForTest(std::function<uint64_t()> clock);
    /// テスト専用: プラグイン再起動処理のフックを差し替える
    static void setPluginRestartHookForTest(std::function<bool(std::string&)> hook);
    /// テスト専用: プラグインディレクトリを指定する
    void setEnginePluginsDirForTest(const std::filesystem::path& directory);
    /// T182: テスト専用: トークン間タイムアウトを差し替える
    static void setInterTokenTimeoutForTest(std::chrono::milliseconds timeout);
#endif

private:
    LlamaManager* manager_{nullptr};
    ModelStorage* model_storage_{nullptr};
    ModelSync* model_sync_{nullptr};
    ModelResolver* model_resolver_{nullptr};
    mutable EngineHost engine_host_;
    mutable std::unique_ptr<EngineRegistry> engines_;
    size_t model_max_ctx_{4096};  // モデルの最大コンテキストサイズ
    mutable std::unique_ptr<VisionProcessor> vision_processor_{nullptr};
    std::function<ResourceUsage()> resource_usage_provider_{};
    std::filesystem::path engine_plugins_dir_;
    mutable std::chrono::steady_clock::time_point plugin_restart_last_{};
    mutable uint64_t plugin_restart_request_count_{0};
    mutable std::chrono::seconds plugin_restart_interval_{0};
    mutable uint64_t plugin_restart_request_limit_{0};
    mutable bool plugin_restart_pending_{false};
    mutable std::mutex plugin_restart_mutex_;

    /// チャットメッセージからプロンプト文字列を構築
    std::string buildChatPrompt(const std::vector<ChatMessage>& messages) const;

    /// モデルパス解決（ModelResolver優先）
    std::string resolveModelPath(const std::string& model_name, std::string* error_message = nullptr) const;
    void maybeSchedulePluginRestart() const;
    void handlePluginCrash() const;
    bool stagePluginRestart(const char* reason, std::string& error) const;
};

/// ChatML形式でプロンプトを構築（テンプレートなしモデル用フォールバック）
std::string buildChatMLPrompt(const std::vector<ChatMessage>& messages);

/// ツール定義をプロンプトに埋め込む形式で変換
std::string formatToolsForPrompt(const std::vector<ToolDefinition>& tools);

/// 出力からツール呼び出しJSONを検出
/// Returns: 検出されたToolCall（複数可）、未検出時は空配列
std::vector<ToolCall> detectToolCalls(const std::string& output);

/// T136: リトライ設定
struct RetryConfig {
    int max_retries{4};
    std::chrono::milliseconds initial_delay{std::chrono::milliseconds(100)};
    std::chrono::milliseconds max_total{std::chrono::milliseconds(30000)};
};

/// T136: 指数バックオフリトライヘルパー
/// T137: クラッシュ後の透過的リトライ
template <typename Fn>
auto with_retry(Fn&& fn, const RetryConfig& config,
                std::function<void()> on_crash = nullptr) -> decltype(fn()) {
    std::exception_ptr last_exception;
    auto total_start = std::chrono::steady_clock::now();
    auto delay = config.initial_delay;

    for (int attempt = 0; attempt <= config.max_retries; ++attempt) {
        try {
            return fn();
        } catch (...) {
            last_exception = std::current_exception();

            // Check total time limit
            auto elapsed = std::chrono::duration_cast<std::chrono::milliseconds>(
                std::chrono::steady_clock::now() - total_start);
            if (elapsed >= config.max_total) {
                break;
            }

            // Don't retry on last attempt
            if (attempt >= config.max_retries) {
                break;
            }

            // Notify crash handler for transparent recovery
            if (on_crash) {
                on_crash();
            }

            std::this_thread::sleep_for(delay);

            // Exponential backoff: 100ms -> 200ms -> 400ms -> 800ms
            delay = std::min(delay * 2, config.max_total - elapsed);
        }
    }

    if (last_exception) {
        std::rethrow_exception(last_exception);
    }
    // Should not reach here, but satisfy compiler
    throw std::runtime_error("Retry failed with no exception captured");
}

}  // namespace llm_node
