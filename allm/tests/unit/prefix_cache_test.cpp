// SPEC-d7feaa2c: T161-T162, T174 Prefix Cache tests
#include <gtest/gtest.h>

#include "core/prefix_cache.h"

namespace allm {
namespace {

// T161: Basic prefix cache operations
TEST(PrefixCacheTest, EmptyCacheReturnsNullopt) {
    PrefixCache cache(1024 * 1024);  // 1MB limit
    auto result = cache.get("nonexistent");
    EXPECT_FALSE(result.has_value());
}

TEST(PrefixCacheTest, StoresAndRetrievesEntry) {
    PrefixCache cache(1024 * 1024);

    PrefixCacheEntry entry;
    entry.prefix_hash = "hash123";
    entry.kv_state = {1.0f, 2.0f, 3.0f};
    entry.token_count = 100;
    entry.vram_bytes = 1024;

    cache.put(entry);

    auto result = cache.get("hash123");
    ASSERT_TRUE(result.has_value());
    EXPECT_EQ(result->prefix_hash, "hash123");
    EXPECT_EQ(result->token_count, 100u);
    EXPECT_EQ(result->kv_state.size(), 3u);
}

TEST(PrefixCacheTest, OverwritesExistingEntry) {
    PrefixCache cache(1024 * 1024);

    PrefixCacheEntry entry1;
    entry1.prefix_hash = "hash123";
    entry1.token_count = 100;
    entry1.vram_bytes = 512;
    cache.put(entry1);

    PrefixCacheEntry entry2;
    entry2.prefix_hash = "hash123";
    entry2.token_count = 200;
    entry2.vram_bytes = 512;
    cache.put(entry2);

    auto result = cache.get("hash123");
    ASSERT_TRUE(result.has_value());
    EXPECT_EQ(result->token_count, 200u);
}

// T162: VRAM allocation management
TEST(PrefixCacheTest, EvictsLRUWhenVramExceeded) {
    PrefixCache cache(1000);  // 1000 bytes limit

    PrefixCacheEntry entry1;
    entry1.prefix_hash = "first";
    entry1.vram_bytes = 400;
    cache.put(entry1);

    PrefixCacheEntry entry2;
    entry2.prefix_hash = "second";
    entry2.vram_bytes = 400;
    cache.put(entry2);

    // Access first to make second LRU
    cache.get("first");

    PrefixCacheEntry entry3;
    entry3.prefix_hash = "third";
    entry3.vram_bytes = 400;
    cache.put(entry3);

    // Second should be evicted (LRU)
    EXPECT_FALSE(cache.get("second").has_value());
    // First and third should still exist
    EXPECT_TRUE(cache.get("first").has_value());
    EXPECT_TRUE(cache.get("third").has_value());
}

TEST(PrefixCacheTest, ReportsVramUsage) {
    PrefixCache cache(1024 * 1024);

    EXPECT_EQ(cache.vramUsage(), 0u);

    PrefixCacheEntry entry1;
    entry1.prefix_hash = "hash1";
    entry1.vram_bytes = 1000;
    cache.put(entry1);
    EXPECT_EQ(cache.vramUsage(), 1000u);

    PrefixCacheEntry entry2;
    entry2.prefix_hash = "hash2";
    entry2.vram_bytes = 500;
    cache.put(entry2);
    EXPECT_EQ(cache.vramUsage(), 1500u);
}

TEST(PrefixCacheTest, ClearRemovesAllEntries) {
    PrefixCache cache(1024 * 1024);

    PrefixCacheEntry entry1;
    entry1.prefix_hash = "hash1";
    entry1.vram_bytes = 1000;
    cache.put(entry1);

    PrefixCacheEntry entry2;
    entry2.prefix_hash = "hash2";
    entry2.vram_bytes = 500;
    cache.put(entry2);

    cache.clear();

    EXPECT_EQ(cache.vramUsage(), 0u);
    EXPECT_FALSE(cache.get("hash1").has_value());
    EXPECT_FALSE(cache.get("hash2").has_value());
}

TEST(PrefixCacheTest, WithVramLimitCreatesWithFraction) {
    // Create cache with 50% of available VRAM
    auto cache = PrefixCache::withVramLimit(0.5);
    // Just verify it's created with reasonable limits
    EXPECT_GT(cache.maxVram(), 0u);
}

TEST(PrefixCacheTest, HashPrefixGeneratesConsistentHash) {
    std::string prefix1 = "Hello, how are you?";
    std::string prefix2 = "Hello, how are you?";
    std::string prefix3 = "Different prefix";

    EXPECT_EQ(PrefixCache::hashPrefix(prefix1), PrefixCache::hashPrefix(prefix2));
    EXPECT_NE(PrefixCache::hashPrefix(prefix1), PrefixCache::hashPrefix(prefix3));
}

TEST(PrefixCacheTest, ReportsEntryCount) {
    PrefixCache cache(1024 * 1024);

    EXPECT_EQ(cache.size(), 0u);

    PrefixCacheEntry entry1;
    entry1.prefix_hash = "hash1";
    entry1.vram_bytes = 100;
    cache.put(entry1);
    EXPECT_EQ(cache.size(), 1u);

    PrefixCacheEntry entry2;
    entry2.prefix_hash = "hash2";
    entry2.vram_bytes = 100;
    cache.put(entry2);
    EXPECT_EQ(cache.size(), 2u);

    cache.clear();
    EXPECT_EQ(cache.size(), 0u);
}

}  // namespace
}  // namespace allm
