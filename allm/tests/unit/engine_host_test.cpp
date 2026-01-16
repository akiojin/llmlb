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
    manifest.architectures = {"llama"};
    manifest.modalities = {"completion"};
    manifest.license = "MIT";
    manifest.supports_vision = false;
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
    manifest.architectures = {"llama"};
    manifest.modalities = {"completion"};
    manifest.license = "MIT";
    manifest.supports_vision = false;
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
    manifest.architectures = {"llama"};
    manifest.modalities = {"completion"};
    manifest.license = "MIT";
    manifest.supports_vision = false;

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
    manifest.architectures = {"llama"};
    manifest.capabilities = {"text"};
    manifest.modalities = {"completion"};
    manifest.license = "MIT";
    manifest.supports_vision = false;
    manifest.gpu_targets = {"cuda"};
    manifest.library = "llm_engine_llama_cpp";

    std::string error;
    EXPECT_TRUE(host.validateManifest(manifest, error));
    EXPECT_TRUE(error.empty());
}

TEST(EngineHostTest, RejectsMissingArchitectures) {
    EngineHost host;
    EnginePluginManifest manifest;
    manifest.engine_id = "llama_cpp";
    manifest.engine_version = "0.1.0";
    manifest.abi_version = EngineHost::kAbiVersion;
    manifest.runtimes = {"llama_cpp"};
    manifest.formats = {"gguf"};
    manifest.capabilities = {"text"};
    manifest.modalities = {"completion"};
    manifest.license = "MIT";
    manifest.supports_vision = false;
    manifest.gpu_targets = {"cuda"};
    manifest.library = "llm_engine_llama_cpp";

    std::string error;
    EXPECT_FALSE(host.validateManifest(manifest, error));
    EXPECT_NE(error.find("architectures"), std::string::npos);
}

TEST(EngineHostTest, RejectsMissingModalities) {
    EngineHost host;
    EnginePluginManifest manifest;
    manifest.engine_id = "llama_cpp";
    manifest.engine_version = "0.1.0";
    manifest.abi_version = EngineHost::kAbiVersion;
    manifest.runtimes = {"llama_cpp"};
    manifest.formats = {"gguf"};
    manifest.architectures = {"llama"};
    manifest.license = "MIT";
    manifest.supports_vision = false;
    manifest.library = "llm_engine_llama_cpp";

    std::string error;
    EXPECT_FALSE(host.validateManifest(manifest, error));
    EXPECT_NE(error.find("modalities"), std::string::npos);
}

TEST(EngineHostTest, RejectsMissingLicense) {
    EngineHost host;
    EnginePluginManifest manifest;
    manifest.engine_id = "llama_cpp";
    manifest.engine_version = "0.1.0";
    manifest.abi_version = EngineHost::kAbiVersion;
    manifest.runtimes = {"llama_cpp"};
    manifest.formats = {"gguf"};
    manifest.architectures = {"llama"};
    manifest.modalities = {"completion"};
    manifest.supports_vision = false;
    manifest.library = "llm_engine_llama_cpp";

    std::string error;
    EXPECT_FALSE(host.validateManifest(manifest, error));
    EXPECT_NE(error.find("license"), std::string::npos);
}

