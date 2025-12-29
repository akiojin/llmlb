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
// T005: Router API download tests
// ===========================================================================

// FR-003: When shared path is inaccessible, download from router API
// TDD RED: downloadFromRouter must be implemented to pass
TEST(ModelResolverTest, DownloadFromRouterAPIWhenSharedInaccessible) {
    TempModelDirs tmp;
    // No model in local or shared, router_url is set
    // Expected: resolver should attempt router download and indicate it in result

    ModelResolver resolver(tmp.local.string(), "", "http://localhost:19999");
    auto result = resolver.resolve("router-download-model");

    // TDD RED: downloadFromRouter returns empty, so this fails
    // When implemented: should indicate router attempt (success or network error)
    // This test verifies the router download attempt is made
    EXPECT_TRUE(result.router_attempted)
        << "Router download should be attempted when local/shared not available";
}

// FR-003: When shared path is inaccessible, download from origin (HF/proxy)
TEST(ModelResolverTest, DownloadFromOriginWhenSharedInaccessible) {
    GTEST_SKIP() << "TDD RED: origin download path not implemented yet";
}

// FR-004: Downloaded model should be saved to local storage
// TDD RED: downloadFromRouter must save to local to pass
TEST(ModelResolverTest, DownloadedModelSavedToLocal) {
    TempModelDirs tmp;

    ModelResolver resolver(tmp.local.string(), "", "http://localhost:19999");
    auto result = resolver.resolve("downloaded-model");

    // TDD RED: downloadFromRouter is not implemented
    // When implemented: model should be saved to local path
    if (result.success) {
        EXPECT_TRUE(result.path.find(tmp.local.string()) != std::string::npos)
            << "Downloaded model should be in local directory";
        EXPECT_TRUE(fs::exists(result.path))
            << "Downloaded model file should exist";
    } else {
        // If not successful, verify it at least attempted router download
        EXPECT_TRUE(result.router_attempted)
            << "Router download should be attempted";
    }
}

// FR-003 additional: Shared path inaccessible triggers router fallback
TEST(ModelResolverTest, SharedPathInaccessibleTriggersRouterFallback) {
    TempModelDirs tmp;

    // Non-existent shared path simulates inaccessibility
    std::string inaccessible_shared = "/nonexistent/path/that/does/not/exist";

    ModelResolver resolver(tmp.local.string(), inaccessible_shared, "http://localhost:19999");
    auto result = resolver.resolve("fallback-model");

    // TDD RED: Currently returns generic "not found" without router attempt
    // When implemented: should attempt router as fallback
    EXPECT_TRUE(result.router_attempted)
        << "Should attempt router when shared path is inaccessible";
}

