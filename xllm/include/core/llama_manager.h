#pragma once

#include <string>
#include <unordered_map>
#include <unordered_set>
#include <memory>
#include <vector>
#include <mutex>
#include <chrono>
#include <optional>
#include <cstdint>

// llama.cpp forward declarations
struct llama_model;
struct llama_context;

namespace xllm {

/// llama.cpp モデルとコンテキストを保持する構造体
struct LlamaContext {
    std::string model_path;
    llama_model* model{nullptr};
    llama_context* ctx{nullptr};
    size_t gpu_layers{0};
    int gpu_id{-1};

    // デストラクタでリソース解放
    ~LlamaContext();

    // コピー禁止
    LlamaContext() = default;
    LlamaContext(const LlamaContext&) = delete;
    LlamaContext& operator=(const LlamaContext&) = delete;

    // ムーブ許可
    LlamaContext(LlamaContext&& other) noexcept;
    LlamaContext& operator=(LlamaContext&& other) noexcept;
};

class LlamaManager {
public:
    explicit LlamaManager(std::string models_dir);
    ~LlamaManager();

    // llama.cpp バックエンド初期化/終了（main.cpp で1回呼び出し）
    static void initBackend();
    static void freeBackend();

    // モデルロード（llama.cpp API使用）
    bool loadModel(const std::string& model_path);

    // モデルがロード済みか確認
    bool isLoaded(const std::string& model_path) const;

    // コンテキスト取得（推論エンジンが使用）
    llama_context* getContext(const std::string& model_path) const;
    llama_model* getModel(const std::string& model_path) const;

    // コンテキスト生成（モデルがロード済みなら生成）- 旧APIとの互換性
    std::shared_ptr<LlamaContext> createContext(const std::string& model) const;

    size_t loadedCount() const;

    // GPU/CPU レイヤー分割の設定
    void setGpuLayerSplit(size_t layers);
    size_t getGpuLayerSplit() const;

    // メモリ管理（実際のモデルサイズ）
    size_t memoryUsageBytes() const;

    // モデルのアンロード
    bool unloadModel(const std::string& model_path);

    // ロード済みモデルの一覧（フルパス）
    std::vector<std::string> getLoadedModels() const;

    // オンデマンドロード: モデルが未ロードなら自動ロード
    bool loadModelIfNeeded(const std::string& model_path);

    // アイドルタイムアウト設定
    void setIdleTimeout(std::chrono::milliseconds timeout);
    std::chrono::milliseconds getIdleTimeout() const;

    // アイドルモデルのアンロード
    size_t unloadIdleModels();

    // 最大ロード数設定
    void setMaxLoadedModels(size_t max_models);
    size_t getMaxLoadedModels() const;

    // ロード可能かチェック（最大数未満か）
    bool canLoadMore() const;

    // メモリ制限設定
    void setMaxMemoryBytes(size_t max_bytes);
    size_t getMaxMemoryBytes() const;

    // VRAM制限設定（並行ロード用）
    void setMaxVramBytes(size_t max_bytes);
    size_t getMaxVramBytes() const;

    // VRAM必要量の推定（ファイルサイズベース）
    size_t estimateVramRequired(const std::string& model_path) const;

    // 並行ロード可能かチェック（VRAM空き確認）
    bool canLoadConcurrently(const std::string& model_path, size_t required_vram) const;

    // ロード中モデルの追跡
    bool isLoading(const std::string& model_path) const;
    void markAsLoading(const std::string& model_path, size_t estimated_vram);
    void markAsLoaded(const std::string& model_path);

    // T179: ロード失敗時のクリーンアップ
    // - loading状態をクリア
    // - evict_lru=trueならVRAM確保のためLRUモデルをアンロード
    void handleLoadFailure(const std::string& model_path, bool evict_lru = false);

    // T179: VRAM不足からの回復を試みる
    // - LRUモデルを順次アンロードしてVRAMを確保
    // - required_vramに達するまでアンロードを繰り返す
    // - 返り値: 解放されたバイト数
    size_t evictForVram(size_t required_vram);

    // 最終アクセス時刻取得
    std::optional<std::chrono::steady_clock::time_point> getLastAccessTime(
        const std::string& model_path) const;

    // LRU: 最も古くアクセスされたモデルを取得
    std::optional<std::string> getLeastRecentlyUsedModel() const;

    // T202/T206: アクティブ保護（推論中のモデルはLRU evictionから保護）
    // 推論開始時にmarkAsActive、終了時にmarkAsInactiveを呼び出す
    void markAsActive(const std::string& model_path);
    void markAsInactive(const std::string& model_path);
    bool isActive(const std::string& model_path) const;
    size_t activeCount() const;

#ifdef XLLM_TESTING
    // テスト専用: ロード済みモデルを実体なしで注入する
    void addLoadedModelForTest(const std::string& model_path,
                               size_t model_size_bytes,
                               std::optional<std::chrono::steady_clock::time_point> last_access = std::nullopt);
    void setLastAccessForTest(const std::string& model_path,
                              std::chrono::steady_clock::time_point last_access);
#endif

private:
    std::string models_dir_;
    mutable std::mutex mutex_;
    std::unordered_map<std::string, std::unique_ptr<LlamaContext>> loaded_models_;
    size_t gpu_layers_{0};
    size_t memory_bytes_{0};
    std::unordered_map<std::string, int> model_gpu_ids_;

    // オンデマンドロード用の設定
    std::chrono::milliseconds idle_timeout_{std::chrono::minutes(5)};
    size_t max_loaded_models_{0};  // 0 = 無制限
    size_t max_memory_bytes_{0};   // 0 = 無制限
    size_t max_vram_bytes_{0};     // 0 = 無制限

    // 並行ロード用: ロード中モデルの追跡
    std::unordered_map<std::string, size_t> loading_models_;  // path -> estimated_vram

    // アクセス時刻追跡
    std::unordered_map<std::string, std::chrono::steady_clock::time_point> last_access_;

    // T202/T206: アクティブ保護（推論中のモデル）
    std::unordered_set<std::string> active_models_;

#ifdef XLLM_TESTING
    // テスト専用: llama_model_sizeの代わりに使うモデルサイズ
    std::unordered_map<std::string, uint64_t> test_model_sizes_;
#endif

    // 正規化されたパスを取得
    std::string canonicalizePath(const std::string& path) const;

    // アクセス時刻を更新
    void updateAccessTime(const std::string& model_path);
};

}  // namespace xllm
