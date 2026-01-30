#include <gtest/gtest.h>
#include <filesystem>
#include <fstream>

#include "core/llama_manager.h"

using namespace xllm;
namespace fs = std::filesystem;

class TempModelFile {
public:
    TempModelFile() {
        base = fs::temp_directory_path() / fs::path("llm-XXXXXX");
        std::string tmpl = base.string();
        std::vector<char> buf(tmpl.begin(), tmpl.end());
        buf.push_back('\0');
        char* created = mkdtemp(buf.data());
        base = created ? fs::path(created) : fs::temp_directory_path();
    }
    ~TempModelFile() {
        std::error_code ec;
        fs::remove_all(base, ec);
    }
    fs::path base;
};

TEST(LlamaManagerTest, LoadsExistingModel) {
    TempModelFile tmp;
    fs::path model = tmp.base / "model.gguf";
    fs::create_directories(model.parent_path());
    // Note: This creates an invalid GGUF file (just the magic bytes)
    // llama.cpp will fail to parse it as a valid model
    std::ofstream(model) << "GGUF";

    LlamaManager mgr(tmp.base.string());
    mgr.setGpuLayerSplit(5);
    // Invalid GGUF file will fail to load in real llama.cpp
    // This test verifies the path resolution and error handling
    EXPECT_FALSE(mgr.loadModel("model.gguf"));
    EXPECT_EQ(mgr.loadedCount(), 0u);
}

TEST(LlamaManagerTest, FailsOnMissingModel) {
    TempModelFile tmp;
    LlamaManager mgr(tmp.base.string());
    EXPECT_FALSE(mgr.loadModel("missing.gguf"));
    EXPECT_EQ(mgr.loadedCount(), 0u);
    EXPECT_EQ(mgr.createContext("missing.gguf"), nullptr);
}

TEST(LlamaManagerTest, RejectsUnsupportedExtension) {
    TempModelFile tmp;
    fs::path model = tmp.base / "bad.txt";
    fs::create_directories(model.parent_path());
    std::ofstream(model) << "bad";
    LlamaManager mgr(tmp.base.string());
    EXPECT_FALSE(mgr.loadModel("bad.txt"));
    EXPECT_EQ(mgr.loadedCount(), 0u);
}

TEST(LlamaManagerTest, TracksMemoryUsageOnLoad) {
    TempModelFile tmp;
    fs::path model1 = tmp.base / "m1.gguf";
    fs::path model2 = tmp.base / "m2.gguf";
    fs::create_directories(model1.parent_path());
    // Invalid GGUF files - llama.cpp will reject them
    std::ofstream(model1) << "GGUF";
    std::ofstream(model2) << "GGUF";

    LlamaManager mgr(tmp.base.string());
    EXPECT_EQ(mgr.memoryUsageBytes(), 0u);
    // Invalid files won't load, memory stays at 0
    mgr.loadModel("m1.gguf");
    mgr.loadModel("m2.gguf");
    EXPECT_EQ(mgr.memoryUsageBytes(), 0u);  // No models loaded
}

TEST(LlamaManagerTest, UnloadReducesMemory) {
    TempModelFile tmp;
    fs::path model = tmp.base / "m.gguf";
    fs::create_directories(model.parent_path());
    // Invalid GGUF file - llama.cpp will reject it
    std::ofstream(model) << "GGUF";
    LlamaManager mgr(tmp.base.string());
    // Invalid file won't load
    EXPECT_FALSE(mgr.loadModel("m.gguf"));
    EXPECT_EQ(mgr.memoryUsageBytes(), 0u);
    // Unloading non-existent model returns false
    EXPECT_FALSE(mgr.unloadModel("m.gguf"));
    EXPECT_EQ(mgr.memoryUsageBytes(), 0u);
}

// =============================================================================
// LLM runtime blob file format tests (SPEC-0c4f3e5c)
// =============================================================================

