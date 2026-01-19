#include "core/model_pool.h"

namespace llm_node {

ModelPool::ModelPool(std::shared_ptr<LlamaManager> manager) : manager_(std::move(manager)) {}

std::shared_ptr<LlamaContext> ModelPool::acquire(const std::string& model) {
    std::lock_guard<std::mutex> lock(mu_);
    const size_t before = manager_->memoryUsageBytes();
    const bool over_limit = memory_limit_ > 0 && before >= memory_limit_;
    if (over_limit) return nullptr;

    if (!manager_->loadModel(model)) return nullptr;
    const size_t after = manager_->memoryUsageBytes();
    if (memory_limit_ > 0 && after > memory_limit_) {
        manager_->unloadModel(model);
        return nullptr;
    }
    return manager_->createContext(model);
}

std::shared_ptr<LlamaContext> ModelPool::acquireForThread(const std::string& model, std::thread::id tid) {
    {
        std::lock_guard<std::mutex> lock(mu_);
        auto it = thread_cache_.find(tid);
        if (it != thread_cache_.end() && it->second && it->second->model_path.find(model) != std::string::npos) {
            return it->second;
        }
    }
    auto ctx = acquire(model);  // acquire handles locking
    {
        std::lock_guard<std::mutex> lock(mu_);
        thread_cache_[tid] = ctx;
    }
    return ctx;
}

// T141: 並行ロード - VRAM空き確認後に並行ロードを許可
std::future<std::shared_ptr<LlamaContext>> ModelPool::acquireAsync(const std::string& model) {
    return std::async(std::launch::async, [this, model]() -> std::shared_ptr<LlamaContext> {
        // 予約フェーズ: ロック取得してVRAM予約
        {
            std::unique_lock<std::mutex> lock(mu_);

            // 既にロード中の同一モデルがあれば待機
            cv_.wait(lock, [this, &model]() {
                return loading_in_progress_.find(model) == loading_in_progress_.end();
            });

            // メモリ制限チェック（予約分も含む）
            const size_t current = manager_->memoryUsageBytes() + reserved_memory_;
            const size_t model_size = estimated_model_size_ > 0 ? estimated_model_size_ : 512ull * 1024 * 1024;
            if (memory_limit_ > 0 && current + model_size > memory_limit_) {
                return nullptr;
            }

            // ロード中として登録、メモリ予約
            loading_in_progress_.insert(model);
            reserved_memory_ += model_size;
        }

        // ロードフェーズ: ロックなしで実行（並行可能）
        bool load_success = manager_->loadModel(model);
        std::shared_ptr<LlamaContext> ctx = nullptr;

        // 完了フェーズ: ロック取得して状態更新
        {
            std::lock_guard<std::mutex> lock(mu_);
            const size_t model_size = estimated_model_size_ > 0 ? estimated_model_size_ : 512ull * 1024 * 1024;
            reserved_memory_ -= model_size;
            loading_in_progress_.erase(model);

            if (load_success) {
                // メモリ制限超過チェック
                const size_t after = manager_->memoryUsageBytes();
                if (memory_limit_ > 0 && after > memory_limit_) {
                    manager_->unloadModel(model);
                } else {
                    ctx = manager_->createContext(model);
                }
            }
        }

        // 待機中スレッドに通知
        cv_.notify_all();
        return ctx;
    });
}

size_t ModelPool::loadingCount() const {
    std::lock_guard<std::mutex> lock(mu_);
    return loading_in_progress_.size();
}

void ModelPool::setEstimatedModelSize(size_t bytes) {
    std::lock_guard<std::mutex> lock(mu_);
    estimated_model_size_ = bytes;
}

size_t ModelPool::getEstimatedModelSize() const {
    std::lock_guard<std::mutex> lock(mu_);
    return estimated_model_size_;
}

bool ModelPool::canLoadConcurrently() const {
    std::lock_guard<std::mutex> lock(mu_);
    const size_t current = manager_->memoryUsageBytes() + reserved_memory_;
    const size_t model_size = estimated_model_size_ > 0 ? estimated_model_size_ : 512ull * 1024 * 1024;
    return memory_limit_ == 0 || current + model_size <= memory_limit_;
}

size_t ModelPool::loadedCount() const {
    std::lock_guard<std::mutex> lock(mu_);
    return manager_->loadedCount();
}

bool ModelPool::unload(const std::string& model) {
    std::lock_guard<std::mutex> lock(mu_);
    return manager_->unloadModel(model);
}

void ModelPool::setMemoryLimit(size_t bytes) {
    std::lock_guard<std::mutex> lock(mu_);
    memory_limit_ = bytes;
}

size_t ModelPool::getMemoryLimit() const {
    std::lock_guard<std::mutex> lock(mu_);
    return memory_limit_;
}

void ModelPool::gc() {
    std::lock_guard<std::mutex> lock(mu_);
    auto loaded = manager_->getLoadedModels();
    for (const auto& m : loaded) {
        manager_->unloadModel(m);
    }
    thread_cache_.clear();
}

}  // namespace llm_node
