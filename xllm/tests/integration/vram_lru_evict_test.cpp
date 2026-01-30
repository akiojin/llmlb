#include <gtest/gtest.h>

#include <chrono>
#include <filesystem>
#include <fstream>
#include <string>
#include <vector>

#include "core/llama_manager.h"

namespace fs = std::filesystem;

namespace {

class TempModelDir {
public:
    TempModelDir() {
        char tmpl[] = "/tmp/vram-lru-XXXXXX";
        if (char* result = mkdtemp(tmpl)) {
            path_ = result;
        }
    }

    ~TempModelDir() {
        if (!path_.empty()) {
            std::error_code ec;
            fs::remove_all(path_, ec);
        }
    }

    const std::string& path() const { return path_; }

    std::string createDummyModel(const std::string& name) const {
        const fs::path p = fs::path(path_) / (name + ".gguf");
        fs::create_directories(p.parent_path());
        std::ofstream ofs(p);
        ofs << "GGUF";
        return p.string();
    }

private:
    std::string path_;
};

bool containsPathSuffix(const std::vector<std::string>& paths, const std::string& suffix) {
    for (const auto& p : paths) {
        if (p.size() >= suffix.size() && p.compare(p.size() - suffix.size(), suffix.size(), suffix) == 0) {
            return true;
        }
    }
    return false;
}

}  // namespace

// =============================================================================
// T209: 複数モデルロード→VRAM枯渇→LRUアンロードの統合テスト
// =============================================================================

TEST(VramLruEvictIntegrationTest, EvictsLeastRecentlyUsedNonActiveModelsUntilBudgetMet) {
    TempModelDir tmp;
    xllm::LlamaManager manager(tmp.path());

    const std::string model_a = tmp.createDummyModel("model-a");
    const std::string model_b = tmp.createDummyModel("model-b");
    const std::string model_c = tmp.createDummyModel("model-c");

    const auto now = std::chrono::steady_clock::now();

    // 疑似ロード（サイズはテスト用に明示）
    manager.addLoadedModelForTest(model_a, /*model_size_bytes=*/400, now - std::chrono::minutes(3));
    manager.addLoadedModelForTest(model_b, /*model_size_bytes=*/300, now - std::chrono::minutes(2));
    manager.addLoadedModelForTest(model_c, /*model_size_bytes=*/200, now - std::chrono::minutes(1));

    EXPECT_EQ(manager.memoryUsageBytes(), 900u);
    EXPECT_EQ(manager.loadedCount(), 3u);

    // 推論中のモデルはLRU対象から除外
    manager.markAsActive(model_b);

    // 500B分のVRAMを確保するためにLRU evictionを実行
    const size_t freed = manager.evictForVram(/*required_vram=*/500);

    // model-a (400) + model-c (200) が候補になり、合計600以上解放される想定
    EXPECT_GE(freed, 500u);

    const auto loaded = manager.getLoadedModels();
    EXPECT_FALSE(containsPathSuffix(loaded, "model-a.gguf"));
    EXPECT_TRUE(containsPathSuffix(loaded, "model-b.gguf"));
    EXPECT_FALSE(containsPathSuffix(loaded, "model-c.gguf"));

    // アクティブなmodel-bのみ残る
    EXPECT_EQ(manager.loadedCount(), 1u);
    EXPECT_EQ(manager.memoryUsageBytes(), 300u);

    manager.markAsInactive(model_b);
}