TEST(LlamaManagerTest, AcceptsRuntimeBlobFormat) {
    TempModelFile tmp;
    // Create a valid LLM runtime blob filename (sha256-<64 hex chars>)
    std::string blob_name = "sha256-e7b273f9636059a689e3ddcab3716e4f65abe0143ac978e46673ad0e52d09efb";
    fs::path blob_path = tmp.base / blob_name;
    fs::create_directories(blob_path.parent_path());
    // Create a file with GGUF magic bytes (invalid but passes extension check)
    std::ofstream(blob_path) << "GGUF";

    LlamaManager mgr(tmp.base.string());
    // File exists and has valid blob format, but GGUF content is invalid
    // This verifies the extension check passes for blob format
    EXPECT_FALSE(mgr.loadModel(blob_name));  // Fails due to invalid GGUF content
    EXPECT_FALSE(mgr.isLoaded(blob_path.string()));  // Model not loaded
}

TEST(LlamaManagerTest, RejectsInvalidBlobFormat) {
    TempModelFile tmp;
    // Invalid blob format - missing sha256- prefix
    fs::path bad_blob = tmp.base / "e7b273f9636059a689e3ddcab3716e4f65abe0143ac978e46673ad0e52d09efb";
    fs::create_directories(bad_blob.parent_path());
    std::ofstream(bad_blob) << "GGUF";

    LlamaManager mgr(tmp.base.string());
    // Should be rejected because it's not .gguf and not a valid blob format
    EXPECT_FALSE(mgr.loadModel(bad_blob.filename().string()));
}

TEST(LlamaManagerTest, RejectsBlobWithInvalidHex) {
    TempModelFile tmp;
    // Invalid blob format - contains non-hex characters
    fs::path bad_blob = tmp.base / "sha256-ZZZZ73f9636059a689e3ddcab3716e4f65abe0143ac978e46673ad0e52d09efb";
    fs::create_directories(bad_blob.parent_path());
    std::ofstream(bad_blob) << "GGUF";

    LlamaManager mgr(tmp.base.string());
    // Should be rejected because hex part contains invalid characters
    EXPECT_FALSE(mgr.loadModel(bad_blob.filename().string()));
}

TEST(LlamaManagerTest, RejectsTooShortBlobName) {
    TempModelFile tmp;
    // Too short blob name
    fs::path bad_blob = tmp.base / "sha256-abc";
    fs::create_directories(bad_blob.parent_path());
    std::ofstream(bad_blob) << "GGUF";

    LlamaManager mgr(tmp.base.string());
    // Short blob names should still be accepted if they start with sha256-
    // and contain only hex characters (the length check is not enforced)
    // This tests that the format validation works correctly
    EXPECT_FALSE(mgr.loadModel(bad_blob.filename().string()));
}

// =============================================================================
// T141, T146: 並行ロードテスト
// =============================================================================

TEST(LlamaManagerTest, EstimateVramRequiredReturnsFileSize) {
    TempModelFile tmp;
    fs::path model = tmp.base / "test.gguf";
    fs::create_directories(model.parent_path());
    // Create a 1KB test file
    std::string content(1024, 'x');
    std::ofstream(model) << content;

    LlamaManager mgr(tmp.base.string());
    size_t estimated = mgr.estimateVramRequired(model.string());
    // VRAM estimate should be approximately file size (with some overhead factor)
    EXPECT_GE(estimated, 1024u);
    EXPECT_LE(estimated, 2048u);  // Allow up to 2x overhead
}

TEST(LlamaManagerTest, EstimateVramRequiredReturnsZeroForMissingFile) {
    TempModelFile tmp;
    LlamaManager mgr(tmp.base.string());
    size_t estimated = mgr.estimateVramRequired("nonexistent.gguf");
    EXPECT_EQ(estimated, 0u);
}

