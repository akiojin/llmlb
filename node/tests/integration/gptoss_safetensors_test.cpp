#include <gtest/gtest.h>
#include <httplib.h>

#include <algorithm>
#include <filesystem>
#include <fstream>
#include <string>
#include <vector>

#include "api/http_server.h"
#include "api/node_endpoints.h"
#include "api/openai_endpoints.h"
#include "core/inference_engine.h"
#include "core/llama_manager.h"
#include "models/model_registry.h"
#include "models/model_storage.h"
#include "runtime/state.h"
#include "utils/config.h"

using namespace llm_node;
namespace fs = std::filesystem;

namespace {
class TempModelDir {
public:
    TempModelDir() {
        base_ = fs::temp_directory_path() / fs::path("gptoss-safetensors-XXXXXX");
        std::string tmpl = base_.string();
        std::vector<char> buf(tmpl.begin(), tmpl.end());
        buf.push_back('\0');
        char* created = mkdtemp(buf.data());
        base_ = created ? fs::path(created) : fs::temp_directory_path();
    }
    ~TempModelDir() {
        std::error_code ec;
        fs::remove_all(base_, ec);
    }
    const fs::path& path() const { return base_; }

private:
    fs::path base_;
};

fs::path ensure_model_dir(const fs::path& models_dir, const std::string& model_id) {
    fs::path dir = models_dir / fs::path(model_id);
    fs::create_directories(dir);
    return dir;
}

void write_text(const fs::path& path, const std::string& content) {
    std::ofstream ofs(path);
    ofs << content;
}

void create_gguf_model(const fs::path& models_dir, const std::string& model_id) {
    auto dir = ensure_model_dir(models_dir, model_id);
    write_text(dir / "model.gguf", "dummy gguf");
}

void create_safetensors_missing_metadata(const fs::path& models_dir, const std::string& model_id) {
    auto dir = ensure_model_dir(models_dir, model_id);
    write_text(dir / "model.safetensors.index.json", R"({"weight_map":{}})");
    // config.json / tokenizer.json are intentionally missing.
}

void create_gptoss_safetensors_model(const fs::path& models_dir, const std::string& model_id) {
    auto dir = ensure_model_dir(models_dir, model_id);
    write_text(dir / "config.json", R"({"model_type":"gpt_oss","architectures":["GptOssForCausalLM"]})");
    write_text(dir / "tokenizer.json", R"({"dummy":true})");
    write_text(dir / "model.safetensors.index.json", R"({"weight_map":{}})");
}
}  // namespace

TEST(GptOssSafetensorsIntegrationTest, ExcludesMissingMetadataModels) {
    TempModelDir tmp;
    create_gguf_model(tmp.path(), "llama-3.1-8b");
    create_safetensors_missing_metadata(tmp.path(), "openai/gpt-oss-20b");

    ModelStorage storage(tmp.path().string());
    LlamaManager llama(tmp.path().string());
    InferenceEngine engine(llama, storage);

    auto descriptors = storage.listAvailableDescriptors();
    std::vector<std::string> names;
    for (const auto& desc : descriptors) {
        if (engine.isModelSupported(desc)) {
            names.push_back(desc.name);
        }
    }

    EXPECT_NE(std::find(names.begin(), names.end(), "llama-3.1-8b"), names.end());
    EXPECT_EQ(std::find(names.begin(), names.end(), "openai/gpt-oss-20b"), names.end());
}

// TDD RED: gpt-oss safetensors inference path is not implemented yet.
TEST(GptOssSafetensorsIntegrationTest, GeneratesTokenFromSafetensorsE2E) {
    GTEST_SKIP() << "TDD RED: gpt-oss safetensors inference path not implemented yet";
    llm_node::set_ready(true);
    TempModelDir tmp;
    create_gptoss_safetensors_model(tmp.path(), "openai/gpt-oss-20b");

    ModelStorage storage(tmp.path().string());
    LlamaManager llama(tmp.path().string());
    InferenceEngine engine(llama, storage);
    ModelRegistry registry;
    registry.setModels({"openai/gpt-oss-20b"});

    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config);
    NodeEndpoints node;
    HttpServer server(18150, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18150);
    std::string body = R"({"model":"openai/gpt-oss-20b","messages":[{"role":"user","content":"hello"}]})";
    auto res = cli.Post("/v1/chat/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    EXPECT_NE(res->body.find("\"content\""), std::string::npos);

    server.stop();
}
