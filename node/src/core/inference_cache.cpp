#include "core/inference_cache.h"

#include <cmath>
#include <spdlog/spdlog.h>

#include "system/resource_monitor.h"

namespace llm_node {

InferenceCache::InferenceCache(size_t max_bytes)
    : max_bytes_(max_bytes) {
    if (max_bytes_ > 0) {
        spdlog::info("InferenceCache initialized with {} MB limit",
                     max_bytes_ / (1024 * 1024));
    }
}

InferenceCache InferenceCache::withRamLimit(double max_ram_fraction) {
    if (max_ram_fraction <= 0.0 || max_ram_fraction > 1.0) {
        return InferenceCache(0);
    }

    auto usage = ResourceMonitor::sampleSystemUsage();
    size_t available_ram = usage.mem_total_bytes - usage.mem_used_bytes;
    size_t max_bytes = static_cast<size_t>(available_ram * max_ram_fraction);

    spdlog::info("InferenceCache: {:.1f}% of available RAM = {} MB",
                 max_ram_fraction * 100.0, max_bytes / (1024 * 1024));

    return InferenceCache(max_bytes);
}

std::string InferenceCache::hashPrompt(const std::string& prompt) {
    // Simple FNV-1a hash for speed
    uint64_t hash = 14695981039346656037ULL;
    for (char c : prompt) {
        hash ^= static_cast<uint64_t>(static_cast<unsigned char>(c));
        hash *= 1099511628211ULL;
    }

    // Convert to hex string
    char buf[17];
    snprintf(buf, sizeof(buf), "%016llx", static_cast<unsigned long long>(hash));
    return std::string(buf);
}

std::optional<std::string> InferenceCache::get(const std::string& prompt_hash,
                                                const std::string& model_name) {
    if (!enabled()) {
        return std::nullopt;
    }

    std::lock_guard<std::mutex> lock(mutex_);

    std::string key = prompt_hash + ":" + model_name;
    auto it = lookup_.find(key);
    if (it == lookup_.end()) {
        ++misses_;
        return std::nullopt;
    }

    // Move to front (most recently used)
    entries_.splice(entries_.begin(), entries_, it->second);
    ++hits_;

    spdlog::debug("InferenceCache hit for model={}", model_name);
    return it->second->result;
}

void InferenceCache::put(const std::string& prompt_hash,
                         const std::string& model_name,
                         const std::string& result,
                         double temperature) {
    if (!enabled()) {
        return;
    }

    // Only cache deterministic results (temperature == 0)
    if (std::abs(temperature) > 1e-9) {
        spdlog::debug("InferenceCache: skipping non-deterministic result (temp={})", temperature);
        return;
    }

    std::lock_guard<std::mutex> lock(mutex_);

    std::string key = prompt_hash + ":" + model_name;

    // Check if already exists
    auto existing = lookup_.find(key);
    if (existing != lookup_.end()) {
        // Update and move to front
        current_bytes_ -= estimateEntryBytes(*existing->second);
        existing->second->result = result;
        existing->second->estimated_bytes = estimateEntryBytes(*existing->second);
        current_bytes_ += existing->second->estimated_bytes;
        entries_.splice(entries_.begin(), entries_, existing->second);
        return;
    }

    // Create new entry
    CacheEntry entry;
    entry.prompt_hash = prompt_hash;
    entry.model_name = model_name;
    entry.result = result;
    entry.estimated_bytes = estimateEntryBytes(entry);

    // Evict old entries if needed
    evictIfNeeded(entry.estimated_bytes);

    // Insert at front
    entries_.push_front(std::move(entry));
    lookup_[key] = entries_.begin();
    current_bytes_ += entries_.front().estimated_bytes;

    spdlog::debug("InferenceCache: stored result for model={}, cache_size={} MB",
                  model_name, current_bytes_ / (1024 * 1024));
}

void InferenceCache::clear() {
    std::lock_guard<std::mutex> lock(mutex_);
    entries_.clear();
    lookup_.clear();
    current_bytes_ = 0;
    spdlog::info("InferenceCache cleared");
}

InferenceCache::Stats InferenceCache::stats() const {
    std::lock_guard<std::mutex> lock(mutex_);
    Stats s;
    s.hits = hits_;
    s.misses = misses_;
    s.current_bytes = current_bytes_;
    s.max_bytes = max_bytes_;
    s.entry_count = entries_.size();
    return s;
}

void InferenceCache::evictIfNeeded(size_t new_entry_bytes) {
    // Already holding lock
    while (!entries_.empty() && current_bytes_ + new_entry_bytes > max_bytes_) {
        // Remove least recently used (back of list)
        auto& victim = entries_.back();
        std::string key = victim.prompt_hash + ":" + victim.model_name;
        current_bytes_ -= victim.estimated_bytes;
        lookup_.erase(key);
        entries_.pop_back();
        spdlog::debug("InferenceCache: evicted entry, new size={} MB",
                      current_bytes_ / (1024 * 1024));
    }
}

size_t InferenceCache::estimateEntryBytes(const CacheEntry& entry) {
    // Estimate memory usage of the entry
    return sizeof(CacheEntry) +
           entry.prompt_hash.capacity() +
           entry.model_name.capacity() +
           entry.result.capacity() +
           64;  // overhead for map/list node
}

}  // namespace llm_node