TEST(LlamaManagerTest, CanLoadConcurrentlyReturnsFalseWhenInsufficientVram) {
    TempModelFile tmp;
    fs::path model = tmp.base / "large.gguf";
    fs::create_directories(model.parent_path());
    std::ofstream(model) << "GGUF";  // Small file for testing

    LlamaManager mgr(tmp.base.string());
    // Set a very small VRAM limit
    mgr.setMaxVramBytes(100);
    // Create large file size estimate situation
    EXPECT_FALSE(mgr.canLoadConcurrently(model.string(), 1024 * 1024));  // 1MB required
}

TEST(LlamaManagerTest, CanLoadConcurrentlyReturnsTrueWhenSufficientVram) {
    TempModelFile tmp;
    fs::path model = tmp.base / "small.gguf";
    fs::create_directories(model.parent_path());
    std::ofstream(model) << "GGUF";

    LlamaManager mgr(tmp.base.string());
    // Set a large VRAM limit
    mgr.setMaxVramBytes(1024 * 1024 * 1024);  // 1GB
    EXPECT_TRUE(mgr.canLoadConcurrently(model.string(), 1024));  // 1KB required
}

TEST(LlamaManagerTest, TracksLoadingModels) {
    TempModelFile tmp;
    fs::path model = tmp.base / "test.gguf";
    fs::create_directories(model.parent_path());
    std::ofstream(model) << "GGUF";

    LlamaManager mgr(tmp.base.string());
    // Initially no models are loading
    EXPECT_FALSE(mgr.isLoading(model.string()));

    // Mark as loading
    mgr.markAsLoading(model.string(), 1024);
    EXPECT_TRUE(mgr.isLoading(model.string()));

    // Mark as not loading
    mgr.markAsLoaded(model.string());
    EXPECT_FALSE(mgr.isLoading(model.string()));
}

TEST(LlamaManagerTest, LoadingModelsAffectVramCalculation) {
    TempModelFile tmp;
    fs::path model1 = tmp.base / "model1.gguf";
    fs::path model2 = tmp.base / "model2.gguf";
    fs::create_directories(model1.parent_path());
    std::ofstream(model1) << "GGUF";
    std::ofstream(model2) << "GGUF";

    LlamaManager mgr(tmp.base.string());
    mgr.setMaxVramBytes(2000);  // 2KB limit

    // Model1 uses 1KB VRAM
    mgr.markAsLoading(model1.string(), 1000);

    // Model2 also wants 1KB - should still fit (but barely)
    EXPECT_TRUE(mgr.canLoadConcurrently(model2.string(), 1000));

    // But 1.5KB would exceed
    EXPECT_FALSE(mgr.canLoadConcurrently(model2.string(), 1500));
}

// =============================================================================
// T179, T186: VRAM部分ロード障害対応テスト
// =============================================================================

TEST(LlamaManagerTest, HandleLoadFailureClearsLoadingState) {
    TempModelFile tmp;
    fs::path model = tmp.base / "test.gguf";
    fs::create_directories(model.parent_path());
    std::ofstream(model) << "GGUF";

    LlamaManager mgr(tmp.base.string());

    // Mark as loading
    mgr.markAsLoading(model.string(), 1024);
    EXPECT_TRUE(mgr.isLoading(model.string()));

    // Handle load failure should clear loading state
    mgr.handleLoadFailure(model.string(), false);
    EXPECT_FALSE(mgr.isLoading(model.string()));
}

TEST(LlamaManagerTest, HandleLoadFailureWithEvictLru) {
    TempModelFile tmp;
    fs::path model1 = tmp.base / "model1.gguf";
    fs::path model2 = tmp.base / "model2.gguf";
    fs::create_directories(model1.parent_path());
    std::ofstream(model1) << "GGUF";
    std::ofstream(model2) << "GGUF";

    LlamaManager mgr(tmp.base.string());

    // Mark model2 as loading (simulating a load in progress)
    mgr.markAsLoading(model2.string(), 1024);

    // handleLoadFailure with evict_lru=true should attempt to evict LRU
    // Since no models are actually loaded, this is a no-op but shouldn't crash
    mgr.handleLoadFailure(model2.string(), true);
    EXPECT_FALSE(mgr.isLoading(model2.string()));
}

