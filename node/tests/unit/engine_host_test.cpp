#include <gtest/gtest.h>

#include <filesystem>
#include <fstream>
#include <chrono>

#include "core/engine_host.h"
#include "core/engine_registry.h"
#include "core/engine_plugin_api.h"

using llm_node::EngineHost;
using llm_node::EngineHostContext;
using llm_node::EngineRegistry;
using llm_node::EnginePluginManifest;
namespace fs = std::filesystem;

TEST(EngineHostTest, RejectsMissingEngineId) {
    EngineHost host;
    EnginePluginManifest manifest;
    manifest.engine_version = "0.1.0";
    manifest.abi_version = EngineHost::kAbiVersion;
    manifest.runtimes = {"llama_cpp"};
    manifest.formats = {"gguf"};
    manifest.library = "llm_engine_llama_cpp";

    std::string error;
    EXPECT_FALSE(host.validateManifest(manifest, error));
    EXPECT_NE(error.find("engine_id"), std::string::npos);
}

TEST(EngineHostTest, RejectsAbiMismatch) {
    EngineHost host;
    EnginePluginManifest manifest;
    manifest.engine_id = "llama_cpp";
    manifest.engine_version = "0.1.0";
    manifest.abi_version = EngineHost::kAbiVersion + 1;
    manifest.runtimes = {"llama_cpp"};
    manifest.formats = {"gguf"};
    manifest.library = "llm_engine_llama_cpp";

    std::string error;
    EXPECT_FALSE(host.validateManifest(manifest, error));
    EXPECT_NE(error.find("abi_version"), std::string::npos);
}

TEST(EngineHostTest, RejectsMissingLibrary) {
    EngineHost host;
    EnginePluginManifest manifest;
    manifest.engine_id = "llama_cpp";
    manifest.engine_version = "0.1.0";
    manifest.abi_version = EngineHost::kAbiVersion;
    manifest.runtimes = {"llama_cpp"};
    manifest.formats = {"gguf"};

    std::string error;
    EXPECT_FALSE(host.validateManifest(manifest, error));
    EXPECT_NE(error.find("library"), std::string::npos);
}

TEST(EngineHostTest, AcceptsCompatibleManifest) {
    EngineHost host;
    EnginePluginManifest manifest;
    manifest.engine_id = "llama_cpp";
    manifest.engine_version = "0.1.0";
    manifest.abi_version = EngineHost::kAbiVersion;
    manifest.runtimes = {"llama_cpp"};
    manifest.formats = {"gguf"};
    manifest.capabilities = {"text"};
    manifest.gpu_targets = {"cuda"};
    manifest.library = "llm_engine_llama_cpp";

    std::string error;
    EXPECT_TRUE(host.validateManifest(manifest, error));
    EXPECT_TRUE(error.empty());
}

TEST(EngineHostTest, LoadsManifestFromFile) {
    EngineHost host;
    fs::path manifest_path = fs::temp_directory_path() / "llm_engine_manifest.json";
    std::ofstream(manifest_path) << R"({
        "engine_id": "llama_cpp",
        "engine_version": "0.1.0",
        "abi_version": 1,
        "runtimes": ["llama_cpp"],
        "formats": ["gguf"],
        "capabilities": ["text"],
        "gpu_targets": ["cuda"],
        "library": "llm_engine_llama_cpp"
    })";

    EnginePluginManifest manifest;
    std::string error;
    EXPECT_TRUE(host.loadManifest(manifest_path, manifest, error));
    EXPECT_TRUE(error.empty());
    EXPECT_EQ(manifest.engine_id, "llama_cpp");
    EXPECT_EQ(manifest.library, "llm_engine_llama_cpp");

    fs::remove(manifest_path);
}

TEST(EngineHostTest, SkipsPluginWithUnsupportedGpuTarget) {
    EngineHost host;
    EngineRegistry registry;
    EngineHostContext context;
    context.abi_version = EngineHost::kAbiVersion;

    const auto temp = fs::temp_directory_path() /
                      ("engine-host-" + std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
    fs::create_directories(temp);
    const auto plugin_dir = temp / "dummy";
    fs::create_directories(plugin_dir);

    const auto manifest_path = plugin_dir / "manifest.json";
    std::ofstream(manifest_path) << R"({
        "engine_id": "dummy_engine",
        "engine_version": "0.1.0",
        "abi_version": 1,
        "runtimes": ["dummy_runtime"],
        "formats": ["gguf"],
        "gpu_targets": ["unknown_gpu"],
        "library": "missing_engine"
    })";

    std::string error;
    EXPECT_TRUE(host.loadPluginsFromDir(temp, registry, context, error));
    EXPECT_TRUE(error.empty());
    EXPECT_EQ(registry.resolve("dummy_runtime"), nullptr);

    std::error_code ec;
    fs::remove_all(temp, ec);
}
