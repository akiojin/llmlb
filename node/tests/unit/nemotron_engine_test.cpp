#include <gtest/gtest.h>

#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <vector>

#include <nlohmann/json.hpp>

#include "core/nemotron_engine.h"
#include "models/model_descriptor.h"
#include "safetensors.hh"

namespace fs = std::filesystem;

namespace {
const char kKnownTensorName[] = "backbone.layers.1.mixer.experts.0.down_proj.weight";

class TempDir {
public:
    TempDir() {
        base = fs::temp_directory_path() / fs::path("nemotron-engine-XXXXXX");
        std::string tmpl = base.string();
        std::vector<char> buf(tmpl.begin(), tmpl.end());
        buf.push_back('\0');
        char* created = mkdtemp(buf.data());
        base = created ? fs::path(created) : fs::temp_directory_path();
    }
    ~TempDir() {
        std::error_code ec;
        fs::remove_all(base, ec);
    }
    fs::path base;
};

bool write_safetensors_file(const fs::path& path, const std::string& tensor_name) {
    safetensors::safetensors_t st;
    safetensors::tensor_t tensor;
    tensor.dtype = safetensors::kFLOAT32;
    tensor.shape = {1};
    tensor.data_offsets = {0, sizeof(float)};
    st.tensors.insert(tensor_name, tensor);

    st.storage.resize(sizeof(float));
    float value = 1.0f;
    std::memcpy(st.storage.data(), &value, sizeof(float));

    std::string warn;
    std::string err;
    bool ok = safetensors::save_to_file(st, path.string(), &warn, &err);
    return ok;
}

void write_required_metadata(const fs::path& dir) {
    nlohmann::json config = {{"model_type", "nemotron"}};
    std::ofstream(dir / "config.json") << config.dump();
    nlohmann::json tokenizer = nlohmann::json::object();
    std::ofstream(dir / "tokenizer.json") << tokenizer.dump();
}
}  // namespace

TEST(NemotronEngineTest, LoadsIndexAndValidatesShard) {
    TempDir tmp;
    const fs::path shard_path = tmp.base / "model-00001-of-00001.safetensors";
    ASSERT_TRUE(write_safetensors_file(shard_path, kKnownTensorName));

    nlohmann::json index = {
        {"metadata", {{"total_size", 4}}},
        {"weight_map", {{kKnownTensorName, shard_path.filename().string()}}},
    };
    const fs::path index_path = tmp.base / "model.safetensors.index.json";
    std::ofstream(index_path) << index.dump();

    write_required_metadata(tmp.base);

    llm_node::ModelDescriptor desc;
    desc.name = "nemotron-test";
    desc.runtime = "nemotron_cpp";
    desc.format = "safetensors";
    desc.primary_path = index_path.string();
    desc.model_dir = tmp.base.string();

    llm_node::NemotronEngine engine;
    auto result = engine.loadModel(desc);

    EXPECT_TRUE(result.success) << result.error_message;
}

TEST(NemotronEngineTest, SupportsTextGenerationDependsOnCuda) {
    llm_node::NemotronEngine engine;
#if defined(_WIN32) && defined(USE_GPTOSS)
    EXPECT_TRUE(engine.supportsTextGeneration());
#elif defined(USE_CUDA)
    EXPECT_TRUE(engine.supportsTextGeneration());
#else
    EXPECT_FALSE(engine.supportsTextGeneration());
#endif
}

TEST(NemotronEngineTest, FailsWithoutRequiredMetadata) {
    TempDir tmp;
    const fs::path shard_path = tmp.base / "model-00001-of-00001.safetensors";
    ASSERT_TRUE(write_safetensors_file(shard_path, kKnownTensorName));

    llm_node::ModelDescriptor desc;
    desc.name = "nemotron-test";
    desc.runtime = "nemotron_cpp";
    desc.format = "safetensors";
    desc.primary_path = shard_path.string();
    desc.model_dir = tmp.base.string();

    llm_node::NemotronEngine engine;
    auto result = engine.loadModel(desc);

    EXPECT_FALSE(result.success);
    EXPECT_NE(result.error_message.find("config.json"), std::string::npos);
}

TEST(NemotronEngineTest, DirectmlRuntimeMissingReportsError) {
#if !defined(_WIN32)
    GTEST_SKIP() << "DirectML backend is only supported on Windows";
#elif !defined(USE_GPTOSS)
    GTEST_SKIP() << "USE_GPTOSS not enabled";
#else
    TempDir tmp;
    auto model_dir = tmp.base / "nvidia" / "nemotron";
    fs::create_directories(model_dir);
    write_required_metadata(model_dir);
    std::ofstream(model_dir / "model.safetensors") << "";
    std::ofstream(model_dir / "model.directml.bin") << "cache";

    llm_node::ModelDescriptor desc;
    desc.name = "nvidia/nemotron";
    desc.runtime = "nemotron_cpp";
    desc.format = "safetensors";
    desc.model_dir = model_dir.string();
    desc.primary_path = (model_dir / "model.safetensors").string();

    struct EnvGuard {
        const char* key;
        std::string prev;
        bool had_prev{false};
        explicit EnvGuard(const char* k, const std::string& value) : key(k) {
            if (const char* v = std::getenv(key)) {
                prev = v;
                had_prev = true;
            }
            _putenv_s(key, value.c_str());
        }
        ~EnvGuard() {
            if (had_prev) {
                _putenv_s(key, prev.c_str());
            } else {
                _putenv_s(key, "");
            }
        }
    };

    auto missing_path = (tmp.base / "missing-nemotron-directml.dll").string();
    EnvGuard guard("LLM_NODE_NEMOTRON_DML_LIB", missing_path);

    llm_node::NemotronEngine engine;
    auto result = engine.loadModel(desc);
    EXPECT_FALSE(result.success);
    EXPECT_NE(result.error_message.find("DirectML runtime"), std::string::npos);
#endif
}

TEST(NemotronEngineTest, MissingPrimaryPathReportsError) {
#if !defined(_WIN32)
    GTEST_SKIP() << "DirectML backend is only supported on Windows";
#elif !defined(USE_GPTOSS)
    GTEST_SKIP() << "USE_GPTOSS not enabled";
#else
    TempDir tmp;
    auto model_dir = tmp.base / "nvidia" / "nemotron";
    fs::create_directories(model_dir);
    write_required_metadata(model_dir);

    llm_node::ModelDescriptor desc;
    desc.name = "nvidia/nemotron";
    desc.runtime = "nemotron_cpp";
    desc.format = "safetensors";
    desc.model_dir = model_dir.string();
    desc.primary_path = (model_dir / "model.safetensors").string();

    llm_node::NemotronEngine engine;
    auto result = engine.loadModel(desc);
    EXPECT_FALSE(result.success);
    EXPECT_NE(result.error_message.find("Primary path not found"), std::string::npos);
#endif
}
