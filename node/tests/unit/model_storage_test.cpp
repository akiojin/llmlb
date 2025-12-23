// SPEC-dcaeaec4: ModelStorage unit tests (TDD RED phase)
#include <gtest/gtest.h>
#include <filesystem>
#include <fstream>

#include "models/model_storage.h"

using namespace llm_node;
namespace fs = std::filesystem;

class TempModelDir {
public:
    TempModelDir() {
        base = fs::temp_directory_path() / fs::path("model-storage-XXXXXX");
        std::string tmpl = base.string();
        std::vector<char> buf(tmpl.begin(), tmpl.end());
        buf.push_back('\0');
        char* created = mkdtemp(buf.data());
        base = created ? fs::path(created) : fs::temp_directory_path();
    }
    ~TempModelDir() {
        std::error_code ec;
        fs::remove_all(base, ec);
    }
    fs::path base;
};

// Helper: create model directory with model.gguf
static void create_model(const fs::path& models_dir, const std::string& dir_name) {
    auto model_dir = models_dir / dir_name;
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "dummy gguf content";
}

static void create_safetensors_model_with_index(const fs::path& models_dir, const std::string& dir_name) {
    auto model_dir = models_dir / dir_name;
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "config.json") << R"({"architectures":["NemotronForCausalLM"]})";
    std::ofstream(model_dir / "tokenizer.json") << R"({"dummy":true})";
    std::ofstream(model_dir / "model.safetensors.index.json") << R"({"weight_map":{}})";
}

static void create_gptoss_safetensors_model_with_index(const fs::path& models_dir, const std::string& dir_name) {
    auto model_dir = models_dir / dir_name;
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "config.json") << R"({"model_type":"gpt_oss","architectures":["GptOssForCausalLM"]})";
    std::ofstream(model_dir / "tokenizer.json") << R"({"dummy":true})";
    std::ofstream(model_dir / "model.safetensors.index.json") << R"({"weight_map":{}})";
}

// FR-2: Model name format conversion (sanitized, lowercase)
TEST(ModelStorageTest, ConvertModelNameToDirectoryName) {
    EXPECT_EQ(ModelStorage::modelNameToDir("gpt-oss-20b"), "gpt-oss-20b");
    EXPECT_EQ(ModelStorage::modelNameToDir("Mistral-7B-Instruct-v0.2"), "mistral-7b-instruct-v0.2");
    EXPECT_EQ(ModelStorage::modelNameToDir("model@name"), "model_name");
}

// FR-3: resolveGguf returns correct path
TEST(ModelStorageTest, ResolveGgufReturnsPathWhenPresent) {
    TempModelDir tmp;
    create_model(tmp.base, "gpt-oss-20b");

    ModelStorage storage(tmp.base.string());
    auto path = storage.resolveGguf("gpt-oss-20b");

    EXPECT_FALSE(path.empty());
    EXPECT_TRUE(fs::exists(path));
    EXPECT_EQ(fs::path(path).filename(), "model.gguf");
}

// FR-3: resolveGguf returns empty when model not found
TEST(ModelStorageTest, ResolveGgufReturnsEmptyWhenMissing) {
    TempModelDir tmp;
    ModelStorage storage(tmp.base.string());
    EXPECT_EQ(storage.resolveGguf("nonexistent"), "");
}

// FR-4: listAvailable returns all models with model.gguf
TEST(ModelStorageTest, ListAvailableReturnsAllModels) {
    TempModelDir tmp;
    create_model(tmp.base, "gpt-oss-20b");
    create_model(tmp.base, "gpt-oss-7b");
    create_model(tmp.base, "qwen3-coder-30b");
    create_safetensors_model_with_index(tmp.base, "nvidia-nemotron");

    ModelStorage storage(tmp.base.string());
    auto list = storage.listAvailable();

    ASSERT_EQ(list.size(), 4u);

    std::vector<std::string> names;
    for (const auto& m : list) {
        names.push_back(m.name);
    }
    std::sort(names.begin(), names.end());

    EXPECT_EQ(names[0], "gpt-oss-20b");
    EXPECT_EQ(names[1], "gpt-oss-7b");
    EXPECT_EQ(names[2], "nvidia-nemotron");
    EXPECT_EQ(names[3], "qwen3-coder-30b");
}

// FR-4: Directories without model.gguf are ignored
TEST(ModelStorageTest, IgnoresDirectoriesWithoutGguf) {
    TempModelDir tmp;
    create_model(tmp.base, "valid_model");
    // Create directory without model.gguf
    fs::create_directories(tmp.base / "invalid_model");

    ModelStorage storage(tmp.base.string());
    auto list = storage.listAvailable();

    ASSERT_EQ(list.size(), 1u);
    EXPECT_EQ(list[0].name, "valid_model");
}

