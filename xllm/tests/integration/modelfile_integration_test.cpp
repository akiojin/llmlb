#include <gtest/gtest.h>
#include <httplib.h>
#include <filesystem>
#include <fstream>
#include <vector>
#include <nlohmann/json.hpp>

#include "api/http_server.h"
#include "api/openai_endpoints.h"
#include "api/node_endpoints.h"
#include "core/inference_engine.h"
#include "models/model_registry.h"
#include "models/modelfile.h"
#include "runtime/state.h"

namespace fs = std::filesystem;
using namespace xllm;

namespace {
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

fs::path make_temp_dir() {
    auto base = fs::temp_directory_path() / fs::path("modelfile-int-XXXXXX");
    std::string tmpl = base.string();
    std::vector<char> buf(tmpl.begin(), tmpl.end());
    buf.push_back('\0');
    char* created = mkdtemp(buf.data());
    return created ? fs::path(created) : fs::temp_directory_path();
}
}

TEST(ModelfileIntegrationTest, AppliesStopSequenceFromModelfile) {
    xllm::set_ready(true);

    const fs::path temp_dir = make_temp_dir();
    EnvGuard home_guard("HOME", temp_dir.string());

    const std::string model_name = "gpt-oss-7b";
    const fs::path path = xllm::Modelfile::pathForModel(model_name);
    fs::create_directories(path.parent_path());

    std::ofstream ofs(path);
    ofs << "FROM gpt-oss-7b\n";
    ofs << "PARAMETER stop \"STOP\"\n";
    ofs.close();

    ModelRegistry registry;
    registry.setModels({model_name});
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config, GpuBackend::Cpu);
    NodeEndpoints node;
    HttpServer server(18160, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18160);
    std::string body = R"({"model":"gpt-oss-7b","messages":[{"role":"user","content":"ping STOP pong"}]})";
    auto res = cli.Post("/v1/chat/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    auto j = nlohmann::json::parse(res->body);
    std::string content = j["choices"][0]["message"]["content"];
    EXPECT_EQ(content, "Response to: ping ");

    server.stop();
}
