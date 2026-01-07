#pragma once

#include <condition_variable>
#include <memory>
#include <mutex>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <vector>
#include <thread>
#include <future>

#include "core/llama_manager.h"

namespace llm_node {

class ModelPool {
public:
    explicit ModelPool(std::shared_ptr<LlamaManager> manager);

    // モデルをロードし、コンテキストを取得する（存在しなければロード）
    std::shared_ptr<LlamaContext> acquire(const std::string& model);

    // 現在ロード済みのモデル数
    size_t loadedCount() const;

    // モデルをアンロード（存在すれば）
    bool unload(const std::string& model);

    // メモリ制限を設定（バイト）
    void setMemoryLimit(size_t bytes);
    size_t getMemoryLimit() const;

    // 強制GC（全アンロード）
    void gc();

    // スレッドごとのモデル割り当て（簡易版）
    std::shared_ptr<LlamaContext> acquireForThread(const std::string& model, std::thread::id tid);

    // T141: 並行ロード - VRAM空き確認後に並行ロードを許可
    // 複数のモデルを同時にロード可能（VRAM予約あり）
    std::future<std::shared_ptr<LlamaContext>> acquireAsync(const std::string& model);

    // ロード中のモデル数を取得
    size_t loadingCount() const;

    // VRAM予約量を設定（並行ロード時のVRAM見積もり用）
    void setEstimatedModelSize(size_t bytes);
    size_t getEstimatedModelSize() const;

    // 並行ロードが可能か確認（VRAM空きチェック）
    bool canLoadConcurrently() const;

private:
    std::shared_ptr<LlamaManager> manager_;
    mutable std::mutex mu_;
    std::condition_variable cv_;
    size_t memory_limit_{0};
    size_t estimated_model_size_{0};  // デフォルトモデルサイズ見積もり
    std::unordered_map<std::thread::id, std::shared_ptr<LlamaContext>> thread_cache_;
    std::unordered_set<std::string> loading_in_progress_;  // ロード中のモデル
    size_t reserved_memory_{0};  // ロード中モデルの予約メモリ
};

}  // namespace llm_node
