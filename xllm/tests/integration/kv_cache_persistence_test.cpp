#include <gtest/gtest.h>
#include <httplib.h>
#include <filesystem>
#include <string>
#include <vector>

#include "api/http_server.h"
#include "api/openai_endpoints.h"
#include "api/node_endpoints.h"
#include "core/inference_engine.h"
#include "core/kv_cache_utils.h"
#include "models/model_registry.h"
#include "runtime/state.h"
#include "utils/config.h"

using namespace xllm;
namespace fs = std::filesystem;

namespace {
class TempDir {
public:
    TempDir() {
        auto base = fs::temp_directory_path() / fs::path("kv-cache-XXXXXX");
        std::string tmpl = base.string();
        std::vector<char> buf(tmpl.begin(), tmpl.end());
        buf.push_back('\0');
        char* created = mkdtemp(buf.data());
        path = created ? fs::path(created) : fs::temp_directory_path();
    }
    ~TempDir() {
        std::error_code ec;
        fs::remove_all(path, ec);
    }
    fs::path path;
};

class EnvGuard {
public:
    EnvGuard(const std::string& key, const std::string& value) : key_(key) {
        const char* prev = std::getenv(key.c_str());
        if (prev) {
            had_prev_ = true;
            prev_value_ = prev;
        }
#ifdef _WIN32
        _putenv_s(key.c_str(), value.c_str());
#else
        setenv(key.c_str(), value.c_str(), 1);
#endif
    }

    ~EnvGuard() {
#ifdef _WIN32
        if (had_prev_) {
            _putenv_s(key_.c_str(), prev_value_.c_str());
        } else {
            _putenv_s(key_.c_str(), "");
        }
#else
        if (had_prev_) {
            setenv(key_.c_str(), prev_value_.c_str(), 1);
        } else {
            unsetenv(key_.c_str());
        }
#endif
    }

private:
    std::string key_;
    bool had_prev_{false};
    std::string prev_value_;
};
}  // namespace

TEST(KvCachePersistenceTest, SavesAndRestoresStubCache) {
    TempDir temp;
    EnvGuard env("XLLM_KV_CACHE_DIR", temp.path.string());

    xllm::set_ready(true);
    ModelRegistry registry;
    registry.setModels({"gpt-oss-7b"});
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config, GpuBackend::Cpu);
    NodeEndpoints node;
    HttpServer server(18101, openai, node);
    server.start();

    std::vector<ChatMessage> messages = {{"user", "hello"}};
    std::string prompt = buildChatMLPrompt(messages);
    fs::path cache_path = build_kv_cache_path("gpt-oss-7b", prompt, temp.path.string());
    fs::path hit_path = cache_path;
    hit_path += ".hit";

    httplib::Client cli("127.0.0.1", 18101);
    std::string body = R"({"model":"gpt-oss-7b","messages":[{"role":"user","content":"hello"}]})";

    auto first = cli.Post("/v1/chat/completions", body, "application/json");
    ASSERT_TRUE(first);
    EXPECT_EQ(first->status, 200);
    EXPECT_TRUE(fs::exists(cache_path));
    EXPECT_FALSE(fs::exists(hit_path));

    auto second = cli.Post("/v1/chat/completions", body, "application/json");
    ASSERT_TRUE(second);
    EXPECT_EQ(second->status, 200);
    EXPECT_TRUE(fs::exists(cache_path));
    EXPECT_TRUE(fs::exists(hit_path));

    server.stop();
}
