// SPEC-d7feaa2c: T133-T135, T143 Inference cache tests
#include <gtest/gtest.h>

#include "core/inference_cache.h"

namespace llm_node {
namespace {

// T133: In-memory LRU cache implementation
TEST(InferenceCacheTest, DisabledByDefault) {
    InferenceCache cache(0);
    EXPECT_FALSE(cache.enabled());

    auto result = cache.get("hash", "model");
    EXPECT_FALSE(result.has_value());
}

TEST(InferenceCacheTest, StoresAndRetrievesResult) {
    InferenceCache cache(1024 * 1024);  // 1MB
    EXPECT_TRUE(cache.enabled());

    cache.put("hash1", "model1", "Hello world", 0.0);

    auto result = cache.get("hash1", "model1");
    ASSERT_TRUE(result.has_value());
    EXPECT_EQ(*result, "Hello world");
}

TEST(InferenceCacheTest, OnlyCachesTemperatureZero) {
    InferenceCache cache(1024 * 1024);

    // Temperature > 0 should not be cached
    cache.put("hash1", "model1", "Random output", 0.7);
    EXPECT_FALSE(cache.get("hash1", "model1").has_value());

    // Temperature = 0 should be cached
    cache.put("hash2", "model1", "Deterministic output", 0.0);
    EXPECT_TRUE(cache.get("hash2", "model1").has_value());
}

TEST(InferenceCacheTest, DifferentiatesModels) {
    InferenceCache cache(1024 * 1024);

    cache.put("hash1", "modelA", "Result A", 0.0);
    cache.put("hash1", "modelB", "Result B", 0.0);

    auto resultA = cache.get("hash1", "modelA");
    auto resultB = cache.get("hash1", "modelB");

    ASSERT_TRUE(resultA.has_value());
    ASSERT_TRUE(resultB.has_value());
    EXPECT_EQ(*resultA, "Result A");
    EXPECT_EQ(*resultB, "Result B");
}

// T134: Cache size limit management
TEST(InferenceCacheTest, EvictsLRUWhenFull) {
    // Small cache that can hold about 2-3 entries
    InferenceCache cache(500);

    cache.put("hash1", "m", "First entry", 0.0);
    cache.put("hash2", "m", "Second entry", 0.0);
    cache.put("hash3", "m", "Third entry", 0.0);

    // First entry should be evicted
    EXPECT_FALSE(cache.get("hash1", "m").has_value());

    // Later entries should still exist
    EXPECT_TRUE(cache.get("hash3", "m").has_value());
}

TEST(InferenceCacheTest, LRUOrderUpdatesOnGet) {
    InferenceCache cache(500);

    cache.put("hash1", "m", "First", 0.0);
    cache.put("hash2", "m", "Second", 0.0);

    // Access first entry to make it recently used
    cache.get("hash1", "m");

    // Add third entry, should evict hash2 (now LRU)
    cache.put("hash3", "m", "Third", 0.0);

    EXPECT_TRUE(cache.get("hash1", "m").has_value());
    EXPECT_FALSE(cache.get("hash2", "m").has_value());
    EXPECT_TRUE(cache.get("hash3", "m").has_value());
}

// T135: Cache hit skips inference
TEST(InferenceCacheTest, TracksHitMissStats) {
    InferenceCache cache(1024 * 1024);

    cache.put("hash1", "m", "Result", 0.0);

    cache.get("hash1", "m");  // hit
    cache.get("hash1", "m");  // hit
    cache.get("hash2", "m");  // miss

    auto stats = cache.stats();
    EXPECT_EQ(stats.hits, 2u);
    EXPECT_EQ(stats.misses, 1u);
}

// T143: Cache hit/miss test
TEST(InferenceCacheTest, ClearRemovesAllEntries) {
    InferenceCache cache(1024 * 1024);

    cache.put("hash1", "m", "Result1", 0.0);
    cache.put("hash2", "m", "Result2", 0.0);

    EXPECT_TRUE(cache.get("hash1", "m").has_value());

    cache.clear();

    EXPECT_FALSE(cache.get("hash1", "m").has_value());
    EXPECT_FALSE(cache.get("hash2", "m").has_value());

    auto stats = cache.stats();
    EXPECT_EQ(stats.entry_count, 0u);
    EXPECT_EQ(stats.current_bytes, 0u);
}

TEST(InferenceCacheTest, HashPromptProducesDeterministicHash) {
    std::string prompt = "Hello, how are you?";

    std::string hash1 = InferenceCache::hashPrompt(prompt);
    std::string hash2 = InferenceCache::hashPrompt(prompt);

    EXPECT_EQ(hash1, hash2);
    EXPECT_EQ(hash1.size(), 16u);  // 64-bit hex = 16 chars
}

TEST(InferenceCacheTest, HashPromptDifferentiatesContent) {
    std::string hash1 = InferenceCache::hashPrompt("Hello");
    std::string hash2 = InferenceCache::hashPrompt("World");

    EXPECT_NE(hash1, hash2);
}

TEST(InferenceCacheTest, UpdatesExistingEntry) {
    InferenceCache cache(1024 * 1024);

    cache.put("hash1", "m", "Old result", 0.0);
    cache.put("hash1", "m", "New result", 0.0);

    auto result = cache.get("hash1", "m");
    ASSERT_TRUE(result.has_value());
    EXPECT_EQ(*result, "New result");

    auto stats = cache.stats();
    EXPECT_EQ(stats.entry_count, 1u);
}

}  // namespace
}  // namespace llm_node