TEST(ModelStorageTest, ResolveDescriptorFallsBackToGguf) {
    TempModelDir tmp;
    create_model(tmp.base, "gpt-oss-7b");

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor("gpt-oss-7b");

    ASSERT_TRUE(desc.has_value());
    EXPECT_EQ(desc->runtime, "llama_cpp");
    EXPECT_EQ(desc->format, "gguf");
    EXPECT_EQ(fs::path(desc->primary_path).filename(), "model.gguf");
}

TEST(ModelStorageTest, ResolveDescriptorFindsSafetensorsIndex) {
    TempModelDir tmp;
    create_safetensors_model_with_index(tmp.base, "nemotron-30b");

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor("nemotron-30b");

    ASSERT_TRUE(desc.has_value());
    EXPECT_EQ(desc->runtime, "nemotron_cpp");
    EXPECT_EQ(desc->format, "safetensors");
    EXPECT_EQ(fs::path(desc->primary_path).filename(), "model.safetensors.index.json");
}

TEST(ModelStorageTest, ResolveDescriptorFindsGptOssSafetensorsIndex) {
    TempModelDir tmp;
    create_gptoss_safetensors_model_with_index(tmp.base, "openai-gpt-oss-20b");

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor("openai-gpt-oss-20b");

    ASSERT_TRUE(desc.has_value());
    EXPECT_EQ(desc->runtime, "gptoss_cpp");
    EXPECT_EQ(desc->format, "safetensors");
    EXPECT_EQ(fs::path(desc->primary_path).filename(), "model.safetensors.index.json");
}

TEST(ModelStorageTest, ResolveDescriptorSkipsSafetensorsWhenMetadataMissing) {
    TempModelDir tmp;
    auto model_dir = tmp.base / "nemotron-30b";
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.safetensors.index.json") << R"({"weight_map":{}})";

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor("nemotron-30b");
    EXPECT_FALSE(desc.has_value());
}

TEST(ModelStorageTest, ListAvailableDescriptorsIncludesGgufAndNemotronSafetensors) {
    TempModelDir tmp;
    create_model(tmp.base, "gpt-oss-20b");
    create_safetensors_model_with_index(tmp.base, "nemotron-30b");

    ModelStorage storage(tmp.base.string());
    auto list = storage.listAvailableDescriptors();

    // gguf + nemotron safetensors
    ASSERT_EQ(list.size(), 2u);
    std::vector<std::string> formats;
    for (const auto& d : list) formats.push_back(d.format);
    std::sort(formats.begin(), formats.end());
    EXPECT_EQ(formats[0], "gguf");
    EXPECT_EQ(formats[1], "safetensors");
}

// Edge case: Empty model name
TEST(ModelStorageTest, HandleEmptyModelName) {
    EXPECT_EQ(ModelStorage::modelNameToDir(""), "_latest");
}

// Validation: Model with valid GGUF file
TEST(ModelStorageTest, ValidateModelWithGguf) {
    TempModelDir tmp;
    create_model(tmp.base, "gpt-oss-20b");

    ModelStorage storage(tmp.base.string());
    EXPECT_TRUE(storage.validateModel("gpt-oss-20b"));
    EXPECT_FALSE(storage.validateModel("nonexistent"));
}

// Directory conversion: directory name to model id (best-effort)
TEST(ModelStorageTest, ConvertDirNameToModelName) {
    EXPECT_EQ(ModelStorage::dirNameToModel("gpt-oss-20b"), "gpt-oss-20b");
    EXPECT_EQ(ModelStorage::dirNameToModel("Qwen3-Coder-30B"), "qwen3-coder-30b");
}

// Delete model directory (SPEC-dcaeaec4 FR-6/FR-7)
TEST(ModelStorageTest, DeleteModelRemovesDirectory) {
    TempModelDir tmp;
    create_model(tmp.base, "to-delete");

    ModelStorage storage(tmp.base.string());
    EXPECT_TRUE(storage.validateModel("to-delete"));

    bool result = storage.deleteModel("to-delete");
    EXPECT_TRUE(result);
    EXPECT_FALSE(storage.validateModel("to-delete"));
    EXPECT_FALSE(fs::exists(tmp.base / "to-delete"));
}

// Delete nonexistent model returns true (idempotent)
TEST(ModelStorageTest, DeleteNonexistentModelReturnsTrue) {
    TempModelDir tmp;
    ModelStorage storage(tmp.base.string());
    EXPECT_TRUE(storage.deleteModel("nonexistent"));
}
