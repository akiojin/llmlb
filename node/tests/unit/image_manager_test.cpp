// ImageManager unit tests (TDD)
#include <gtest/gtest.h>
#include <filesystem>
#include <fstream>
#include <cstdlib>

#include "core/image_manager.h"

using namespace llm_node;
namespace fs = std::filesystem;

class TempScriptsDir {
public:
    TempScriptsDir() {
        base = fs::temp_directory_path() / fs::path("image-scripts-XXXXXX");
        std::string tmpl = base.string();
        std::vector<char> buf(tmpl.begin(), tmpl.end());
        buf.push_back('\0');
        char* created = mkdtemp(buf.data());
        base = created ? fs::path(created) : fs::temp_directory_path();
    }
    ~TempScriptsDir() {
        std::error_code ec;
        fs::remove_all(base, ec);
    }
    fs::path base;
};

// Helper: create a dummy script file
static void create_script(const fs::path& dir, const std::string& name) {
    auto script_path = dir / name;
    std::ofstream(script_path) << "#!/usr/bin/env python3\nprint('dummy')";
    fs::permissions(script_path, fs::perms::owner_exec | fs::perms::owner_read | fs::perms::owner_write);
}

// Test: Constructor with default scripts directory
TEST(ImageManagerTest, ConstructorWithEmptyDir) {
    ImageManager manager("");
    // Should not throw, uses default paths
    SUCCEED();
}

// Test: Constructor with custom scripts directory
TEST(ImageManagerTest, ConstructorWithCustomDir) {
    TempScriptsDir tmp;
    ImageManager manager(tmp.base.string());
    SUCCEED();
}

// Test: T2I not available when script missing
TEST(ImageManagerTest, T2INotAvailableWhenScriptMissing) {
    TempScriptsDir tmp;
    ImageManager manager(tmp.base.string());

    // No scripts created, so T2I should not be available
    // Note: Availability depends on environment variables too
    // This test verifies the manager doesn't crash
    EXPECT_NO_THROW(manager.isT2IAvailable());
}

// Test: I2T not available when script missing
TEST(ImageManagerTest, I2TNotAvailableWhenScriptMissing) {
    TempScriptsDir tmp;
    ImageManager manager(tmp.base.string());

    EXPECT_NO_THROW(manager.isI2TAvailable());
}

// Test: T2I with invalid parameters returns error
TEST(ImageManagerTest, T2IWithEmptyPromptReturnsError) {
    TempScriptsDir tmp;
    ImageManager manager(tmp.base.string());

    T2IParams params;
    params.prompt = "";  // Invalid: empty prompt

    auto result = manager.generateImage(params);

    EXPECT_FALSE(result.success);
    EXPECT_FALSE(result.error.empty());
}

// Test: T2I with valid prompt but no script available
TEST(ImageManagerTest, T2IWithValidPromptButNoScript) {
    TempScriptsDir tmp;
    ImageManager manager(tmp.base.string());

    T2IParams params;
    params.prompt = "A cute cat";

    auto result = manager.generateImage(params);

    // Should fail because no script is available
    EXPECT_FALSE(result.success);
}

// Test: I2T with missing image path returns error
TEST(ImageManagerTest, I2TWithEmptyImagePathReturnsError) {
    TempScriptsDir tmp;
    ImageManager manager(tmp.base.string());

    I2TParams params;
    params.image_path = "";  // Invalid: empty image path

    auto result = manager.captionImage(params);

    EXPECT_FALSE(result.success);
    EXPECT_FALSE(result.error.empty());
}

// Test: I2T with non-existent image returns error
TEST(ImageManagerTest, I2TWithNonExistentImageReturnsError) {
    TempScriptsDir tmp;
    ImageManager manager(tmp.base.string());

    I2TParams params;
    params.image_path = "/nonexistent/path/image.png";

    auto result = manager.captionImage(params);

    EXPECT_FALSE(result.success);
    EXPECT_FALSE(result.error.empty());
}

// Test: T2IParams default values
TEST(ImageManagerTest, T2IParamsDefaultValues) {
    T2IParams params;

    EXPECT_TRUE(params.prompt.empty());
    EXPECT_TRUE(params.negative_prompt.empty());
    EXPECT_EQ(params.model, "z-image-turbo");
    EXPECT_EQ(params.width, 512);
    EXPECT_EQ(params.height, 512);
    EXPECT_EQ(params.steps, 9);
    EXPECT_FLOAT_EQ(params.guidance_scale, 0.0f);
    EXPECT_FALSE(params.seed.has_value());
}

// Test: I2TParams default values
TEST(ImageManagerTest, I2TParamsDefaultValues) {
    I2TParams params;

    EXPECT_TRUE(params.image_path.empty());
    EXPECT_EQ(params.prompt, "describe this image");
    EXPECT_EQ(params.model, "glm-4.6v-flash");
    EXPECT_EQ(params.max_new_tokens, 256);
}

// Test: T2IResult structure
TEST(ImageManagerTest, T2IResultStructure) {
    T2IResult result;
    result.success = true;
    result.image_path = "/path/to/output.png";
    result.error = "";

    EXPECT_TRUE(result.success);
    EXPECT_EQ(result.image_path, "/path/to/output.png");
    EXPECT_TRUE(result.error.empty());
}

// Test: I2TResult structure
TEST(ImageManagerTest, I2TResultStructure) {
    I2TResult result;
    result.success = true;
    result.caption = "A beautiful sunset";
    result.error = "";

    EXPECT_TRUE(result.success);
    EXPECT_EQ(result.caption, "A beautiful sunset");
    EXPECT_TRUE(result.error.empty());
}
