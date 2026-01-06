// T163/T175: Vision mmproj auto-detection tests
#include <gtest/gtest.h>
#include <filesystem>
#include <fstream>

#include "core/vision_processor.h"

using namespace llm_node;
namespace fs = std::filesystem;

class TempMmprojDir {
public:
    TempMmprojDir() {
        base = fs::temp_directory_path() / fs::path("mmproj-test-XXXXXX");
        std::string tmpl = base.string();
        std::vector<char> buf(tmpl.begin(), tmpl.end());
        buf.push_back('\0');
        char* created = mkdtemp(buf.data());
        base = created ? fs::path(created) : fs::temp_directory_path();
    }
    ~TempMmprojDir() {
        std::error_code ec;
        fs::remove_all(base, ec);
    }
    fs::path base;
};

static void createFile(const fs::path& path, const std::string& content = "dummy") {
    std::ofstream(path) << content;
}

// T175: mmproj自動検出テスト

TEST(FindMmprojInDirectoryTest, EmptyDirectoryReturnsNullopt) {
    TempMmprojDir temp;
    auto result = findMmprojInDirectory(temp.base.string());
    EXPECT_FALSE(result.has_value());
}

TEST(FindMmprojInDirectoryTest, NonexistentDirectoryReturnsNullopt) {
    auto result = findMmprojInDirectory("/nonexistent/path/12345");
    EXPECT_FALSE(result.has_value());
}

TEST(FindMmprojInDirectoryTest, DirectoryWithOnlyModelGgufReturnsNullopt) {
    TempMmprojDir temp;
    createFile(temp.base / "model.gguf");
    createFile(temp.base / "config.json");

    auto result = findMmprojInDirectory(temp.base.string());
    EXPECT_FALSE(result.has_value());
}

TEST(FindMmprojInDirectoryTest, FindsSingleMmprojFile) {
    TempMmprojDir temp;
    createFile(temp.base / "model.gguf");
    createFile(temp.base / "mmproj-model-f16.gguf");

    auto result = findMmprojInDirectory(temp.base.string());
    ASSERT_TRUE(result.has_value());
    EXPECT_TRUE(result->find("mmproj") != std::string::npos);
}

TEST(FindMmprojInDirectoryTest, FindsMmprojWithUpperCase) {
    TempMmprojDir temp;
    createFile(temp.base / "model.gguf");
    createFile(temp.base / "MMPROJ-model-f16.gguf");

    auto result = findMmprojInDirectory(temp.base.string());
    ASSERT_TRUE(result.has_value());
    EXPECT_TRUE(result->find("MMPROJ") != std::string::npos);
}

TEST(FindMmprojInDirectoryTest, FindsMmprojWithMixedCase) {
    TempMmprojDir temp;
    createFile(temp.base / "model.gguf");
    createFile(temp.base / "MmProj-model.gguf");

    auto result = findMmprojInDirectory(temp.base.string());
    ASSERT_TRUE(result.has_value());
    EXPECT_TRUE(result->find("MmProj") != std::string::npos);
}

TEST(FindMmprojInDirectoryTest, SelectsFirstAlphabetically) {
    TempMmprojDir temp;
    createFile(temp.base / "model.gguf");
    createFile(temp.base / "mmproj-b.gguf");
    createFile(temp.base / "mmproj-a.gguf");
    createFile(temp.base / "mmproj-c.gguf");

    auto result = findMmprojInDirectory(temp.base.string());
    ASSERT_TRUE(result.has_value());
    EXPECT_TRUE(result->find("mmproj-a.gguf") != std::string::npos);
}

TEST(FindMmprojInDirectoryTest, IgnoresNonGgufFiles) {
    TempMmprojDir temp;
    createFile(temp.base / "model.gguf");
    createFile(temp.base / "mmproj.bin");
    createFile(temp.base / "mmproj.safetensors");

    auto result = findMmprojInDirectory(temp.base.string());
    EXPECT_FALSE(result.has_value());
}

TEST(FindMmprojInDirectoryTest, IgnoresDirectories) {
    TempMmprojDir temp;
    createFile(temp.base / "model.gguf");
    fs::create_directory(temp.base / "mmproj.gguf");

    auto result = findMmprojInDirectory(temp.base.string());
    EXPECT_FALSE(result.has_value());
}

TEST(FindMmprojInDirectoryTest, HandlesVisionModelWithMmproj) {
    TempMmprojDir temp;
    createFile(temp.base / "llava-v1.6-mistral-7b.Q4_K_M.gguf");
    createFile(temp.base / "mmproj-llava-v1.6-mistral-7b-f16.gguf");
    createFile(temp.base / "config.json", R"({"architectures":["LlavaForConditionalGeneration"]})");

    auto result = findMmprojInDirectory(temp.base.string());
    ASSERT_TRUE(result.has_value());
    EXPECT_TRUE(result->find("mmproj-llava") != std::string::npos);
}

TEST(FindMmprojInDirectoryTest, FindsMmprojWithDifferentNaming) {
    TempMmprojDir temp;
    createFile(temp.base / "model.gguf");
    // Some models use different naming conventions
    createFile(temp.base / "vision-mmproj-encoder.gguf");

    auto result = findMmprojInDirectory(temp.base.string());
    ASSERT_TRUE(result.has_value());
    EXPECT_TRUE(result->find("mmproj") != std::string::npos);
}
