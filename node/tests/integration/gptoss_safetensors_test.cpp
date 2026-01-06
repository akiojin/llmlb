#include <gtest/gtest.h>
#include <httplib.h>

#include <algorithm>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <optional>
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

struct GptOssTestModel {
    fs::path models_dir;
    std::string model_id;
    fs::path model_dir;
};

std::optional<GptOssTestModel> resolve_gptoss_test_model(std::string& error) {
    const char* model_dir_env = std::getenv("LLM_NODE_GPTOSS_TEST_MODEL_DIR");
    const char* models_dir_env = std::getenv("LLM_NODE_GPTOSS_TEST_MODELS_DIR");
    const char* model_id_env = std::getenv("LLM_NODE_GPTOSS_TEST_MODEL_ID");

    fs::path model_dir;
    fs::path models_dir;
    std::string model_id;

    if (model_dir_env && *model_dir_env) {
        model_dir = fs::path(model_dir_env);
    }
    if (models_dir_env && *models_dir_env) {
        models_dir = fs::path(models_dir_env);
    }
    if (model_id_env && *model_id_env) {
        model_id = model_id_env;
    }

    if (models_dir.empty()) {
        if (model_dir.empty()) {
            error = "LLM_NODE_GPTOSS_TEST_MODEL_DIR or LLM_NODE_GPTOSS_TEST_MODELS_DIR is not set";
            return std::nullopt;
        }
        models_dir = model_dir.parent_path();
    }

    if (model_id.empty()) {
        if (model_dir.empty()) {
            error = "LLM_NODE_GPTOSS_TEST_MODEL_ID is not set";
            return std::nullopt;
        }
        std::error_code ec;
        auto rel = fs::relative(model_dir, models_dir, ec);
        if (ec || rel.empty() || rel.string().rfind("..", 0) == 0) {
            error = "gpt-oss model dir must be under models dir";
            return std::nullopt;
        }
        model_id = ModelStorage::dirNameToModel(rel.string());
    }

    if (model_dir.empty()) {
        model_dir = models_dir / ModelStorage::modelNameToDir(model_id);
    }

    if (!fs::exists(model_dir)) {
        error = "gpt-oss model directory does not exist";
        return std::nullopt;
    }

    if (!fs::exists(model_dir / "config.json") || !fs::exists(model_dir / "tokenizer.json")) {
        error = "config.json or tokenizer.json is missing in gpt-oss model directory";
        return std::nullopt;
    }

    const auto index_path = model_dir / "model.safetensors.index.json";
    const auto st_path = model_dir / "model.safetensors";
    if (!fs::exists(index_path) && !fs::exists(st_path)) {
        error = "safetensors index/model file not found in gpt-oss model directory";
        return std::nullopt;
    }

    GptOssTestModel model;
    model.models_dir = models_dir;
    model.model_id = model_id;
    model.model_dir = model_dir;
    return model;
}

bool has_metal_artifact(const fs::path& model_dir) {
    return fs::exists(model_dir / "model.metal.bin") ||
           fs::exists(model_dir / "metal" / "model.bin") ||
           fs::exists(model_dir / "model.bin");
}

bool has_directml_artifact(const fs::path& model_dir) {
    return fs::exists(model_dir / "model.directml.bin") ||
           fs::exists(model_dir / "model.dml.bin");
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

TEST(GptOssSafetensorsIntegrationTest, GeneratesTokenFromMetalArtifactE2E) {
#if !defined(__APPLE__)
    GTEST_SKIP() << "Metal backend is only supported on macOS";
#elif !defined(USE_GPTOSS)
    GTEST_SKIP() << "USE_GPTOSS not enabled";
#else
    std::string error;
    auto model = resolve_gptoss_test_model(error);
    if (!model) {
        GTEST_SKIP() << error;
    }
    if (!has_metal_artifact(model->model_dir)) {
        GTEST_SKIP() << "Metal artifact not found in model dir";
    }

    llm_node::set_ready(true);
    ModelStorage storage(model->models_dir.string());
    LlamaManager llama(model->models_dir.string());
    InferenceEngine engine(llama, storage);
    ModelRegistry registry;
    registry.setModels({model->model_id});

    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config, GpuBackend::kCpu);
    NodeEndpoints node;
    HttpServer server(18150, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18150);
    std::string body =
        std::string(R"({"model":")") + model->model_id +
        R"(","messages":[{"role":"user","content":"hello"}]})";
    auto res = cli.Post("/v1/chat/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    EXPECT_NE(res->body.find("\"content\""), std::string::npos);

    server.stop();
#endif
}

TEST(GptOssSafetensorsIntegrationTest, GeneratesTokenFromDirectmlArtifactE2E) {
#if !defined(_WIN32)
    GTEST_SKIP() << "DirectML backend is only supported on Windows";
#elif !defined(USE_GPTOSS)
    GTEST_SKIP() << "USE_GPTOSS not enabled";
#elif !defined(USE_DIRECTML)
    GTEST_SKIP() << "DirectML support is frozen";
#else
    std::string error;
    auto model = resolve_gptoss_test_model(error);
    if (!model) {
        GTEST_SKIP() << error;
    }
    if (!has_directml_artifact(model->model_dir)) {
        GTEST_SKIP() << "DirectML artifact not found in model dir";
    }

    llm_node::set_ready(true);
    ModelStorage storage(model->models_dir.string());
    LlamaManager llama(model->models_dir.string());
    InferenceEngine engine(llama, storage);
    ModelRegistry registry;
    registry.setModels({model->model_id});

    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config, GpuBackend::kCpu);
    NodeEndpoints node;
    HttpServer server(18151, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18151);
    std::string body =
        std::string(R"({"model":")") + model->model_id +
        R"(","messages":[{"role":"user","content":"hello"}]})";
    auto res = cli.Post("/v1/chat/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    EXPECT_NE(res->body.find("\"content\""), std::string::npos);

    server.stop();
#endif
}
