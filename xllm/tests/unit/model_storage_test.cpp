// SPEC-dcaeaec4: ModelStorage unit tests (TDD RED phase)
#include <gtest/gtest.h>
#include <algorithm>
#include <filesystem>
#include <fstream>
#include <nlohmann/json.hpp>

#include "models/model_storage.h"

using namespace xllm;
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

static void create_gguf_file(const fs::path& model_dir, const std::string& filename) {
    fs::create_directories(model_dir);
    std::ofstream(model_dir / filename) << "dummy gguf content";
}

static void create_safetensors_model_with_index(const fs::path& models_dir, const std::string& dir_name) {
    auto model_dir = models_dir / dir_name;
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "config.json") << R"({"architectures":["LlamaForCausalLM"]})";
    std::ofstream(model_dir / "tokenizer.json") << R"({"dummy":true})";
    std::ofstream(model_dir / "model.safetensors.index.json") << R"({"weight_map":{}})";
}

static void create_safetensors_index_with_missing_shard(const fs::path& models_dir, const std::string& dir_name) {
    auto model_dir = models_dir / dir_name;
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "config.json") << R"({"architectures":["LlamaForCausalLM"]})";
    std::ofstream(model_dir / "tokenizer.json") << R"({"dummy":true})";
    std::ofstream(model_dir / "model.safetensors.index.json") << R"({
        "weight_map": {
            "model.layers.0.weight": "model-00001.safetensors"
        }
    })";
    // NOTE: shard file is intentionally missing to exercise validation logic.
}

static void create_safetensors_model_with_shards(const fs::path& models_dir, const std::string& dir_name) {
    auto model_dir = models_dir / dir_name;
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "config.json") << R"({"model_type":"llama","architectures":["LlamaForCausalLM"]})";
    std::ofstream(model_dir / "tokenizer.json") << R"({"dummy":true})";
    std::ofstream(model_dir / "model-00001.safetensors") << "shard1";
    std::ofstream(model_dir / "model-00002.safetensors") << "shard2";
    std::ofstream(model_dir / "model.safetensors.index.json") << R"({
        "weight_map": {
            "model.layers.0.weight": "model-00001.safetensors",
            "model.layers.1.weight": "model-00002.safetensors"
        }
    })";
}

static void create_safetensors_model_with_architectures(
    const fs::path& models_dir,
    const std::string& dir_name,
    const std::vector<std::string>& architectures) {
    auto model_dir = models_dir / dir_name;
    fs::create_directories(model_dir);
    nlohmann::json j;
    j["architectures"] = architectures;
    std::ofstream(model_dir / "config.json") << j.dump();
    std::ofstream(model_dir / "tokenizer.json") << R"({"dummy":true})";
    std::ofstream(model_dir / "model.safetensors") << "dummy";
}

