#include <gtest/gtest.h>
#include <cstdlib>
#include <fstream>
#include <filesystem>
#include <unordered_map>

#include "utils/config.h"

using namespace allm;
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
    EnvGuard guard({"ALLM_CONFIG", "LLM_MODELS_DIR",
                    "ALLM_PORT", "ALLM_ENGINE_PLUGINS_DIR"});

    fs::path tmp = fs::temp_directory_path() / "nodecfg.json";
    std::ofstream(tmp) << R"({
        "models_dir": "/tmp/models",
        "node_port": 18080,
        "engine_plugins_dir": "/tmp/engines",
        "require_gpu": false
    })";
    setenv("ALLM_CONFIG", tmp.string().c_str(), 1);

    auto info = loadNodeConfigWithLog();
    auto cfg = info.first;

    EXPECT_EQ(cfg.models_dir, "/tmp/models");
    EXPECT_EQ(cfg.engine_plugins_dir, "/tmp/engines");
    EXPECT_EQ(cfg.node_port, 18080);
    EXPECT_TRUE(cfg.require_gpu);  // require_gpu is forced to true
    EXPECT_NE(info.second.find("file="), std::string::npos);

    fs::remove(tmp);
}

TEST(UtilsConfigTest, EnvOverridesNodeConfig) {
    EnvGuard guard({"LLM_MODELS_DIR", "ALLM_PORT", "ALLM_CONFIG",
                    "ALLM_MODELS_DIR", "ALLM_ENGINE_PLUGINS_DIR"});

    unsetenv("ALLM_CONFIG");
    // Test with deprecated env var names (fallback)
    setenv("LLM_MODELS_DIR", "/env/models", 1);
    setenv("ALLM_PORT", "19000", 1);
    setenv("ALLM_ENGINE_PLUGINS_DIR", "/env/engines", 1);
    auto cfg = loadNodeConfig();
    EXPECT_EQ(cfg.models_dir, "/env/models");
    EXPECT_EQ(cfg.engine_plugins_dir, "/env/engines");
    EXPECT_EQ(cfg.node_port, 19000);
    EXPECT_TRUE(cfg.require_gpu);  // env flags are ignored, GPU required
}

TEST(UtilsConfigTest, NewEnvVarsTakePriorityOverDeprecated) {
    EnvGuard guard({"ALLM_MODELS_DIR", "LLM_MODELS_DIR",
                    "ALLM_PORT", "ALLM_CONFIG",
                    "ALLM_ENGINE_PLUGINS_DIR"});

    unsetenv("ALLM_CONFIG");

    // Set both new and deprecated env vars
    setenv("ALLM_MODELS_DIR", "/new/models", 1);
    setenv("LLM_MODELS_DIR", "/old/models", 1);  // Should be ignored
    auto cfg = loadNodeConfig();
    EXPECT_EQ(cfg.models_dir, "/new/models");
    EXPECT_TRUE(cfg.require_gpu);  // GPU requirement cannot be disabled
}
