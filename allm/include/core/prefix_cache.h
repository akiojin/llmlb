#pragma once

#include <chrono>
#include <list>
#include <mutex>
#include <optional>
#include <string>
#include <unordered_map>
#include <vector>

namespace allm {

/// T161: Prefix Cache entry storing KV cache state for a prompt prefix
struct PrefixCacheEntry {
    std::string prefix_hash;           // Hash of the prompt prefix
    std::vector<float> kv_state;       // Serialized KV cache state
    size_t token_count{0};             // Number of tokens in prefix
    size_t vram_bytes{0};              // VRAM usage for this entry
    std::chrono::steady_clock::time_point last_access;
};

/// T161-T162: Prefix Cache for sharing KV cache across requests with same prefix
class PrefixCache {
public:
    /// Create cache with specified VRAM limit in bytes
    explicit PrefixCache(size_t max_vram_bytes = 0);

    /// Create cache with VRAM limit as fraction of available VRAM
    static PrefixCache withVramLimit(double max_vram_fraction);

    /// Get entry by prefix hash, returns nullopt if not found
    std::optional<PrefixCacheEntry> get(const std::string& prefix_hash);

    /// Store entry, evicting LRU entries if VRAM limit exceeded
    void put(const PrefixCacheEntry& entry);

    /// Remove all entries
    void clear();

    /// Get current VRAM usage
    size_t vramUsage() const;

    /// Get maximum VRAM limit
    size_t maxVram() const { return max_vram_bytes_; }

    /// Get number of entries
    size_t size() const;

    /// Generate hash for a prompt prefix
    static std::string hashPrefix(const std::string& prefix);

private:
    size_t max_vram_bytes_{0};
    size_t current_vram_{0};
    mutable std::mutex mutex_;

    // LRU list: front is most recently used
    std::list<PrefixCacheEntry> entries_;
    std::unordered_map<std::string, std::list<PrefixCacheEntry>::iterator> lookup_;

    /// Evict entries until under VRAM limit
    void evictIfNeeded();
};

}  // namespace allm
