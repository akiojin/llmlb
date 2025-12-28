#include <gtest/gtest.h>

#include "core/engine_host.h"

using llm_node::EngineHost;
using llm_node::EnginePluginManifest;

TEST(EngineHostTest, RejectsMissingEngineId) {
    EngineHost host;
    EnginePluginManifest manifest;
    manifest.engine_version = "0.1.0";
    manifest.abi_version = EngineHost::kAbiVersion;
    manifest.runtimes = {"llama_cpp"};
    manifest.formats = {"gguf"};

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

    std::string error;
    EXPECT_FALSE(host.validateManifest(manifest, error));
    EXPECT_NE(error.find("abi_version"), std::string::npos);
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

    std::string error;
    EXPECT_TRUE(host.validateManifest(manifest, error));
    EXPECT_TRUE(error.empty());
}
