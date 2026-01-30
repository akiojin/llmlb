#include <gtest/gtest.h>

#include <filesystem>
#include <fstream>
#include <vector>
#include <cstdlib>

#include "core/lora_utils.h"

namespace fs = std::filesystem;

using namespace xllm;

namespace {
class TempDir {
public:
    TempDir() {
        auto base = fs::temp_directory_path() / fs::path("lora-utils-XXXXXX");
        std::string tmpl = base.string();
        std::vector<char> buf(tmpl.begin(), tmpl.end());
        buf.push_back('\0');
        char* created = mkdtemp(buf.data());
        path = created ? fs::path(created) : fs::temp_directory_path();
    }

    ~TempDir() {
        std::error_code ec;
        fs::remove_all(path, ec);
    }

    fs::path path;
};

void touch_file(const fs::path& file_path) {
    std::ofstream ofs(file_path);
    ofs << "dummy";
}
}  // namespace

TEST(LoraUtilsTest, ResolvesNameRelativeToBaseDir) {
    TempDir temp;
    fs::path lora_file = temp.path / "adapter.gguf";
    touch_file(lora_file);

    LoraRequest req;
    req.name = "adapter.gguf";
    req.scale = 0.5f;

    std::string error;
    auto resolved = resolve_lora_requests({req}, temp.path.string(), error);

    ASSERT_TRUE(error.empty());
    ASSERT_EQ(resolved.size(), 1u);
    EXPECT_EQ(resolved[0].name, "adapter.gguf");
    EXPECT_EQ(fs::path(resolved[0].path), fs::absolute(lora_file));
    EXPECT_FLOAT_EQ(resolved[0].scale, 0.5f);
}

TEST(LoraUtilsTest, ResolvesAbsolutePathAndInfersName) {
    TempDir temp;
    fs::path lora_file = temp.path / "custom-lora.gguf";
    touch_file(lora_file);

    LoraRequest req;
    req.path = lora_file.string();

    std::string error;
    auto resolved = resolve_lora_requests({req}, temp.path.string(), error);

    ASSERT_TRUE(error.empty());
    ASSERT_EQ(resolved.size(), 1u);
    EXPECT_EQ(resolved[0].name, "custom-lora.gguf");
    EXPECT_EQ(fs::path(resolved[0].path), fs::absolute(lora_file));
    EXPECT_FLOAT_EQ(resolved[0].scale, 1.0f);
}

TEST(LoraUtilsTest, ReturnsErrorWhenFileMissing) {
    TempDir temp;

    LoraRequest req;
    req.name = "missing.gguf";

    std::string error;
    auto resolved = resolve_lora_requests({req}, temp.path.string(), error);

    EXPECT_TRUE(resolved.empty());
    EXPECT_FALSE(error.empty());
}
