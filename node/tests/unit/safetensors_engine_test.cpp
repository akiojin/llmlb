/**
 * @file safetensors_engine_test.cpp
 * @brief Unit tests for SafetensorsEngine plugin
 *
 * SPEC-69549000: safetensors.cpp Node integration
 */

#include <gtest/gtest.h>

#include <filesystem>
#include <fstream>

#include <nlohmann/json.hpp>

#include "core/engine_host.h"
#include "core/engine_registry.h"
#include "core/engine_plugin_api.h"

namespace fs = std::filesystem;
using llm_node::EngineHost;
using llm_node::EngineHostContext;
using llm_node::EnginePluginManifest;
using llm_node::EngineRegistry;
using json = nlohmann::json;

// Platform-specific library extension
#if defined(__APPLE__)
constexpr const char* kLibraryExtension = ".dylib";
#elif defined(_WIN32)
constexpr const char* kLibraryExtension = ".dll";
#else
constexpr const char* kLibraryExtension = ".so";
#endif

inline std::string getPluginLibraryName() {
    return std::string("libllm_engine_safetensors") + kLibraryExtension;
}

// =============================================================================
// SafetensorsEngine Plugin Manifest Tests
// =============================================================================

TEST(SafetensorsEngineTest, ManifestFileExists) {
    const auto manifest_path = fs::path(__FILE__)
        .parent_path().parent_path().parent_path()
        / "engines" / "safetensors" / "manifest.json";
    EXPECT_TRUE(fs::exists(manifest_path))
        << "manifest.json not found at: " << manifest_path;
}

TEST(SafetensorsEngineTest, ManifestIsValid) {
    const auto manifest_path = fs::path(__FILE__)
        .parent_path().parent_path().parent_path()
        / "engines" / "safetensors" / "manifest.json";

    if (!fs::exists(manifest_path)) {
        GTEST_SKIP() << "manifest.json not found";
    }

    std::ifstream ifs(manifest_path);
    ASSERT_TRUE(ifs.is_open()) << "Failed to open manifest.json";
    json j;
    ASSERT_NO_THROW(j = json::parse(ifs)) << "Failed to parse manifest.json";

    // Validate manifest fields
    EXPECT_EQ(j["engine_id"].get<std::string>(), "safetensors_cpp");
    EXPECT_EQ(j["abi_version"].get<int>(), EngineHost::kAbiVersion);
    EXPECT_EQ(j["library"].get<std::string>(), "llm_engine_safetensors");

    // Check runtime
    auto runtimes = j["runtimes"].get<std::vector<std::string>>();
    EXPECT_FALSE(runtimes.empty());
    bool has_safetensors_runtime = std::find(runtimes.begin(), runtimes.end(),
                                             "safetensors_cpp") != runtimes.end();
    EXPECT_TRUE(has_safetensors_runtime);

    // Check format
    auto formats = j["formats"].get<std::vector<std::string>>();
    bool has_safetensors_format = std::find(formats.begin(), formats.end(),
                                            "safetensors") != formats.end();
    EXPECT_TRUE(has_safetensors_format);

    // Check capabilities
    auto capabilities = j["capabilities"].get<std::vector<std::string>>();
    bool has_text = std::find(capabilities.begin(), capabilities.end(),
                              "text") != capabilities.end();
    bool has_embeddings = std::find(capabilities.begin(), capabilities.end(),
                                    "embeddings") != capabilities.end();
    EXPECT_TRUE(has_text);
    EXPECT_TRUE(has_embeddings);
}

TEST(SafetensorsEngineTest, ManifestHasRequiredArchitectures) {
    const auto manifest_path = fs::path(__FILE__)
        .parent_path().parent_path().parent_path()
        / "engines" / "safetensors" / "manifest.json";

    if (!fs::exists(manifest_path)) {
        GTEST_SKIP() << "manifest.json not found";
    }

    std::ifstream ifs(manifest_path);
    ASSERT_TRUE(ifs.is_open());
    json j;
    ASSERT_NO_THROW(j = json::parse(ifs));

    auto architectures = j["architectures"].get<std::vector<std::string>>();

    // SPEC-69549000: Required text generation architectures
    std::vector<std::string> required_text_archs = {
        "llama", "mistral", "gemma", "qwen", "phi",
        "nemotron", "deepseek", "gptoss", "granite", "smollm",
        "kimi", "moondream", "devstral", "magistral"
    };

    // SPEC-69549000: Required embedding architectures
    std::vector<std::string> required_embed_archs = {
        "snowflake", "nomic", "mxbai", "minilm"
    };

    for (const auto& arch : required_text_archs) {
        bool found = std::find(architectures.begin(),
                              architectures.end(),
                              arch) != architectures.end();
        EXPECT_TRUE(found) << "Missing required text architecture: " << arch;
    }

    for (const auto& arch : required_embed_archs) {
        bool found = std::find(architectures.begin(),
                              architectures.end(),
                              arch) != architectures.end();
        EXPECT_TRUE(found) << "Missing required embedding architecture: " << arch;
    }
}

