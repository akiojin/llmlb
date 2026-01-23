// SPEC-58378000: OllamaCompat unit tests

#include <gtest/gtest.h>
#include "cli/ollama_compat.h"
#include <filesystem>
#include <fstream>

using namespace xllm::cli;
namespace fs = std::filesystem;

class OllamaCompatTest : public ::testing::Test {
protected:
    void SetUp() override {
        // Create temp directory for tests
        test_dir_ = fs::temp_directory_path() / "ollama_compat_test";
        fs::create_directories(test_dir_);
    }

    void TearDown() override {
        // Clean up temp directory
        fs::remove_all(test_dir_);
    }

    fs::path test_dir_;
};

// Test hasOllamaPrefix static method
TEST(OllamaCompatStaticTest, HasOllamaPrefixTrue) {
    EXPECT_TRUE(OllamaCompat::hasOllamaPrefix("ollama:llama3.2"));
    EXPECT_TRUE(OllamaCompat::hasOllamaPrefix("ollama:mistral:latest"));
}

TEST(OllamaCompatStaticTest, HasOllamaPrefixFalse) {
    EXPECT_FALSE(OllamaCompat::hasOllamaPrefix("llama3.2"));
    EXPECT_FALSE(OllamaCompat::hasOllamaPrefix("mistral:latest"));
    EXPECT_FALSE(OllamaCompat::hasOllamaPrefix("ollam:typo"));
    EXPECT_FALSE(OllamaCompat::hasOllamaPrefix(""));
}

// Test stripOllamaPrefix static method
TEST(OllamaCompatStaticTest, StripOllamaPrefixWithPrefix) {
    EXPECT_EQ(OllamaCompat::stripOllamaPrefix("ollama:llama3.2"), "llama3.2");
    EXPECT_EQ(OllamaCompat::stripOllamaPrefix("ollama:mistral:latest"), "mistral:latest");
}

TEST(OllamaCompatStaticTest, StripOllamaPrefixWithoutPrefix) {
    EXPECT_EQ(OllamaCompat::stripOllamaPrefix("llama3.2"), "llama3.2");
    EXPECT_EQ(OllamaCompat::stripOllamaPrefix("mistral:latest"), "mistral:latest");
}

// Test instance methods with non-existent directory
TEST_F(OllamaCompatTest, NonExistentDirectory) {
    OllamaCompat compat("/nonexistent/path");
    EXPECT_FALSE(compat.isAvailable());
}

TEST_F(OllamaCompatTest, EmptyDirectory) {
    OllamaCompat compat(test_dir_.string());
    EXPECT_TRUE(compat.isAvailable());  // Directory exists but is empty
}

TEST_F(OllamaCompatTest, ListModelsEmptyDirectory) {
    OllamaCompat compat(test_dir_.string());
    auto models = compat.listModels();
    EXPECT_TRUE(models.empty());
}

TEST_F(OllamaCompatTest, GetModelNotFound) {
    OllamaCompat compat(test_dir_.string());
    auto model = compat.getModel("llama3.2");
    EXPECT_FALSE(model.has_value());
}

TEST_F(OllamaCompatTest, ResolveBlobPathNotFound) {
    OllamaCompat compat(test_dir_.string());
    auto path = compat.resolveBlobPath("llama3.2");
    EXPECT_TRUE(path.empty());
}

// Test with mock ollama directory structure
TEST_F(OllamaCompatTest, ParseValidManifest) {
    // Create mock ollama directory structure
    fs::path manifests_dir = test_dir_ / "manifests" / "registry.ollama.ai" / "library" / "llama3";
    fs::path blobs_dir = test_dir_ / "blobs";
    fs::create_directories(manifests_dir);
    fs::create_directories(blobs_dir);

    // Create manifest file - use smaller size that fits in JSON integer parsing
    std::string digest = "abc123def456";
    std::string manifest_content = R"({
        "schemaVersion": 2,
        "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
        "layers": [
            {
                "mediaType": "application/vnd.ollama.image.model",
                "digest": "sha256:)" + digest + R"(",
                "size": 123456789
            }
        ]
    })";

    std::ofstream manifest_file(manifests_dir / "latest");
    manifest_file << manifest_content;
    manifest_file.close();

    // Create blob file
    std::ofstream blob_file(blobs_dir / ("sha256-" + digest));
    blob_file << "mock model data";
    blob_file.close();

    // Test
    OllamaCompat compat(test_dir_.string());
    EXPECT_TRUE(compat.isAvailable());

    auto models = compat.listModels();
    EXPECT_EQ(models.size(), 1);
    if (!models.empty()) {
        EXPECT_EQ(models[0].name, "llama3:latest");
        EXPECT_EQ(models[0].size_bytes, 123456789ULL);
        EXPECT_TRUE(models[0].readonly);
    }
}

