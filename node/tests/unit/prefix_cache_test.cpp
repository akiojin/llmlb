#include <gtest/gtest.h>
#include <thread>
#include <vector>

#include "core/prefix_cache.h"

using namespace llm_node;

TEST(PrefixCacheTest, DefaultsAreEmpty) {
    PrefixCache cache;
    EXPECT_EQ(cache.entryCount(), 0u);
    EXPECT_EQ(cache.getCurrentUsage(), 0u);
    EXPECT_EQ(cache.getVramLimit(), 0u);
}

TEST(PrefixCacheTest, PutAndGet) {
    PrefixCache cache;
    std::vector<uint8_t> state = {1, 2, 3, 4, 5};

    cache.put("hash1", state, 10, 1024);
    EXPECT_EQ(cache.entryCount(), 1u);

    auto result = cache.get("hash1");
    ASSERT_TRUE(result.has_value());
    EXPECT_EQ(result->kv_state, state);
    EXPECT_EQ(result->token_count, 10u);
    EXPECT_EQ(result->vram_bytes, 1024u);
}

TEST(PrefixCacheTest, GetMissReturnsNullopt) {
    PrefixCache cache;
    auto result = cache.get("nonexistent");
    EXPECT_FALSE(result.has_value());
}

TEST(PrefixCacheTest, PutOverwritesExisting) {
    PrefixCache cache;
    std::vector<uint8_t> state1 = {1, 2, 3};
    std::vector<uint8_t> state2 = {4, 5, 6, 7};

    cache.put("hash1", state1, 5, 512);
    cache.put("hash1", state2, 8, 1024);

    EXPECT_EQ(cache.entryCount(), 1u);
    EXPECT_EQ(cache.getCurrentUsage(), 1024u);

    auto result = cache.get("hash1");
    ASSERT_TRUE(result.has_value());
    EXPECT_EQ(result->kv_state, state2);
    EXPECT_EQ(result->token_count, 8u);
}

TEST(PrefixCacheTest, LruEvictionOnVramLimit) {
    PrefixCache cache;
    cache.setVramLimit(2000);

    cache.put("hash1", {1, 2, 3}, 10, 1000);
    cache.put("hash2", {4, 5, 6}, 20, 1000);
    EXPECT_EQ(cache.entryCount(), 2u);

    // Adding third entry should evict hash1 (LRU)
    cache.put("hash3", {7, 8, 9}, 30, 1000);
    EXPECT_EQ(cache.entryCount(), 2u);
    EXPECT_FALSE(cache.get("hash1").has_value());
    EXPECT_TRUE(cache.get("hash2").has_value());
    EXPECT_TRUE(cache.get("hash3").has_value());
}

TEST(PrefixCacheTest, GetUpdatesLruOrder) {
    PrefixCache cache;
    cache.setVramLimit(2000);

    cache.put("hash1", {1}, 10, 1000);
    cache.put("hash2", {2}, 20, 1000);

    // Access hash1 to make it recently used
    cache.get("hash1");

    // Adding hash3 should evict hash2 (now LRU)
    cache.put("hash3", {3}, 30, 1000);

    EXPECT_TRUE(cache.get("hash1").has_value());
    EXPECT_FALSE(cache.get("hash2").has_value());
    EXPECT_TRUE(cache.get("hash3").has_value());
}

TEST(PrefixCacheTest, Clear) {
    PrefixCache cache;
    cache.put("hash1", {1, 2, 3}, 10, 1024);
    cache.put("hash2", {4, 5, 6}, 20, 2048);

    cache.clear();

    EXPECT_EQ(cache.entryCount(), 0u);
    EXPECT_EQ(cache.getCurrentUsage(), 0u);
    EXPECT_FALSE(cache.get("hash1").has_value());
    EXPECT_FALSE(cache.get("hash2").has_value());
}

TEST(PrefixCacheTest, StatsTracking) {
    PrefixCache cache;
    cache.setVramLimit(10000);

    cache.put("hash1", {1}, 10, 1024);
    cache.get("hash1");  // hit
    cache.get("hash1");  // hit
    cache.get("nonexistent");  // miss

    auto stats = cache.getStats();
    EXPECT_EQ(stats.hit_count, 2u);
    EXPECT_EQ(stats.miss_count, 1u);
    EXPECT_EQ(stats.entry_count, 1u);
    EXPECT_EQ(stats.current_vram_bytes, 1024u);
    EXPECT_EQ(stats.vram_limit_bytes, 10000u);
}

