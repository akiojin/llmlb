#include <gtest/gtest.h>
#include <cstdlib>
#include <fstream>
#include <filesystem>
#include <unordered_map>

#include "utils/config.h"

using namespace xllm;
namespace fs = std::filesystem;

class EnvGuard {
public:
    EnvGuard(const std::vector<std::string>& keys) : keys_(keys) {
        for (const auto& k : keys_) {
            const char* v = std::getenv(k.c_str());
            if (v) saved_[k] = v;
        }
    }
    ~EnvGuard() {
        for (const auto& k : keys_) {
            if (auto it = saved_.find(k); it != saved_.end()) {
                setenv(k.c_str(), it->second.c_str(), 1);
            } else {
                unsetenv(k.c_str());
            }
        }
    }
private:
    std::vector<std::string> keys_;
    std::unordered_map<std::string, std::string> saved_;
};

TEST(UtilsConfigTest, LoadsNodeConfigFromFileWithLock) {
    EnvGuard guard({"XLLM_CONFIG", "LLM_MODELS_DIR",
                    "XLLM_PORT"});

    fs::path tmp = fs::temp_directory_path() / "nodecfg.json";
    std::ofstream(tmp) << R"({
        "models_dir": "/tmp/models",
        "node_port": 18080,
        "require_gpu": false
    })";
    setenv("XLLM_CONFIG", tmp.string().c_str(), 1);

    auto info = loadNodeConfigWithLog();
    auto cfg = info.first;

    EXPECT_EQ(cfg.models_dir, "/tmp/models");
    EXPECT_EQ(cfg.node_port, 18080);
    EXPECT_TRUE(cfg.require_gpu);  // require_gpu is forced to true
    EXPECT_NE(info.second.find("file="), std::string::npos);

    fs::remove(tmp);
}

TEST(UtilsConfigTest, EnvOverridesNodeConfig) {
    EnvGuard guard({"LLM_MODELS_DIR", "XLLM_PORT", "XLLM_CONFIG",
                    "XLLM_MODELS_DIR"});

    unsetenv("XLLM_CONFIG");
    // Test with deprecated env var names (fallback)
    setenv("LLM_MODELS_DIR", "/env/models", 1);
    setenv("XLLM_PORT", "19000", 1);
    auto cfg = loadNodeConfig();
    EXPECT_EQ(cfg.models_dir, "/env/models");
    EXPECT_EQ(cfg.node_port, 19000);
    EXPECT_TRUE(cfg.require_gpu);  // env flags are ignored, GPU required
}

TEST(UtilsConfigTest, NewEnvVarsTakePriorityOverDeprecated) {
    EnvGuard guard({"XLLM_MODELS_DIR", "LLM_MODELS_DIR",
                    "XLLM_PORT", "XLLM_CONFIG"});

    unsetenv("XLLM_CONFIG");

    // Set both new and deprecated env vars
    setenv("XLLM_MODELS_DIR", "/new/models", 1);
    setenv("LLM_MODELS_DIR", "/old/models", 1);  // Should be ignored
    auto cfg = loadNodeConfig();
    EXPECT_EQ(cfg.models_dir, "/new/models");
    EXPECT_TRUE(cfg.require_gpu);  // GPU requirement cannot be disabled
}

TEST(UtilsConfigTest, DefaultConfigPathIsXllmDir) {
    // Verify the default config path is ~/.xllm/config.json (not ~/.llmlb/)
    EnvGuard guard({"XLLM_CONFIG", "HOME"});
    unsetenv("XLLM_CONFIG");

    // Create a temporary home directory with .xllm/config.json
    fs::path tmp_home = fs::temp_directory_path() / "test_xllm_home";
    fs::create_directories(tmp_home / ".xllm");
    std::ofstream(tmp_home / ".xllm" / "config.json") << R"({
        "node_port": 12345
    })";
    setenv("HOME", tmp_home.string().c_str(), 1);

    auto [cfg, log] = loadNodeConfigWithLog();

    // Should load from ~/.xllm/config.json
    EXPECT_EQ(cfg.node_port, 12345);
    EXPECT_NE(log.find(".xllm/config.json"), std::string::npos);

    fs::remove_all(tmp_home);
}

TEST(UtilsConfigTest, DefaultModelsDirIsXllmModels) {
    // Verify the default models_dir is ~/.xllm/models (not ~/.llmlb/)
    EnvGuard guard({"XLLM_CONFIG", "HOME", "XLLM_MODELS_DIR", "LLM_MODELS_DIR"});
    unsetenv("XLLM_CONFIG");
    unsetenv("XLLM_MODELS_DIR");
    unsetenv("LLM_MODELS_DIR");

    fs::path tmp_home = fs::temp_directory_path() / "test_xllm_models";
    fs::create_directories(tmp_home);
    setenv("HOME", tmp_home.string().c_str(), 1);

    auto cfg = loadNodeConfig();

    // Default models_dir should be under .xllm
    EXPECT_NE(cfg.models_dir.find(".xllm"), std::string::npos);
    EXPECT_NE(cfg.models_dir.find("models"), std::string::npos);

    fs::remove_all(tmp_home);
}