static void write_manifest_with_format(const fs::path& model_dir, const std::string& format) {
    std::ofstream(model_dir / "manifest.json") << std::string(R"({"format":")") + format + R"(","files":[]})";
}

static void write_manifest_with_formats(const fs::path& model_dir, const std::vector<std::string>& formats) {
    nlohmann::json j;
    j["formats"] = formats;
    j["files"] = nlohmann::json::array();
    std::ofstream(model_dir / "manifest.json") << j.dump();
}

static void write_manifest_with_format_and_quantization(
    const fs::path& model_dir,
    const std::string& format,
    const std::string& quantization) {
    nlohmann::json j;
    j["format"] = format;
    j["quantization"] = quantization;
    j["files"] = nlohmann::json::array();
    std::ofstream(model_dir / "manifest.json") << j.dump();
}

// FR-2: Model name format conversion (sanitized, lowercase)
TEST(ModelStorageTest, ConvertModelNameToDirectoryName) {
    EXPECT_EQ(ModelStorage::modelNameToDir("gpt-oss-20b"), "gpt-oss-20b");
    EXPECT_EQ(ModelStorage::modelNameToDir("Mistral-7B-Instruct-v0.2"), "mistral-7b-instruct-v0.2");
    EXPECT_EQ(ModelStorage::modelNameToDir("model@name"), "model_name");
}

TEST(ModelStorageTest, ParseModelNameWithQuantization) {
    auto parsed = ModelStorage::parseModelName("llama-7b:Q4_K_M");
    ASSERT_TRUE(parsed.has_value());
    EXPECT_EQ(parsed->base, "llama-7b");
    ASSERT_TRUE(parsed->quantization.has_value());
    EXPECT_EQ(*parsed->quantization, "Q4_K_M");
}

TEST(ModelStorageTest, ParseModelNameRejectsInvalidQuantizationFormat) {
    EXPECT_FALSE(ModelStorage::parseModelName("llama-7b:").has_value());
    EXPECT_FALSE(ModelStorage::parseModelName(":Q4_K_M").has_value());
    EXPECT_FALSE(ModelStorage::parseModelName("llama-7b:Q4_K_M:extra").has_value());
}

TEST(ModelStorageTest, NormalizesArchitectureFamiliesFromConfig) {
    TempModelDir tmp;
    ModelStorage storage(tmp.base.string());

    create_safetensors_model_with_architectures(
        tmp.base,
        "family-test",
        {"GptOssForCausalLM", "NemotronForCausalLM", "Qwen2ForCausalLM", "ChatGLM4ForCausalLM"});

    auto desc = storage.resolveDescriptor("family-test");
    ASSERT_TRUE(desc.has_value());
    EXPECT_EQ(desc->runtime, "safetensors_cpp");
    EXPECT_EQ(desc->format, "safetensors");
    std::vector<std::string> expected = {"gptoss", "nemotron", "qwen", "glm"};
    EXPECT_EQ(desc->architectures, expected);
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

TEST(ModelStorageTest, ResolveDescriptorAddsManifestQuantization) {
    TempModelDir tmp;
    auto model_dir = tmp.base / "quantized-model";
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "gguf";
    write_manifest_with_format_and_quantization(model_dir, "gguf", "Q4_K_M");

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor("quantized-model");
    ASSERT_TRUE(desc.has_value());
    ASSERT_TRUE(desc->metadata.has_value());
    EXPECT_EQ(desc->metadata->value("quantization", ""), "Q4_K_M");
    EXPECT_EQ(desc->metadata->value("quantization_request", ""), "Q4_K_M");
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
    create_model(tmp.base, "llama-20b");
    create_model(tmp.base, "llama-7b");
    create_model(tmp.base, "qwen3-coder-30b");
    create_safetensors_model_with_index(tmp.base, "mistral-7b");

    ModelStorage storage(tmp.base.string());
    auto list = storage.listAvailable();

    ASSERT_EQ(list.size(), 4u);

    std::vector<std::string> names;
    for (const auto& m : list) {
        names.push_back(m.name);
    }
    std::sort(names.begin(), names.end());

    EXPECT_EQ(names[0], "llama-20b");
    EXPECT_EQ(names[1], "llama-7b");
    EXPECT_EQ(names[2], "mistral-7b");
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
    create_safetensors_model_with_index(tmp.base, "llama-30b-safetensors");

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor("llama-30b-safetensors");

    ASSERT_TRUE(desc.has_value());
    EXPECT_EQ(desc->runtime, "safetensors_cpp");
    EXPECT_EQ(desc->format, "safetensors");
    EXPECT_EQ(fs::path(desc->primary_path).filename(), "model.safetensors.index.json");
}

TEST(ModelStorageTest, ManifestFormatPrefersSafetensorsOverGguf) {
    TempModelDir tmp;
    const std::string model_name = "llama-20b";
    create_model(tmp.base, model_name);
    create_safetensors_model_with_index(tmp.base, model_name);
    write_manifest_with_format(tmp.base / model_name, "safetensors");

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor(model_name);

    ASSERT_TRUE(desc.has_value());
    EXPECT_EQ(desc->format, "safetensors");
    EXPECT_EQ(desc->runtime, "safetensors_cpp");

    auto list = storage.listAvailable();
    auto it = std::find_if(list.begin(), list.end(), [&](const ModelInfo& info) {
        return info.name == model_name;
    });
    ASSERT_TRUE(it != list.end());
    EXPECT_EQ(it->format, "safetensors");
}

TEST(ModelStorageTest, ManifestFormatPrefersGgufOverSafetensors) {
    TempModelDir tmp;
    const std::string model_name = "llama-7b";
    create_model(tmp.base, model_name);
    create_safetensors_model_with_index(tmp.base, model_name);
    write_manifest_with_format(tmp.base / model_name, "gguf");

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor(model_name);

    ASSERT_TRUE(desc.has_value());
    EXPECT_EQ(desc->format, "gguf");
    EXPECT_EQ(desc->runtime, "llama_cpp");

    auto list = storage.listAvailable();
    auto it = std::find_if(list.begin(), list.end(), [&](const ModelInfo& info) {
        return info.name == model_name;
    });
    ASSERT_TRUE(it != list.end());
    EXPECT_EQ(it->format, "gguf");
}

TEST(ModelStorageTest, ManifestFormatsPrefersFirstEntrySafetensors) {
    TempModelDir tmp;
    const std::string model_name = "llama-20b";
    create_model(tmp.base, model_name);
    create_safetensors_model_with_index(tmp.base, model_name);
    write_manifest_with_formats(tmp.base / model_name, {"safetensors", "gguf"});

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor(model_name);

    ASSERT_TRUE(desc.has_value());
    EXPECT_EQ(desc->format, "safetensors");
    EXPECT_EQ(desc->runtime, "safetensors_cpp");
    auto list = storage.listAvailable();
    auto it = std::find_if(list.begin(), list.end(), [&](const ModelInfo& info) {
        return info.name == model_name;
    });
    ASSERT_TRUE(it != list.end());
    EXPECT_EQ(it->format, "safetensors");
}

TEST(ModelStorageTest, ManifestFormatsPrefersFirstEntryGguf) {
    TempModelDir tmp;
    const std::string model_name = "llama-7b";
    create_model(tmp.base, model_name);
    create_safetensors_model_with_index(tmp.base, model_name);
    write_manifest_with_formats(tmp.base / model_name, {"gguf", "safetensors"});

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor(model_name);
    ASSERT_TRUE(desc.has_value());
    EXPECT_EQ(desc->format, "gguf");
    EXPECT_EQ(desc->runtime, "llama_cpp");
}

TEST(ModelStorageTest, ResolveDescriptorSelectsQuantizedGguf) {
    TempModelDir tmp;
    const std::string model_name = "llama-7b";
    auto model_dir = tmp.base / model_name;
    create_gguf_file(model_dir, "llama-7b.Q4_K_M.gguf");
    create_gguf_file(model_dir, "llama-7b.Q5_K_M.gguf");

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor("llama-7b:Q4_K_M");

    ASSERT_TRUE(desc.has_value());
    EXPECT_EQ(desc->format, "gguf");
    EXPECT_EQ(fs::path(desc->primary_path).filename(), "llama-7b.Q4_K_M.gguf");
}

TEST(ModelStorageTest, ResolveDescriptorRejectsUnknownQuantization) {
    TempModelDir tmp;
    const std::string model_name = "llama-7b";
    auto model_dir = tmp.base / model_name;
    create_gguf_file(model_dir, "llama-7b.Q4_K_M.gguf");
    create_gguf_file(model_dir, "llama-7b.Q5_K_M.gguf");

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor("llama-7b:Q6_K");

    EXPECT_FALSE(desc.has_value());
}

TEST(ModelStorageTest, ResolveDescriptorQuantizationIsCaseSensitive) {
    TempModelDir tmp;
    const std::string model_name = "llama-7b";
    auto model_dir = tmp.base / model_name;
    create_gguf_file(model_dir, "llama-7b.Q4_K_M.gguf");

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor("llama-7b:q4_k_m");

    EXPECT_FALSE(desc.has_value());
}

TEST(ModelStorageTest, ResolveDescriptorUsesManifestDefaultQuantization) {
    TempModelDir tmp;
    const std::string model_name = "llama-7b";
    auto model_dir = tmp.base / model_name;
    create_gguf_file(model_dir, "model.gguf");
    write_manifest_with_format_and_quantization(model_dir, "gguf", "Q4_K_M");

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor("llama-7b");

    ASSERT_TRUE(desc.has_value());
    EXPECT_EQ(fs::path(desc->primary_path).filename(), "model.gguf");

    auto quantized = storage.resolveDescriptor("llama-7b:Q4_K_M");
    ASSERT_TRUE(quantized.has_value());
    EXPECT_EQ(fs::path(quantized->primary_path).filename(), "model.gguf");
}

TEST(ModelStorageTest, ResolveDescriptorRejectsMismatchedManifestQuantization) {
    TempModelDir tmp;
    const std::string model_name = "llama-7b";
    auto model_dir = tmp.base / model_name;
    create_gguf_file(model_dir, "model.gguf");
    write_manifest_with_format_and_quantization(model_dir, "gguf", "Q4_K_M");

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor("llama-7b:Q5_K_M");
    EXPECT_FALSE(desc.has_value());
}

TEST(ModelStorageTest, ResolveDescriptorAllowsSafetensorsKvQuantization) {
    TempModelDir tmp;
    const std::string model_name = "llama-safetensors";
    create_safetensors_model_with_index(tmp.base, model_name);

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor(model_name + ":kv_int8");

    ASSERT_TRUE(desc.has_value());
    EXPECT_EQ(desc->runtime, "safetensors_cpp");
    EXPECT_EQ(desc->format, "safetensors");
    ASSERT_TRUE(desc->metadata.has_value());
    EXPECT_EQ(desc->metadata->value("quantization_request", ""), "kv_int8");
    EXPECT_EQ(desc->metadata->value("quantization", ""), "kv_int8");
}

TEST(ModelStorageTest, ResolveDescriptorRejectsUnsupportedSafetensorsKvQuantization) {
    TempModelDir tmp;
    const std::string model_name = "llama-safetensors";
    create_safetensors_model_with_index(tmp.base, model_name);

    ModelStorage storage(tmp.base.string());
    EXPECT_FALSE(storage.resolveDescriptor(model_name + ":kv_int4").has_value());
    EXPECT_FALSE(storage.validateModel(model_name + ":kv_int4"));
}

TEST(ModelStorageTest, ResolveDescriptorIncludesCapabilitiesForGguf) {
    TempModelDir tmp;
    create_model(tmp.base, "gpt-oss-7b");

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor("gpt-oss-7b");

    ASSERT_TRUE(desc.has_value());
    EXPECT_NE(std::find(desc->capabilities.begin(), desc->capabilities.end(), "text"),
              desc->capabilities.end());
    EXPECT_NE(std::find(desc->capabilities.begin(), desc->capabilities.end(), "embeddings"),
              desc->capabilities.end());
}

TEST(ModelStorageTest, ResolveDescriptorIncludesSafetensorsShardMetadata) {
    TempModelDir tmp;
    create_safetensors_model_with_shards(tmp.base, "llama-20b-sharded");

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor("llama-20b-sharded");

    ASSERT_TRUE(desc.has_value());
    ASSERT_TRUE(desc->metadata.has_value());
    auto meta = desc->metadata.value();
    ASSERT_TRUE(meta.contains("safetensors"));
    ASSERT_TRUE(meta["safetensors"].contains("index"));
    ASSERT_TRUE(meta["safetensors"].contains("shards"));
    EXPECT_EQ(meta["safetensors"]["index"], "model.safetensors.index.json");
    ASSERT_TRUE(meta["safetensors"]["shards"].is_array());
    EXPECT_EQ(meta["safetensors"]["shards"].size(), 2);
    EXPECT_EQ(meta["safetensors"]["shards"][0], "model-00001.safetensors");
    EXPECT_EQ(meta["safetensors"]["shards"][1], "model-00002.safetensors");
}

TEST(ModelStorageTest, ResolveDescriptorSkipsSafetensorsWhenMetadataMissing) {
    TempModelDir tmp;
    auto model_dir = tmp.base / "llama-30b";
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.safetensors.index.json") << R"({"weight_map":{}})";

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor("llama-30b");
    EXPECT_FALSE(desc.has_value());
}

// RED: index が存在しても shard が欠損している場合は無効とみなす
TEST(ModelStorageTest, ResolveDescriptorRejectsMissingShards) {
    TempModelDir tmp;
    create_safetensors_index_with_missing_shard(tmp.base, "llama-30b");

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor("llama-30b");
    EXPECT_FALSE(desc.has_value());
}

TEST(ModelStorageTest, ListAvailableDescriptorsIncludesGgufAndSafetensors) {
    TempModelDir tmp;
    create_model(tmp.base, "llama-20b");
    create_safetensors_model_with_index(tmp.base, "mistral-30b");

    ModelStorage storage(tmp.base.string());
    auto list = storage.listAvailableDescriptors();

    // gguf + safetensors
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

// =============================================================================
// SPEC-93536000: Architecture Auto-Detection Tests (Phase 7)
// =============================================================================

// 7.8 Unit Test: config.jsonからのアーキテクチャ検出
TEST(ModelStorageTest, ExtractArchitecturesFromConfigJson) {
    TempModelDir tmp;
    ModelStorage storage(tmp.base.string());

    // Test: Single architecture
    create_safetensors_model_with_architectures(tmp.base, "single-arch", {"LlamaForCausalLM"});
    auto desc = storage.resolveDescriptor("single-arch");
    ASSERT_TRUE(desc.has_value());
    ASSERT_EQ(desc->architectures.size(), 1u);
    EXPECT_EQ(desc->architectures[0], "llama");
}

TEST(ModelStorageTest, ExtractMultipleArchitecturesFromConfigJson) {
    TempModelDir tmp;
    ModelStorage storage(tmp.base.string());

    // Test: Multiple architectures
    create_safetensors_model_with_architectures(
        tmp.base,
        "multi-arch",
        {"LlamaForCausalLM", "MistralForCausalLM"});
    auto desc = storage.resolveDescriptor("multi-arch");
    ASSERT_TRUE(desc.has_value());
    ASSERT_EQ(desc->architectures.size(), 2u);
    // Architectures should be normalized
    EXPECT_EQ(desc->architectures[0], "llama");
    EXPECT_EQ(desc->architectures[1], "mistral");
}

TEST(ModelStorageTest, ExtractArchitecturesHandlesMissingConfigJson) {
    TempModelDir tmp;
    auto model_dir = tmp.base / "no-config";
    fs::create_directories(model_dir);
    // Only GGUF file, no config.json
    std::ofstream(model_dir / "model.gguf") << "dummy gguf";

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor("no-config");
    ASSERT_TRUE(desc.has_value());
    // GGUF without config.json should still work, architectures may be empty
    EXPECT_EQ(desc->format, "gguf");
}

TEST(ModelStorageTest, ExtractArchitecturesHandlesEmptyArchitecturesArray) {
    TempModelDir tmp;
    auto model_dir = tmp.base / "empty-arch";
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "config.json") << R"({"architectures":[]})";
    std::ofstream(model_dir / "tokenizer.json") << R"({"dummy":true})";
    std::ofstream(model_dir / "model.safetensors") << "dummy";

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor("empty-arch");
    ASSERT_TRUE(desc.has_value());
    EXPECT_TRUE(desc->architectures.empty());
}

TEST(ModelStorageTest, ExtractArchitecturesHandlesMalformedConfigJson) {
    TempModelDir tmp;
    auto model_dir = tmp.base / "malformed-config";
    fs::create_directories(model_dir);
    // Malformed JSON - architectures is not an array
    std::ofstream(model_dir / "config.json") << R"({"architectures":"not-an-array"})";
    std::ofstream(model_dir / "tokenizer.json") << R"({"dummy":true})";
    std::ofstream(model_dir / "model.safetensors") << "dummy";

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor("malformed-config");
    ASSERT_TRUE(desc.has_value());
    // Should gracefully handle and return empty architectures
    EXPECT_TRUE(desc->architectures.empty());
}

TEST(ModelStorageTest, ExtractArchitecturesSkipsNonStringValues) {
    TempModelDir tmp;
    auto model_dir = tmp.base / "mixed-types";
    fs::create_directories(model_dir);
    // JSON with mixed types in architectures array
    std::ofstream(model_dir / "config.json")
        << R"({"architectures":["LlamaForCausalLM", 123, null, "MistralForCausalLM"]})";
    std::ofstream(model_dir / "tokenizer.json") << R"({"dummy":true})";
    std::ofstream(model_dir / "model.safetensors") << "dummy";

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor("mixed-types");
    ASSERT_TRUE(desc.has_value());
    // Should only extract valid string architectures
    ASSERT_EQ(desc->architectures.size(), 2u);
    EXPECT_EQ(desc->architectures[0], "llama");
    EXPECT_EQ(desc->architectures[1], "mistral");
}

// 7.9 Unit Test: GGUFからのアーキテクチャ検出
// Note: GGUF architecture is read via llama_model_meta_val_str() which requires
// loading actual GGUF files with llama.cpp. This is tested at integration level.
TEST(ModelStorageTest, GgufModelWithoutConfigJsonUsesLlamaCpp) {
    TempModelDir tmp;
    create_model(tmp.base, "gguf-only");

    ModelStorage storage(tmp.base.string());
    auto desc = storage.resolveDescriptor("gguf-only");
    ASSERT_TRUE(desc.has_value());
    EXPECT_EQ(desc->runtime, "llama_cpp");
    EXPECT_EQ(desc->format, "gguf");
    // Architecture detection from GGUF metadata requires actual model loading
    // and is tested in integration tests
}

// 7.10 Unit Test: 未対応アーキテクチャの除外
TEST(ModelStorageTest, NormalizesUnknownArchitecturesToCompactForm) {
    TempModelDir tmp;
    ModelStorage storage(tmp.base.string());

    // Unknown architecture should be normalized to compact form (not excluded)
    // Note: The normalization only strips known suffixes for known families
    // For unknown architectures, the full class name is kept in lowercase
    create_safetensors_model_with_architectures(
        tmp.base,
        "unknown-arch",
        {"UnknownCustomForCausalLM"});
    auto desc = storage.resolveDescriptor("unknown-arch");
    ASSERT_TRUE(desc.has_value());
    ASSERT_EQ(desc->architectures.size(), 1u);
    // Unknown architectures are normalized to lowercase compact form
    EXPECT_EQ(desc->architectures[0], "unknowncustomforcausallm");
}

TEST(ModelStorageTest, NormalizesKnownArchitectureFamilies) {
    TempModelDir tmp;
    ModelStorage storage(tmp.base.string());

    // Test various known architecture families
    std::vector<std::pair<std::string, std::string>> test_cases = {
        {"Qwen2ForCausalLM", "qwen"},
        {"Qwen2_5ForCausalLM", "qwen"},
        {"LlamaForCausalLM", "llama"},
        {"MistralForCausalLM", "mistral"},
        {"GemmaForCausalLM", "gemma"},
        {"Phi3ForCausalLM", "phi"},
        {"DeepseekV3ForCausalLM", "deepseek"},
        {"GptOssForCausalLM", "gptoss"},
        {"ChatGLM4ForCausalLM", "glm"},
        {"GraniteForCausalLM", "granite"},
        {"SmolLMForCausalLM", "smollm"},
        {"NemotronForCausalLM", "nemotron"},
    };

    for (size_t i = 0; i < test_cases.size(); ++i) {
        const auto& [input, expected] = test_cases[i];
        std::string model_name = "arch-test-" + std::to_string(i);
        create_safetensors_model_with_architectures(tmp.base, model_name, {input});
        auto desc = storage.resolveDescriptor(model_name);
        ASSERT_TRUE(desc.has_value()) << "Failed for: " << input;
        ASSERT_EQ(desc->architectures.size(), 1u) << "Failed for: " << input;
        EXPECT_EQ(desc->architectures[0], expected)
            << "Input: " << input << ", Expected: " << expected
            << ", Got: " << desc->architectures[0];
    }
}

TEST(ModelStorageTest, DeduplicatesArchitectures) {
    TempModelDir tmp;
    ModelStorage storage(tmp.base.string());

    // Same architecture appearing multiple times should be deduplicated
    create_safetensors_model_with_architectures(
        tmp.base,
        "duplicate-arch",
        {"LlamaForCausalLM", "LLaMAForCausalLM", "LlamaModel"});
    auto desc = storage.resolveDescriptor("duplicate-arch");
    ASSERT_TRUE(desc.has_value());
    // All should normalize to "llama" and be deduplicated
    ASSERT_EQ(desc->architectures.size(), 1u);
    EXPECT_EQ(desc->architectures[0], "llama");
}

// 7.11 Integration Test: 任意のHuggingFaceモデルの自動認識
TEST(ModelStorageTest, ListAvailableIncludesArchitectures) {
    TempModelDir tmp;

    // Create models with different architectures
    create_safetensors_model_with_architectures(tmp.base, "llama-model", {"LlamaForCausalLM"});
    create_safetensors_model_with_architectures(tmp.base, "qwen-model", {"Qwen2ForCausalLM"});
    create_model(tmp.base, "gguf-model");  // GGUF without config.json

    ModelStorage storage(tmp.base.string());
    auto list = storage.listAvailable();

    ASSERT_EQ(list.size(), 3u);

    // Verify models are listed
    std::vector<std::string> names;
    for (const auto& m : list) {
        names.push_back(m.name);
    }
    std::sort(names.begin(), names.end());

    EXPECT_EQ(names[0], "gguf-model");
    EXPECT_EQ(names[1], "llama-model");
    EXPECT_EQ(names[2], "qwen-model");
}

TEST(ModelStorageTest, ListAvailableDescriptorsIncludesArchitectureInfo) {
    TempModelDir tmp;

    // Create a model with architecture info
    create_safetensors_model_with_architectures(tmp.base, "arch-model", {"MistralForCausalLM"});

    ModelStorage storage(tmp.base.string());
    auto descriptors = storage.listAvailableDescriptors();

    ASSERT_EQ(descriptors.size(), 1u);
    EXPECT_EQ(descriptors[0].name, "arch-model");
    ASSERT_EQ(descriptors[0].architectures.size(), 1u);
    EXPECT_EQ(descriptors[0].architectures[0], "mistral");
}