TEST_F(OllamaCompatTest, GetModelByName) {
    // Create mock ollama directory structure
    fs::path manifests_dir = test_dir_ / "manifests" / "registry.ollama.ai" / "library" / "mistral";
    fs::path blobs_dir = test_dir_ / "blobs";
    fs::create_directories(manifests_dir);
    fs::create_directories(blobs_dir);

    std::string digest = "xyz789abc012";
    std::string manifest_content = R"({
        "schemaVersion": 2,
        "layers": [
            {
                "mediaType": "application/vnd.ollama.image.model",
                "digest": "sha256:)" + digest + R"(",
                "size": 987654321
            }
        ]
    })";

    std::ofstream manifest_file(manifests_dir / "7b");
    manifest_file << manifest_content;
    manifest_file.close();

    std::ofstream blob_file(blobs_dir / ("sha256-" + digest));
    blob_file << "mock model data";
    blob_file.close();

    // Test
    OllamaCompat compat(test_dir_.string());
    auto model = compat.getModel("mistral:7b");
    EXPECT_TRUE(model.has_value());
    if (model) {
        EXPECT_EQ(model->name, "mistral:7b");
        EXPECT_EQ(model->size_bytes, 987654321ULL);
        EXPECT_TRUE(model->readonly);
    }
}

TEST_F(OllamaCompatTest, GetModelDefaultTag) {
    // Create mock ollama directory structure
    fs::path manifests_dir = test_dir_ / "manifests" / "registry.ollama.ai" / "library" / "phi3";
    fs::path blobs_dir = test_dir_ / "blobs";
    fs::create_directories(manifests_dir);
    fs::create_directories(blobs_dir);

    std::string digest = "phi3digest";
    std::string manifest_content = R"({
        "schemaVersion": 2,
        "layers": [
            {
                "mediaType": "application/vnd.ollama.image.model",
                "digest": "sha256:)" + digest + R"(",
                "size": 2300000000
            }
        ]
    })";

    std::ofstream manifest_file(manifests_dir / "latest");
    manifest_file << manifest_content;
    manifest_file.close();

    std::ofstream blob_file(blobs_dir / ("sha256-" + digest));
    blob_file << "mock model data";
    blob_file.close();

    // Test - should find model with default "latest" tag
    OllamaCompat compat(test_dir_.string());
    auto model = compat.getModel("phi3");  // No tag specified
    EXPECT_TRUE(model.has_value());
    if (model) {
        EXPECT_EQ(model->name, "phi3");
    }
}

TEST_F(OllamaCompatTest, ResolveBlobPath) {
    // Create mock ollama directory structure
    fs::path manifests_dir = test_dir_ / "manifests" / "registry.ollama.ai" / "library" / "gemma";
    fs::path blobs_dir = test_dir_ / "blobs";
    fs::create_directories(manifests_dir);
    fs::create_directories(blobs_dir);

    std::string digest = "gemmadigest123";
    std::string manifest_content = R"({
        "layers": [
            {
                "mediaType": "application/vnd.ollama.image.model",
                "digest": "sha256:)" + digest + R"(",
                "size": 5600000000
            }
        ]
    })";

    std::ofstream manifest_file(manifests_dir / "latest");
    manifest_file << manifest_content;
    manifest_file.close();

    fs::path blob_path = blobs_dir / ("sha256-" + digest);
    std::ofstream blob_file(blob_path);
    blob_file << "mock model data";
    blob_file.close();

    // Test
    OllamaCompat compat(test_dir_.string());
    auto resolved_path = compat.resolveBlobPath("gemma");
    EXPECT_FALSE(resolved_path.empty());
    EXPECT_EQ(resolved_path, blob_path.string());
}