TEST(EngineHostTest, LoadsManifestFromFile) {
    EngineHost host;
    fs::path manifest_path = fs::temp_directory_path() / "llm_engine_manifest.json";
    std::ofstream(manifest_path) << R"({
        "engine_id": "llama_cpp",
        "engine_version": "0.1.0",
        "abi_version": 2,
        "runtimes": ["llama_cpp"],
        "formats": ["gguf"],
        "architectures": ["llama"],
        "modalities": ["completion"],
        "license": "MIT",
        "supports_vision": false,
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
    ASSERT_FALSE(manifest.architectures.empty());
    EXPECT_EQ(manifest.architectures.front(), "llama");

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
        "abi_version": 2,
        "runtimes": ["dummy_runtime"],
        "formats": ["gguf"],
        "architectures": ["llama"],
        "modalities": ["completion"],
        "license": "MIT",
        "supports_vision": false,
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

// =============================================================================
// T183, T190: プラグインログ統合テスト
// =============================================================================

using llm_node::PluginLogLevel;
using llm_node::PluginLogCallback;
using llm_node::pluginLogLevelToString;
using llm_node::defaultPluginLogHandler;

TEST(PluginLogTest, EngineHostContextHasLogCallbackField) {
    EngineHostContext context;
    // デフォルトでnullptr
    EXPECT_EQ(context.log_callback, nullptr);
    EXPECT_EQ(context.log_callback_ctx, nullptr);
}

TEST(PluginLogTest, LogLevelToStringReturnsCorrectValues) {
    EXPECT_STREQ(pluginLogLevelToString(PluginLogLevel::kTrace), "trace");
    EXPECT_STREQ(pluginLogLevelToString(PluginLogLevel::kDebug), "debug");
    EXPECT_STREQ(pluginLogLevelToString(PluginLogLevel::kInfo), "info");
    EXPECT_STREQ(pluginLogLevelToString(PluginLogLevel::kWarn), "warn");
    EXPECT_STREQ(pluginLogLevelToString(PluginLogLevel::kError), "error");
}

TEST(PluginLogTest, LogLevelToStringReturnsUnknownForInvalidLevel) {
    auto invalid_level = static_cast<PluginLogLevel>(999);
    EXPECT_STREQ(pluginLogLevelToString(invalid_level), "unknown");
}

namespace {
struct CapturedLog {
    std::string plugin_id;
    int level{-1};
    std::string message;
    int call_count{0};
};

void testLogCallback(void* ctx, const char* plugin_id, int level, const char* message) {
    auto* captured = static_cast<CapturedLog*>(ctx);
    if (captured) {
        captured->plugin_id = plugin_id ? plugin_id : "";
        captured->level = level;
        captured->message = message ? message : "";
        captured->call_count++;
    }
}
}  // namespace

TEST(PluginLogTest, LogCallbackReceivesAllParameters) {
    CapturedLog captured;
    EngineHostContext context;
    context.log_callback = testLogCallback;
    context.log_callback_ctx = &captured;

    // コールバックを呼び出し
    context.log_callback(context.log_callback_ctx, "test_plugin",
                         static_cast<int>(PluginLogLevel::kWarn), "test message");

    EXPECT_EQ(captured.plugin_id, "test_plugin");
    EXPECT_EQ(captured.level, static_cast<int>(PluginLogLevel::kWarn));
    EXPECT_EQ(captured.message, "test message");
    EXPECT_EQ(captured.call_count, 1);
}

TEST(PluginLogTest, DefaultLogHandlerHandlesNullPluginId) {
    // nullのplugin_idでクラッシュしないことを確認
    defaultPluginLogHandler(nullptr, nullptr, static_cast<int>(PluginLogLevel::kInfo), "message");
    // クラッシュしなければ成功
}

TEST(PluginLogTest, DefaultLogHandlerHandlesNullMessage) {
    // nullのmessageでクラッシュしないことを確認
    defaultPluginLogHandler(nullptr, "plugin", static_cast<int>(PluginLogLevel::kInfo), nullptr);
    // クラッシュしなければ成功
}

TEST(PluginLogTest, DefaultLogHandlerHandlesAllLogLevels) {
    // 全レベルでクラッシュしないことを確認
    defaultPluginLogHandler(nullptr, "test", static_cast<int>(PluginLogLevel::kTrace), "trace");
    defaultPluginLogHandler(nullptr, "test", static_cast<int>(PluginLogLevel::kDebug), "debug");
    defaultPluginLogHandler(nullptr, "test", static_cast<int>(PluginLogLevel::kInfo), "info");
    defaultPluginLogHandler(nullptr, "test", static_cast<int>(PluginLogLevel::kWarn), "warn");
    defaultPluginLogHandler(nullptr, "test", static_cast<int>(PluginLogLevel::kError), "error");
    defaultPluginLogHandler(nullptr, "test", 999, "unknown level");
    // クラッシュしなければ成功
}

TEST(PluginLogTest, PluginLogLevelEnumHasCorrectIntValues) {
    // C ABIとの互換性のため、enumの整数値を確認
    EXPECT_EQ(static_cast<int>(PluginLogLevel::kTrace), 0);
    EXPECT_EQ(static_cast<int>(PluginLogLevel::kDebug), 1);
    EXPECT_EQ(static_cast<int>(PluginLogLevel::kInfo), 2);
    EXPECT_EQ(static_cast<int>(PluginLogLevel::kWarn), 3);
    EXPECT_EQ(static_cast<int>(PluginLogLevel::kError), 4);
}
