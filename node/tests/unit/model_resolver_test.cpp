// SPEC-48678000: ModelResolver unit tests (TDD RED phase)
// T004-T007: Model auto-resolution tests
#include <gtest/gtest.h>
#include <filesystem>
#include <fstream>

#include "models/model_resolver.h"

using namespace llm_node;
namespace fs = std::filesystem;

class TempModelDirs {
public:
    TempModelDirs() {
        // Create temporary directories for local and shared paths
        local = fs::temp_directory_path() / "model-resolver-local-XXXXXX";
        shared = fs::temp_directory_path() / "model-resolver-shared-XXXXXX";

        std::string local_tmpl = local.string();
        std::vector<char> local_buf(local_tmpl.begin(), local_tmpl.end());
        local_buf.push_back('\0');
        char* local_created = mkdtemp(local_buf.data());
        local = local_created ? fs::path(local_created) : fs::temp_directory_path() / "local";

        std::string shared_tmpl = shared.string();
        std::vector<char> shared_buf(shared_tmpl.begin(), shared_tmpl.end());
        shared_buf.push_back('\0');
        char* shared_created = mkdtemp(shared_buf.data());
        shared = shared_created ? fs::path(shared_created) : fs::temp_directory_path() / "shared";

        fs::create_directories(local);
        fs::create_directories(shared);
    }

    ~TempModelDirs() {
        std::error_code ec;
        fs::remove_all(local, ec);
        fs::remove_all(shared, ec);
    }

    fs::path local;
    fs::path shared;
};

// Helper: create model directory with model.gguf
static void create_model(const fs::path& models_dir, const std::string& dir_name) {
    auto model_dir = models_dir / dir_name;
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "dummy gguf content";
}

// ===========================================================================
// T004: Shared path direct reference tests
// ===========================================================================

// FR-002: When model is not local but exists in shared path, return shared path directly
TEST(ModelResolverTest, ResolveFromSharedPathWhenNotLocal) {
    TempModelDirs tmp;
    create_model(tmp.shared, "llama-3.1-8b");

    ModelResolver resolver(tmp.local.string(), tmp.shared.string(), "");
    auto result = resolver.resolve("llama-3.1-8b");

    EXPECT_TRUE(result.success);
    EXPECT_FALSE(result.path.empty());
    // Path should be in shared directory (not copied to local)
    EXPECT_TRUE(result.path.find(tmp.shared.string()) != std::string::npos);
    EXPECT_TRUE(fs::exists(result.path));
}

// FR-002: Shared path reference should not copy the file
TEST(ModelResolverTest, SharedPathDoesNotCopyToLocal) {
    TempModelDirs tmp;
    create_model(tmp.shared, "qwen-14b");

    ModelResolver resolver(tmp.local.string(), tmp.shared.string(), "");
    auto result = resolver.resolve("qwen-14b");

    EXPECT_TRUE(result.success);
    // Local directory should remain empty (no copy)
    EXPECT_TRUE(fs::is_empty(tmp.local));
}

// FR-001: Local path takes priority over shared path
TEST(ModelResolverTest, LocalPathTakesPriority) {
    TempModelDirs tmp;
    create_model(tmp.local, "gpt-oss-7b");
    create_model(tmp.shared, "gpt-oss-7b");

    ModelResolver resolver(tmp.local.string(), tmp.shared.string(), "");
    auto result = resolver.resolve("gpt-oss-7b");

    EXPECT_TRUE(result.success);
    // Path should be in local directory (priority)
    EXPECT_TRUE(result.path.find(tmp.local.string()) != std::string::npos);
}

// ===========================================================================
// T005: Router API download tests (mock server)
// ===========================================================================

// FR-003: When shared path is inaccessible, download from router API
TEST(ModelResolverTest, DownloadFromRouterAPIWhenSharedInaccessible) {
    TempModelDirs tmp;
    // No model in local or shared, router_url is set
    // Note: This test requires a mock server, mark as disabled for now
    GTEST_SKIP() << "Requires mock HTTP server implementation";
}

// FR-004: Downloaded model should be saved to local storage
TEST(ModelResolverTest, DownloadedModelSavedToLocal) {
    TempModelDirs tmp;
    // Note: This test requires a mock server, mark as disabled for now
    GTEST_SKIP() << "Requires mock HTTP server implementation";
}

// ===========================================================================
// T006: Error handling tests
// ===========================================================================

// FR-005: Return error when model not found anywhere
TEST(ModelResolverTest, ReturnErrorWhenModelNotFound) {
    TempModelDirs tmp;

    ModelResolver resolver(tmp.local.string(), tmp.shared.string(), "");
    auto result = resolver.resolve("nonexistent-model");

    EXPECT_FALSE(result.success);
    EXPECT_TRUE(result.path.empty());
    EXPECT_FALSE(result.error_message.empty());
    EXPECT_TRUE(result.error_message.find("not found") != std::string::npos ||
                result.error_message.find("Not found") != std::string::npos);
}

// Error response should be within 1 second
TEST(ModelResolverTest, ErrorResponseWithinOneSecond) {
    TempModelDirs tmp;

    ModelResolver resolver(tmp.local.string(), tmp.shared.string(), "");

    auto start = std::chrono::steady_clock::now();
    auto result = resolver.resolve("nonexistent-model");
    auto end = std::chrono::steady_clock::now();

    auto duration = std::chrono::duration_cast<std::chrono::milliseconds>(end - start);
    EXPECT_LT(duration.count(), 1000) << "Error response took longer than 1 second";
}

// ===========================================================================
// T007: Integration test - Full resolution flow
// ===========================================================================

// Full fallback flow: local -> shared -> router API -> error
TEST(ModelResolverTest, FullFallbackFlow) {
    TempModelDirs tmp;

    // Test 1: Local exists -> use local
    create_model(tmp.local, "model-a");
    ModelResolver resolver1(tmp.local.string(), tmp.shared.string(), "");
    auto result1 = resolver1.resolve("model-a");
    EXPECT_TRUE(result1.success);
    EXPECT_TRUE(result1.path.find(tmp.local.string()) != std::string::npos);

    // Test 2: Only shared exists -> use shared (no copy)
    create_model(tmp.shared, "model-b");
    ModelResolver resolver2(tmp.local.string(), tmp.shared.string(), "");
    auto result2 = resolver2.resolve("model-b");
    EXPECT_TRUE(result2.success);
    EXPECT_TRUE(result2.path.find(tmp.shared.string()) != std::string::npos);

    // Test 3: Neither exists, no router -> error
    ModelResolver resolver3(tmp.local.string(), tmp.shared.string(), "");
    auto result3 = resolver3.resolve("model-c");
    EXPECT_FALSE(result3.success);
}

// FR-006: HuggingFace direct download is prohibited
TEST(ModelResolverTest, HuggingFaceDirectDownloadProhibited) {
    TempModelDirs tmp;

    // Resolver should never attempt to download from huggingface.co
    ModelResolver resolver(tmp.local.string(), tmp.shared.string(), "");
    auto result = resolver.resolve("meta-llama/Llama-3.1-8B");

    // Should fail (not try HuggingFace)
    EXPECT_FALSE(result.success);
    // Error message should not suggest HuggingFace download
    EXPECT_TRUE(result.error_message.find("huggingface") == std::string::npos);
}