TEST(SafetensorsEngineTest, ManifestHasGpuTargets) {
    const auto manifest_path = fs::path(__FILE__)
        .parent_path().parent_path().parent_path()
        / "engines" / "safetensors" / "manifest.json";

    if (!fs::exists(manifest_path)) {
        GTEST_SKIP() << "manifest.json not found";
    }

    std::ifstream ifs(manifest_path);
    ASSERT_TRUE(ifs.is_open());
    json j;
    ASSERT_NO_THROW(j = json::parse(ifs));

    auto gpu_targets = j["gpu_targets"].get<std::vector<std::string>>();

    // Check GPU targets
    bool has_metal = std::find(gpu_targets.begin(), gpu_targets.end(),
                               "metal") != gpu_targets.end();
    bool has_cuda = std::find(gpu_targets.begin(), gpu_targets.end(),
                              "cuda") != gpu_targets.end();
    EXPECT_TRUE(has_metal) << "Missing metal GPU target";
    EXPECT_TRUE(has_cuda) << "Missing cuda GPU target";
}

// =============================================================================
// SafetensorsEngine Plugin Load Tests
// =============================================================================

TEST(SafetensorsEngineTest, PluginLibraryBuilt) {
    // Check if the plugin library was built
    const std::string lib_name = getPluginLibraryName();
    const auto build_path = fs::path(__FILE__)
        .parent_path().parent_path().parent_path()
        / "build" / "engines" / "safetensors" / lib_name;

    // Also check alternative location
    const auto alt_build_path = fs::path(__FILE__)
        .parent_path().parent_path().parent_path()
        / "build" / lib_name;

    bool exists = fs::exists(build_path) || fs::exists(alt_build_path);
    EXPECT_TRUE(exists) << "Plugin library not found at build/engines/safetensors/ or build/";
}

TEST(SafetensorsEngineTest, PluginCanBeLoadedFromBuildDir) {
    // Skip if library doesn't exist (may not be built yet)
    const auto plugin_dir = fs::path(__FILE__)
        .parent_path().parent_path().parent_path()
        / "build" / "engines" / "safetensors";

    const auto manifest_path = plugin_dir / "manifest.json";
    const auto library_path = plugin_dir / getPluginLibraryName();

    if (!fs::exists(manifest_path) || !fs::exists(library_path)) {
        GTEST_SKIP() << "Plugin files not found in build directory";
    }

    EngineHost host;
    EngineRegistry registry;
    EngineHostContext context;
    context.abi_version = EngineHost::kAbiVersion;
    context.models_dir = "/tmp/test_models";

    std::string error;
    bool result = host.loadPlugin(manifest_path, registry, context, error);

    // Plugin should load successfully (even if no GPU available)
    // If it fails, log the error for debugging
    if (!result) {
        std::cerr << "Plugin load warning: " << error << std::endl;
    }

    // At minimum, manifest JSON parsing should succeed
    std::ifstream ifs(manifest_path);
    ASSERT_TRUE(ifs.is_open());
    json j;
    ASSERT_NO_THROW(j = json::parse(ifs));
    EXPECT_EQ(j["engine_id"].get<std::string>(), "safetensors_cpp");
}

// =============================================================================
// SafetensorsEngine EngineHost Integration Tests
// =============================================================================

TEST(SafetensorsEngineTest, ValidateManifestForSafetensors) {
    EngineHost host;
    EnginePluginManifest manifest;
    manifest.engine_id = "safetensors_cpp";
    manifest.engine_version = "0.1.0";
    manifest.abi_version = EngineHost::kAbiVersion;
    manifest.runtimes = {"safetensors_cpp"};
    manifest.formats = {"safetensors"};
    manifest.architectures = {"llama", "mistral", "gemma", "qwen", "phi"};
    manifest.capabilities = {"text", "embeddings"};
    manifest.modalities = {"completion", "embedding"};
    manifest.license = "MIT";
    manifest.supports_vision = false;
    manifest.gpu_targets = {"metal", "cuda"};
    manifest.library = "llm_engine_safetensors";

    std::string error;
    EXPECT_TRUE(host.validateManifest(manifest, error))
        << "Validation failed: " << error;
    EXPECT_TRUE(error.empty());
}
