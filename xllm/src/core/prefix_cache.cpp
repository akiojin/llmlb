// SPEC-d7feaa2c: T161-T162 Prefix Cache implementation
#include "core/prefix_cache.h"
#include "system/resource_monitor.h"

#include <spdlog/spdlog.h>

namespace xllm {

// FNV-1a hash for prefix
static uint64_t fnv1a_hash(const std::string& data) {
    const uint64_t FNV_PRIME = 0x100000001b3;
    const uint64_t FNV_OFFSET = 0xcbf29ce484222325;

    uint64_t hash = FNV_OFFSET;
    for (char c : data) {
        hash ^= static_cast<uint64_t>(static_cast<unsigned char>(c));
        hash *= FNV_PRIME;
    }
    return hash;
}

PrefixCache::PrefixCache(size_t max_vram_bytes)
    : max_vram_bytes_(max_vram_bytes) {}

PrefixCache PrefixCache::withVramLimit(double max_vram_fraction) {
    auto usage = ResourceMonitor::sampleSystemUsage();

    size_t total_vram = usage.vram_total_bytes;
    if (total_vram == 0) {
        // Fallback to RAM-based estimate if no VRAM detected
        total_vram = usage.mem_total_bytes / 4;  // Assume 25% for cache
    }

    size_t max_bytes = static_cast<size_t>(total_vram * max_vram_fraction);
    spdlog::debug("PrefixCache: max_vram={}B ({}% of {}B)",
                  max_bytes, max_vram_fraction * 100, total_vram);

    return PrefixCache(max_bytes);
}

std::optional<PrefixCacheEntry> PrefixCache::get(const std::string& prefix_hash) {
    std::lock_guard<std::mutex> lock(mutex_);

    auto it = lookup_.find(prefix_hash);
    if (it == lookup_.end()) {
        return std::nullopt;
    }

    // Move to front (most recently used)
    auto entry_it = it->second;
    PrefixCacheEntry entry = *entry_it;
    entry.last_access = std::chrono::steady_clock::now();

    entries_.erase(entry_it);
    entries_.push_front(entry);
    lookup_[prefix_hash] = entries_.begin();

    spdlog::debug("PrefixCache hit: {}", prefix_hash);
    return entry;
}

void PrefixCache::put(const PrefixCacheEntry& entry) {
    std::lock_guard<std::mutex> lock(mutex_);

    // Check if entry already exists
    auto it = lookup_.find(entry.prefix_hash);
    if (it != lookup_.end()) {
        // Update existing: remove old VRAM usage
        current_vram_ -= it->second->vram_bytes;
        entries_.erase(it->second);
        lookup_.erase(it);
    }

    // Add new entry to front
    PrefixCacheEntry new_entry = entry;
    new_entry.last_access = std::chrono::steady_clock::now();

    entries_.push_front(new_entry);
    lookup_[entry.prefix_hash] = entries_.begin();
    current_vram_ += entry.vram_bytes;

    spdlog::debug("PrefixCache put: {} ({}B)", entry.prefix_hash, entry.vram_bytes);

    // Evict if over limit
    evictIfNeeded();
}

void PrefixCache::clear() {
    std::lock_guard<std::mutex> lock(mutex_);
    entries_.clear();
    lookup_.clear();
    current_vram_ = 0;
    spdlog::debug("PrefixCache cleared");
}

size_t PrefixCache::vramUsage() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return current_vram_;
}

size_t PrefixCache::size() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return entries_.size();
}

std::string PrefixCache::hashPrefix(const std::string& prefix) {
    uint64_t hash = fnv1a_hash(prefix);
    char buf[17];
    snprintf(buf, sizeof(buf), "%016llx", static_cast<unsigned long long>(hash));
    return std::string(buf);
}

void PrefixCache::evictIfNeeded() {
    // Must be called with lock held
    if (max_vram_bytes_ == 0) {
        return;  // No limit
    }

    while (current_vram_ > max_vram_bytes_ && !entries_.empty()) {
        // Evict LRU (back of list)
        auto& lru = entries_.back();
        spdlog::debug("PrefixCache evicting LRU: {} ({}B)", lru.prefix_hash, lru.vram_bytes);

        current_vram_ -= lru.vram_bytes;
        lookup_.erase(lru.prefix_hash);
        entries_.pop_back();
    }
}

}  // namespace xllm
