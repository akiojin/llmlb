/**
 * @file kv_cache.cpp
 * @brief KV cache management implementation (Task 33)
 */

#include "safetensors_internal.h"
#include <cstring>
#include <algorithm>

namespace stcpp {

bool kv_cache_alloc(
    KVCache& cache,
    int32_t n_ctx,
    int32_t n_layers,
    int32_t n_heads,
    int32_t head_dim,
    bool quantized
) {
    if (n_ctx <= 0 || n_layers <= 0 || n_heads <= 0 || head_dim <= 0) {
        return false;
    }

    cache.n_ctx = n_ctx;
    cache.n_used = 0;
    cache.n_layers = n_layers;
    cache.n_heads = n_heads;
    cache.head_dim = head_dim;
    cache.quantized = quantized;

    // Calculate total size needed
    // Each layer has K and V tensors of shape [n_ctx, n_heads, head_dim]
    size_t elements_per_layer = static_cast<size_t>(n_ctx) * n_heads * head_dim;
    size_t total_elements = elements_per_layer * n_layers;

    try {
        // Allocate K and V caches
        cache.k_data.resize(total_elements, 0.0f);
        cache.v_data.resize(total_elements, 0.0f);
    } catch (const std::bad_alloc&) {
        cache.k_data.clear();
        cache.v_data.clear();
        return false;
    }

    return true;
}

void kv_cache_clear(KVCache& cache) {
    cache.n_used = 0;
    // Optionally zero out data for security
    // std::fill(cache.k_data.begin(), cache.k_data.end(), 0.0f);
    // std::fill(cache.v_data.begin(), cache.v_data.end(), 0.0f);
}

void kv_cache_defrag(KVCache& cache) {
    // In a real implementation, this would:
    // 1. Track which positions in the KV cache are in use
    // 2. Compact the cache by moving active entries to the front
    // 3. Update any position mappings
    //
    // For now, this is a no-op since we use simple linear allocation

    // Ensure n_used doesn't exceed n_ctx
    if (cache.n_used > cache.n_ctx) {
        cache.n_used = cache.n_ctx;
    }
}

// Simple hash function for prompt caching
uint64_t compute_prompt_hash(const std::string& prompt) {
    // FNV-1a hash
    uint64_t hash = 14695981039346656037ULL;
    for (char c : prompt) {
        hash ^= static_cast<uint64_t>(static_cast<unsigned char>(c));
        hash *= 1099511628211ULL;
    }
    return hash;
}

}  // namespace stcpp
