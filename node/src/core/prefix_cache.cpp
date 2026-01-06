#include "core/prefix_cache.h"

#include <functional>

namespace llm_node {

void PrefixCache::setVramLimit(size_t bytes) {
    std::lock_guard<std::mutex> lock(mutex_);
    vram_limit_ = bytes;
    evictIfNeeded();
}

size_t PrefixCache::getVramLimit() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return vram_limit_;
}

size_t PrefixCache::getCurrentUsage() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return current_usage_;
}

std::optional<PrefixCache::Entry> PrefixCache::get(const std::string& prefix_hash) {
    std::lock_guard<std::mutex> lock(mutex_);
    auto it = entries_.find(prefix_hash);
    if (it == entries_.end()) {
        ++miss_count_;
        return std::nullopt;
    }
    ++hit_count_;
    // LRUリストの先頭に移動
    lru_.splice(lru_.begin(), lru_, it->second);
    return it->second->data;
}

void PrefixCache::put(const std::string& prefix_hash, std::vector<uint8_t> kv_state,
                      size_t token_count, size_t vram_bytes) {
    std::lock_guard<std::mutex> lock(mutex_);

    // 既存エントリがある場合は削除
    auto it = entries_.find(prefix_hash);
    if (it != entries_.end()) {
        current_usage_ -= it->second->data.vram_bytes;
        lru_.erase(it->second);
        entries_.erase(it);
    }

    // 新しいエントリを追加
    InternalEntry entry;
    entry.hash = prefix_hash;
    entry.data.kv_state = std::move(kv_state);
    entry.data.token_count = token_count;
    entry.data.vram_bytes = vram_bytes;

    lru_.push_front(std::move(entry));
    entries_[prefix_hash] = lru_.begin();
    current_usage_ += vram_bytes;

    evictIfNeeded();
}

void PrefixCache::clear() {
    std::lock_guard<std::mutex> lock(mutex_);
    lru_.clear();
    entries_.clear();
    current_usage_ = 0;
}

PrefixCache::Stats PrefixCache::getStats() const {
    std::lock_guard<std::mutex> lock(mutex_);
    Stats stats;
    stats.hit_count = hit_count_;
    stats.miss_count = miss_count_;
    stats.entry_count = entries_.size();
    stats.current_vram_bytes = current_usage_;
    stats.vram_limit_bytes = vram_limit_;
    return stats;
}

size_t PrefixCache::entryCount() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return entries_.size();
}

void PrefixCache::evictIfNeeded() {
    // ロックは呼び出し元で取得済み
    while (vram_limit_ > 0 && current_usage_ > vram_limit_ && !lru_.empty()) {
        // LRUリストの末尾（最も古いエントリ）を削除
        auto& oldest = lru_.back();
        current_usage_ -= oldest.data.vram_bytes;
        entries_.erase(oldest.hash);
        lru_.pop_back();
    }
}

std::string computePrefixHash(const std::string& prefix) {
    // FNV-1a hash for simplicity
    constexpr uint64_t fnv_offset = 14695981039346656037ULL;
    constexpr uint64_t fnv_prime = 1099511628211ULL;

    uint64_t hash = fnv_offset;
    for (char c : prefix) {
        hash ^= static_cast<uint64_t>(static_cast<unsigned char>(c));
        hash *= fnv_prime;
    }

    // 16進数文字列に変換
    char buf[17];
    snprintf(buf, sizeof(buf), "%016llx", static_cast<unsigned long long>(hash));
    return std::string(buf);
}

}  // namespace llm_node
