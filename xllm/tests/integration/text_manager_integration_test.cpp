#include <gtest/gtest.h>

#include <algorithm>
#include <filesystem>
#include <string>
#include <vector>

#include "core/inference_engine.h"
#include "core/llama_manager.h"
#include "models/model_storage.h"

namespace fs = std::filesystem;

namespace {

class TempDir {
public:
    TempDir() {
        auto base = fs::temp_directory_path() / fs::path("text-manager-it-XXXXXX");
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

bool contains(const std::vector<std::string>& values, const std::string& needle) {
    return std::find(values.begin(), values.end(), needle) != values.end();
}

}  // namespace

using xllm::InferenceEngine;
using xllm::LlamaManager;
using xllm::ModelStorage;

TEST(TextManagerIntegrationTest, InferenceEngineReportsBuiltInRuntimes) {
    TempDir tmp;
    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage, nullptr, nullptr);

    auto runtimes = engine.getRegisteredRuntimes();
    EXPECT_TRUE(contains(runtimes, "llama_cpp"));
}