TEST(PrefixCacheTest, SetVramLimitTriggersEviction) {
    PrefixCache cache;

    cache.put("hash1", {1}, 10, 1000);
    cache.put("hash2", {2}, 20, 1000);
    cache.put("hash3", {3}, 30, 1000);
    EXPECT_EQ(cache.entryCount(), 3u);
    EXPECT_EQ(cache.getCurrentUsage(), 3000u);

    // Setting limit should evict oldest entries
    cache.setVramLimit(1500);
    EXPECT_LE(cache.getCurrentUsage(), 1500u);
    EXPECT_EQ(cache.entryCount(), 1u);
    EXPECT_TRUE(cache.get("hash3").has_value());  // Most recent
}

TEST(PrefixCacheTest, ThreadSafety) {
    PrefixCache cache;
    cache.setVramLimit(100000);

    std::vector<std::thread> threads;
    for (int i = 0; i < 8; ++i) {
        threads.emplace_back([&cache, i]() {
            for (int j = 0; j < 100; ++j) {
                std::string hash = "hash_" + std::to_string(i) + "_" + std::to_string(j);
                cache.put(hash, {static_cast<uint8_t>(i), static_cast<uint8_t>(j)}, j, 100);
                cache.get(hash);
            }
        });
    }

    for (auto& t : threads) {
        t.join();
    }

    // No crash, basic sanity check
    EXPECT_LE(cache.getCurrentUsage(), 100000u);
}

// T174: Prefix Cacheヒット/ミステスト

TEST(PrefixCacheTest, HitMissRatioTracking) {
    PrefixCache cache;

    cache.put("prefix1", {1, 2, 3}, 100, 4096);

    // 5 hits
    for (int i = 0; i < 5; ++i) {
        EXPECT_TRUE(cache.get("prefix1").has_value());
    }

    // 3 misses
    for (int i = 0; i < 3; ++i) {
        EXPECT_FALSE(cache.get("nonexistent").has_value());
    }

    auto stats = cache.getStats();
    EXPECT_EQ(stats.hit_count, 5u);
    EXPECT_EQ(stats.miss_count, 3u);

    double hit_ratio = static_cast<double>(stats.hit_count) /
                       static_cast<double>(stats.hit_count + stats.miss_count);
    EXPECT_NEAR(hit_ratio, 0.625, 0.001);
}

TEST(PrefixCacheTest, LruEvictionPreservesRecentlyUsed) {
    PrefixCache cache;
    cache.setVramLimit(3000);

    // Add 3 entries
    cache.put("a", {1}, 10, 1000);
    cache.put("b", {2}, 20, 1000);
    cache.put("c", {3}, 30, 1000);

    // Access 'a' to make it most recently used
    cache.get("a");

    // Add 'd' - should evict 'b' (least recently used)
    cache.put("d", {4}, 40, 1000);

    EXPECT_TRUE(cache.get("a").has_value());
    EXPECT_FALSE(cache.get("b").has_value());
    EXPECT_TRUE(cache.get("c").has_value());
    EXPECT_TRUE(cache.get("d").has_value());
}

TEST(ComputePrefixHashTest, DeterministicHashing) {
    std::string prefix = "You are a helpful assistant.";

    std::string hash1 = computePrefixHash(prefix);
    std::string hash2 = computePrefixHash(prefix);

    EXPECT_EQ(hash1, hash2);
    EXPECT_EQ(hash1.size(), 16u);  // 16 hex chars for 64-bit hash
}

TEST(ComputePrefixHashTest, DifferentPrefixesDifferentHashes) {
    std::string hash1 = computePrefixHash("Hello");
    std::string hash2 = computePrefixHash("World");

    EXPECT_NE(hash1, hash2);
}

TEST(ComputePrefixHashTest, EmptyString) {
    std::string hash = computePrefixHash("");
    EXPECT_EQ(hash.size(), 16u);
}
