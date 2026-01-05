#pragma once

#include <cstddef>
#include <cstdint>
#include <functional>
#include <list>
#include <mutex>
#include <optional>
#include <string>
#include <unordered_map>

namespace llm_node {

/// LRU cache for inference results.
/// Only caches results when temperature=0 (deterministic outputs).
class InferenceCache {
public:
    struct CacheEntry {
        std::string prompt_hash;
        std::string model_name;
        std::string result;
        size_t estimated_bytes{0};
    };

    struct Stats {
        uint64_t hits{0};
        uint64_t misses{0};
        size_t current_bytes{0};
        size_t max_bytes{0};
        size_t entry_count{0};
    };

    /// Create cache with max size in bytes.
    /// If max_bytes is 0, cache is disabled.
    explicit InferenceCache(size_t max_bytes = 0);

    /// Create cache with RAM-based limit.
    /// max_ram_fraction: fraction of available RAM to use (e.g., 0.1 for 10%)
    static InferenceCache withRamLimit(double max_ram_fraction);

    /// Look up cached result for the given prompt.
    /// Returns nullopt if not found or temperature != 0.
    std::optional<std::string> get(const std::string& prompt_hash,
                                   const std::string& model_name);

    /// Store result in cache.
    /// Only stores if temperature == 0.
    void put(const std::string& prompt_hash,
             const std::string& model_name,
             const std::string& result,
             double temperature);

    /// Clear all cached entries.
    void clear();

    /// Get cache statistics.
    Stats stats() const;

    /// Check if caching is enabled.
    bool enabled() const { return max_bytes_ > 0; }

    /// Generate hash for prompt content.
    static std::string hashPrompt(const std::string& prompt);

private:
    void evictIfNeeded(size_t new_entry_bytes);
    static size_t estimateEntryBytes(const CacheEntry& entry);

    mutable std::mutex mutex_;
    size_t max_bytes_{0};
    size_t current_bytes_{0};
    uint64_t hits_{0};
    uint64_t misses_{0};

    // LRU list: front = most recently used, back = least recently used
    std::list<CacheEntry> entries_;
    // Map from "prompt_hash:model_name" to iterator in entries_
    std::unordered_map<std::string, std::list<CacheEntry>::iterator> lookup_;
};

}  // namespace llm_node
