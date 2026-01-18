// SPEC-d7feaa2c: T175 mmproj auto-detection tests
#include <gtest/gtest.h>

#include <filesystem>
#include <fstream>

#include "core/vision_processor.h"
#include "models/model_storage.h"

namespace fs = std::filesystem;

namespace allm {

class TempVisionDir {
public:
    TempVisionDir() {
        base = fs::temp_directory_path() / fs::path("vision-processor-XXXXXX");
        std::string tmpl = base.string();
        std::vector<char> buf(tmpl.begin(), tmpl.end());
        buf.push_back('\0');
        char* created = mkdtemp(buf.data());
        base = created ? fs::path(created) : fs::temp_directory_path();
    }
    ~TempVisionDir() {
        std::error_code ec;
        fs::remove_all(base, ec);
    }
    fs::path base;
};

// Test fixture with friend access to VisionProcessor
class VisionProcessorTest : public ::testing::Test {
protected:
    std::unique_ptr<TempVisionDir> temp_dir_;
    std::unique_ptr<ModelStorage> storage_;
    std::unique_ptr<VisionProcessor> processor_;

    void SetUp() override {
        temp_dir_ = std::make_unique<TempVisionDir>();
        storage_ = std::make_unique<ModelStorage>(temp_dir_->base.string());
        processor_ = std::make_unique<VisionProcessor>(*storage_);
    }

    void TearDown() override {
        processor_.reset();
        storage_.reset();
        temp_dir_.reset();
    }

    // Helper to access private resolveMmprojPath
    std::optional<std::string> resolveMmprojPath(const std::string& model_name,
                                                  const std::string& model_path) {
        return processor_->resolveMmprojPath(model_name, model_path);
    }

    // Helper to create model directory with GGUF
    fs::path createModelDir(const std::string& name) {
        auto model_dir = temp_dir_->base / name;
        fs::create_directories(model_dir);
        std::ofstream(model_dir / "model.gguf") << "dummy gguf";
        return model_dir;
    }

    // Helper to create mmproj file
    void createMmprojFile(const fs::path& model_dir, const std::string& filename) {
        std::ofstream(model_dir / filename) << "dummy mmproj";
    }

    // Helper to create metadata JSON with mmproj_path
    void createMetadataWithMmproj(const fs::path& model_dir, const std::string& mmproj_path) {
        std::ofstream(model_dir / "metadata.json")
            << R"({"mmproj_path":")" << mmproj_path << R"("})";
    }
};

// T175: mmproj file detected in model directory by filename pattern
TEST_F(VisionProcessorTest, DetectsMmprojByFilename) {
    auto model_dir = createModelDir("test-vision-model");
    createMmprojFile(model_dir, "mmproj-model-f16.gguf");

    auto model_path = (model_dir / "model.gguf").string();
    auto result = resolveMmprojPath("test-vision-model", model_path);

    ASSERT_TRUE(result.has_value());
    EXPECT_TRUE(result->find("mmproj") != std::string::npos);
    EXPECT_TRUE(result->find(".gguf") != std::string::npos);
}

// T175: mmproj file detected with various naming conventions
TEST_F(VisionProcessorTest, DetectsMmprojWithVariousNamingConventions) {
    auto model_dir = createModelDir("vision-model-2");
    createMmprojFile(model_dir, "model-mmproj-q8_0.gguf");

    auto model_path = (model_dir / "model.gguf").string();
    auto result = resolveMmprojPath("vision-model-2", model_path);

    ASSERT_TRUE(result.has_value());
    EXPECT_TRUE(result->find("mmproj") != std::string::npos);
}

// T175: mmproj file detected case-insensitively
TEST_F(VisionProcessorTest, DetectsMmprojCaseInsensitive) {
    auto model_dir = createModelDir("vision-model-upper");
    createMmprojFile(model_dir, "MMPROJ-f16.gguf");

    auto model_path = (model_dir / "model.gguf").string();
    auto result = resolveMmprojPath("vision-model-upper", model_path);

    ASSERT_TRUE(result.has_value());
    // Should find the file despite uppercase
    EXPECT_TRUE(fs::exists(*result));
}

// T175: Returns nullopt when no mmproj file exists
TEST_F(VisionProcessorTest, ReturnsNulloptWhenNoMmproj) {
    auto model_dir = createModelDir("non-vision-model");
    // No mmproj file created

    auto model_path = (model_dir / "model.gguf").string();
    auto result = resolveMmprojPath("non-vision-model", model_path);

    EXPECT_FALSE(result.has_value());
}

// T175: Selects first mmproj alphabetically when multiple exist
TEST_F(VisionProcessorTest, SelectsFirstMmprojAlphabetically) {
    auto model_dir = createModelDir("multi-mmproj-model");
    createMmprojFile(model_dir, "mmproj-b-f16.gguf");
    createMmprojFile(model_dir, "mmproj-a-f32.gguf");
    createMmprojFile(model_dir, "mmproj-c-q4.gguf");

    auto model_path = (model_dir / "model.gguf").string();
    auto result = resolveMmprojPath("multi-mmproj-model", model_path);

    ASSERT_TRUE(result.has_value());
    // Should select "mmproj-a-f32.gguf" as it comes first alphabetically
    EXPECT_TRUE(result->find("mmproj-a") != std::string::npos);
}

// T175: Ignores non-gguf files with mmproj in name
TEST_F(VisionProcessorTest, IgnoresNonGgufFiles) {
    auto model_dir = createModelDir("mixed-files-model");
    // Create non-gguf files with mmproj in name
    std::ofstream(model_dir / "mmproj.txt") << "not a gguf";
    std::ofstream(model_dir / "mmproj.bin") << "not a gguf";

    auto model_path = (model_dir / "model.gguf").string();
    auto result = resolveMmprojPath("mixed-files-model", model_path);

    EXPECT_FALSE(result.has_value());
}

// T175: Ignores gguf files without mmproj in name
TEST_F(VisionProcessorTest, IgnoresNonMmprojGgufFiles) {
    auto model_dir = createModelDir("other-gguf-model");
    createMmprojFile(model_dir, "adapter-lora.gguf");
    createMmprojFile(model_dir, "extra-weights.gguf");

    auto model_path = (model_dir / "model.gguf").string();
    auto result = resolveMmprojPath("other-gguf-model", model_path);

    EXPECT_FALSE(result.has_value());
}

}  // namespace allm