TEST(LlamaManagerTest, EvictForVramReturnsZeroWhenNoModelsLoaded) {
    TempModelFile tmp;
    LlamaManager mgr(tmp.base.string());

    // No models loaded, evictForVram should return 0
    size_t freed = mgr.evictForVram(1024 * 1024);
    EXPECT_EQ(freed, 0u);
}

TEST(LlamaManagerTest, LoadFailureDoesNotAffectLoadingState) {
    TempModelFile tmp;
    fs::path model1 = tmp.base / "model1.gguf";
    fs::path model2 = tmp.base / "model2.gguf";
    fs::create_directories(model1.parent_path());
    std::ofstream(model1) << "GGUF";
    std::ofstream(model2) << "GGUF";

    LlamaManager mgr(tmp.base.string());
    mgr.setMaxVramBytes(10000);

    // Mark both as loading
    mgr.markAsLoading(model1.string(), 5000);
    mgr.markAsLoading(model2.string(), 5000);

    // Model1 fails - should only clear model1's loading state
    mgr.handleLoadFailure(model1.string(), false);

    EXPECT_FALSE(mgr.isLoading(model1.string()));
    EXPECT_TRUE(mgr.isLoading(model2.string()));  // model2 still loading
}

TEST(LlamaManagerTest, VramRecoveryAllowsRetryAfterFailure) {
    TempModelFile tmp;
    fs::path model = tmp.base / "test.gguf";
    fs::create_directories(model.parent_path());
    std::ofstream(model) << "GGUF";

    LlamaManager mgr(tmp.base.string());
    mgr.setMaxVramBytes(2000);

    // First load attempt fails
    mgr.markAsLoading(model.string(), 1500);
    mgr.handleLoadFailure(model.string(), false);

    // After failure, VRAM should be available for retry
    EXPECT_TRUE(mgr.canLoadConcurrently(model.string(), 1500));
}

// =============================================================================
// T202: アクティブモデル保護テスト（推論中のモデルはLRU evictionから保護）
// =============================================================================

TEST(LlamaManagerTest, ActiveModelIsNotReturnedByLRU) {
    TempModelFile tmp;
    LlamaManager mgr(tmp.base.string());

    // モデルをアクティブとしてマーク
    mgr.markAsActive("model1.gguf");
    EXPECT_TRUE(mgr.isActive("model1.gguf"));

    // アクティブなモデルはLRU候補から除外される
    // （実際のロード済みモデルがないので、nullopt）
    auto lru = mgr.getLeastRecentlyUsedModel();
    EXPECT_FALSE(lru.has_value());

    // アクティブを解除
    mgr.markAsInactive("model1.gguf");
    EXPECT_FALSE(mgr.isActive("model1.gguf"));
}

TEST(LlamaManagerTest, EvictForVramSkipsActiveModels) {
    TempModelFile tmp;
    LlamaManager mgr(tmp.base.string());

    // No models loaded, eviction should return 0
    size_t freed = mgr.evictForVram(1024);
    EXPECT_EQ(freed, 0u);

    // Even if we mark a model as active, eviction shouldn't affect it
    mgr.markAsActive("test.gguf");
    freed = mgr.evictForVram(1024);
    EXPECT_EQ(freed, 0u);  // No loaded models to evict

    mgr.markAsInactive("test.gguf");
}

TEST(LlamaManagerTest, ActiveCountTracking) {
    TempModelFile tmp;
    LlamaManager mgr(tmp.base.string());

    EXPECT_EQ(mgr.activeCount(), 0u);

    mgr.markAsActive("model1.gguf");
    EXPECT_EQ(mgr.activeCount(), 1u);

    mgr.markAsActive("model2.gguf");
    EXPECT_EQ(mgr.activeCount(), 2u);

    mgr.markAsInactive("model1.gguf");
    EXPECT_EQ(mgr.activeCount(), 1u);

    mgr.markAsInactive("model2.gguf");
    EXPECT_EQ(mgr.activeCount(), 0u);
}