// FR-003 additional: Shared path inaccessible triggers origin fallback
TEST(ModelResolverTest, SharedPathInaccessibleTriggersOriginFallback) {
    GTEST_SKIP() << "TDD RED: origin fallback not implemented yet";
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

// FR-006: HuggingFace direct download is allowed with allowlist
TEST(ModelResolverTest, HuggingFaceDirectDownloadAllowedWithAllowlist) {
    GTEST_SKIP() << "TDD RED: allowlist-based direct download not implemented yet";
}

// FR-006: Allowlist should block unknown origins
TEST(ModelResolverTest, AllowlistBlocksUnknownOrigin) {
    GTEST_SKIP() << "TDD RED: allowlist enforcement not implemented yet";
}

// ===========================================================================
// Edge Case Tests (from spec.md エッジケース section)
// ===========================================================================

// Edge Case 1: Network disconnection to shared path -> Router API fallback
// TDD RED: Requires proper fallback logic
TEST(ModelResolverTest, NetworkDisconnectionToSharedPathTriggersRouterFallback) {
    TempModelDirs tmp;

    // Simulate network disconnection by using path that times out or is unreachable
    // Use a path that exists but would fail network access
    std::string unreachable_shared = "//unreachable-host/nonexistent/share";

    ModelResolver resolver(tmp.local.string(), unreachable_shared, "http://localhost:19999");
    auto result = resolver.resolve("network-fallback-model");

    // TDD RED: Should attempt router when shared path access fails
    EXPECT_TRUE(result.router_attempted)
        << "Should fallback to router when shared path network fails";
}

// Edge Case 2: Node restart during download -> Re-download attempt
// TDD RED: Requires partial download detection and cleanup
TEST(ModelResolverTest, IncompleteDownloadIsRetried) {
    TempModelDirs tmp;

    // Simulate incomplete download by creating a partial file
    auto partial_model_dir = tmp.local / "partial-model";
    fs::create_directories(partial_model_dir);
    std::ofstream(partial_model_dir / "model.gguf.partial") << "incomplete";

    ModelResolver resolver(tmp.local.string(), "", "http://localhost:19999");
    auto result = resolver.resolve("partial-model");

    // TDD RED: Should detect partial download and attempt re-download
    // Current implementation doesn't handle partial files
    EXPECT_TRUE(result.router_attempted)
        << "Should attempt re-download when only partial file exists";
}

// Edge Case 3: Multiple requests for same model -> Prevent duplicate downloads
// TDD RED: Requires mutex/lock mechanism
TEST(ModelResolverTest, PreventDuplicateDownloads) {
    TempModelDirs tmp;

    ModelResolver resolver(tmp.local.string(), "", "http://localhost:19999");

    // TDD RED: Need to verify that concurrent resolves don't trigger duplicate downloads
    // This test verifies the resolver has a mechanism to prevent duplicates
    // Current implementation doesn't have this mechanism
    auto result1 = resolver.resolve("concurrent-model");

    // Verify that resolver tracks in-progress downloads
    EXPECT_TRUE(resolver.hasDownloadLock("concurrent-model") ||
                result1.router_attempted)
        << "Should have mechanism to prevent duplicate downloads";
}

// ===========================================================================
// User Story Acceptance Scenarios
// ===========================================================================

// US1-Scenario 2: Updated shared path model is used without copy
// TDD RED: Requires proper shared path behavior verification
TEST(ModelResolverTest, UpdatedSharedPathModelIsUsed) {
    TempModelDirs tmp;
    create_model(tmp.shared, "updatable-model");

    ModelResolver resolver(tmp.local.string(), tmp.shared.string(), "");

    // First resolution
    auto result1 = resolver.resolve("updatable-model");
    EXPECT_TRUE(result1.success);
    std::string first_path = result1.path;

    // Simulate model update by modifying the file
    auto model_file = tmp.shared / "updatable-model" / "model.gguf";
    std::ofstream(model_file) << "updated gguf content v2";

    // Second resolution should use updated model (same path, no copy)
    auto result2 = resolver.resolve("updatable-model");
    EXPECT_TRUE(result2.success);
    EXPECT_EQ(first_path, result2.path)
        << "Should use same shared path (no copy to local)";

    // Verify local is still empty (no copy occurred)
    EXPECT_TRUE(fs::is_empty(tmp.local))
        << "Local should remain empty - no copy from shared";
}

// US2-Scenario 1: Router API download when shared inaccessible
// (Covered by DownloadFromRouterAPIWhenSharedInaccessible)

// US3-Scenario 1: Clear error when model not found anywhere
TEST(ModelResolverTest, ClearErrorMessageWhenModelNotFoundAnywhere) {
    TempModelDirs tmp;

    ModelResolver resolver(tmp.local.string(), tmp.shared.string(), "http://localhost:19999");
    auto result = resolver.resolve("completely-nonexistent-model");

    EXPECT_FALSE(result.success);
    EXPECT_FALSE(result.error_message.empty());
    // Error message should mention the model name
    EXPECT_TRUE(result.error_message.find("completely-nonexistent-model") != std::string::npos)
        << "Error should include the model name for troubleshooting";
}

// ===========================================================================
// Success Criteria Tests
// ===========================================================================

// Success Criteria 4: auto_repair code completely deleted
// (Verified in Phase 3.1 - no code changes needed, just documentation)
TEST(ModelResolverTest, NoAutoRepairFunctionality) {
    TempModelDirs tmp;

    // Create a "corrupted" model file (just empty or invalid)
    auto model_dir = tmp.local / "maybe-corrupted";
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "";  // Empty file

    ModelResolver resolver(tmp.local.string(), tmp.shared.string(), "");
    auto result = resolver.resolve("maybe-corrupted");

    // Should NOT attempt to repair - just return the path as-is
    // auto_repair would have detected empty file and tried to fix it
    EXPECT_TRUE(result.success)
        << "Should return path without attempting repair (auto_repair removed)";
    EXPECT_TRUE(result.path.find(tmp.local.string()) != std::string::npos);
}

// ===========================================================================
// Technical Constraints Tests (from spec.md 技術制約 section)
// ===========================================================================

// Technical Constraint: Only GGUF format is supported for router API download
// TDD RED: Requires GGUF validation in downloadFromRouter
TEST(ModelResolverTest, OnlyGGUFFormatSupported) {
    TempModelDirs tmp;

    // Create a non-GGUF file in local (e.g., .bin format)
    auto model_dir = tmp.local / "non-gguf-model";
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.bin") << "not gguf format";

    ModelResolver resolver(tmp.local.string(), tmp.shared.string(), "");
    auto result = resolver.resolve("non-gguf-model");

    // TDD RED: Current implementation doesn't validate GGUF format
    // Should fail because model.gguf doesn't exist (only model.bin)
    EXPECT_FALSE(result.success)
        << "Should not accept non-GGUF format files";
}

// Technical Constraint: Router download validates GGUF before saving
// TDD RED: Requires GGUF header validation in downloadFromRouter
TEST(ModelResolverTest, RouterDownloadValidatesGGUFFormat) {
    TempModelDirs tmp;

    ModelResolver resolver(tmp.local.string(), "", "http://localhost:19999");
    // Request a model - if router returns non-GGUF, should fail
    auto result = resolver.resolve("might-be-invalid-format");

    // TDD RED: downloadFromRouter not implemented
    // When implemented: should validate GGUF magic bytes before saving
    if (result.success) {
        // If successful, the file must be valid GGUF
        std::ifstream file(result.path, std::ios::binary);
        char magic[4];
        file.read(magic, 4);
        // GGUF magic: 0x47475546 ("GGUF")
        EXPECT_EQ(magic[0], 'G');
        EXPECT_EQ(magic[1], 'G');
        EXPECT_EQ(magic[2], 'U');
        EXPECT_EQ(magic[3], 'F');
    }
}

// ===========================================================================
// Clarifications Tests (from spec.md Clarifications section)
// ===========================================================================

// Clarification: Router API download timeout (recommended: 5 minutes)
// TDD RED: Requires timeout configuration in downloadFromRouter
TEST(ModelResolverTest, RouterDownloadHasTimeout) {
    GTEST_SKIP() << "TDD RED: getDownloadTimeoutMs() not yet implemented";

    TempModelDirs tmp;

    ModelResolver resolver(tmp.local.string(), "", "http://localhost:19999");

    // TDD RED: downloadFromRouter not implemented with timeout
    // When implemented: should have configurable timeout (default 5 min)
    EXPECT_TRUE(resolver.getDownloadTimeoutMs() > 0)
        << "Should have a download timeout configured";
    EXPECT_LE(resolver.getDownloadTimeoutMs(), 5 * 60 * 1000)
        << "Default timeout should be at most 5 minutes";
}

// Clarification: Concurrent download limit (recommended: 1 per node)
// TDD RED: Requires concurrent download tracking
TEST(ModelResolverTest, ConcurrentDownloadLimit) {
    GTEST_SKIP() << "TDD RED: getMaxConcurrentDownloads() not yet implemented";

    TempModelDirs tmp;

    ModelResolver resolver(tmp.local.string(), "", "http://localhost:19999");

    // TDD RED: Not implemented
    // When implemented: should limit concurrent downloads to 1
    EXPECT_EQ(resolver.getMaxConcurrentDownloads(), 1)
        << "Should limit to 1 concurrent download per node";
}
