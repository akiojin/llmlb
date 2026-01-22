#include <gtest/gtest.h>
#include <filesystem>
#include <fstream>

#include "core/model_pool.h"

using namespace xllm;
namespace fs = std::filesystem;

class TempModelPoolDir {
public:
    TempModelPoolDir() {
        base = fs::temp_directory_path() / fs::path("pool-XXXXXX");
        std::string tmpl = base.string();
        std::vector<char> buf(tmpl.begin(), tmpl.end());
        buf.push_back('\0');
        char* created = mkdtemp(buf.data());
        base = created ? fs::path(created) : fs::temp_directory_path();
    }
    ~TempModelPoolDir() {
        std::error_code ec;
        fs::remove_all(base, ec);
    }
    fs::path base;
};

TEST(ModelPoolTest, LoadsAndCreatesContext) {
    TempModelPoolDir tmp;
    fs::path model = tmp.base / "m.gguf";
    fs::create_directories(model.parent_path());
    // Invalid GGUF file - llama.cpp will reject it
    std::ofstream(model) << "GGUF";

    auto manager = std::make_shared<LlamaManager>(tmp.base.string());
    ModelPool pool(manager);

    // Invalid GGUF files cannot be loaded by llama.cpp
    auto ctx = pool.acquire("m.gguf");
    EXPECT_EQ(ctx, nullptr);
    EXPECT_EQ(pool.loadedCount(), 0u);
}

TEST(ModelPoolTest, ReturnsNullWhenMissing) {
    TempModelPoolDir tmp;
    auto manager = std::make_shared<LlamaManager>(tmp.base.string());
    ModelPool pool(manager);

    auto ctx = pool.acquire("missing.gguf");
    EXPECT_EQ(ctx, nullptr);
    EXPECT_EQ(pool.loadedCount(), 0u);
}

TEST(ModelPoolTest, RespectsMemoryLimit) {
    TempModelPoolDir tmp;
    fs::path model = tmp.base / "m.gguf";
    fs::create_directories(model.parent_path());
    // Invalid GGUF file
    std::ofstream(model) << "GGUF";

    auto manager = std::make_shared<LlamaManager>(tmp.base.string());
    ModelPool pool(manager);
    pool.setMemoryLimit(256ull * 1024ull * 1024ull);  // lower than 512MB placeholder
    // Invalid file won't load anyway
    auto ctx = pool.acquire("m.gguf");
    EXPECT_EQ(ctx, nullptr);
    EXPECT_EQ(pool.loadedCount(), 0u);
}

TEST(ModelPoolTest, ThreadSafeAcquire) {
    TempModelPoolDir tmp;
    fs::path model = tmp.base / "m.gguf";
    fs::create_directories(model.parent_path());
    // Invalid GGUF file
    std::ofstream(model) << "GGUF";

    auto manager = std::make_shared<LlamaManager>(tmp.base.string());
    ModelPool pool(manager);

    std::vector<std::thread> threads;
    std::atomic<int> success{0};
    for (int i = 0; i < 8; ++i) {
        threads.emplace_back([&]() {
            auto ctx = pool.acquire("m.gguf");
            if (ctx) success++;
        });
    }
    for (auto& t : threads) t.join();
    // Invalid files won't load, so success count is 0
    EXPECT_EQ(success.load(), 0);
}

TEST(ModelPoolTest, ThreadLocalCacheReturnsSameContext) {
    TempModelPoolDir tmp;
    fs::path model = tmp.base / "m.gguf";
    fs::create_directories(model.parent_path());
    // Invalid GGUF file
    std::ofstream(model) << "GGUF";
    auto manager = std::make_shared<LlamaManager>(tmp.base.string());
    ModelPool pool(manager);

    // Invalid files return nullptr
    auto ctx1 = pool.acquireForThread("m.gguf", std::this_thread::get_id());
    auto ctx2 = pool.acquireForThread("m.gguf", std::this_thread::get_id());
    EXPECT_EQ(ctx1, nullptr);
    EXPECT_EQ(ctx2, nullptr);
}

TEST(ModelPoolTest, GcClearsThreadCache) {
    TempModelPoolDir tmp;
    fs::path model = tmp.base / "m.gguf";
    fs::create_directories(model.parent_path());
    // Invalid GGUF file
    std::ofstream(model) << "GGUF";
    auto manager = std::make_shared<LlamaManager>(tmp.base.string());
    ModelPool pool(manager);

    // Invalid files return nullptr
    auto ctx1 = pool.acquireForThread("m.gguf", std::this_thread::get_id());
    EXPECT_EQ(ctx1, nullptr);
    pool.gc();
    auto ctx2 = pool.acquireForThread("m.gguf", std::this_thread::get_id());
    EXPECT_EQ(ctx2, nullptr);
}

TEST(ModelPoolTest, GcUnloadsAll) {
    TempModelPoolDir tmp;
    fs::path model = tmp.base / "m.gguf";
    fs::create_directories(model.parent_path());
    // Invalid GGUF file
    std::ofstream(model) << "GGUF";
    auto manager = std::make_shared<LlamaManager>(tmp.base.string());
    ModelPool pool(manager);
    // Invalid files won't load
    auto ctx = pool.acquire("m.gguf");
    EXPECT_EQ(ctx, nullptr);
    EXPECT_EQ(pool.loadedCount(), 0u);
    pool.gc();
    EXPECT_EQ(pool.loadedCount(), 0u);
    EXPECT_EQ(manager->memoryUsageBytes(), 0u);
}

TEST(ModelPoolTest, UnloadRemovesModel) {
    TempModelPoolDir tmp;
    fs::path model = tmp.base / "m.gguf";
    fs::create_directories(model.parent_path());
    // Invalid GGUF file
    std::ofstream(model) << "GGUF";
    auto manager = std::make_shared<LlamaManager>(tmp.base.string());
    ModelPool pool(manager);
    // Invalid files won't load
    auto ctx = pool.acquire("m.gguf");
    EXPECT_EQ(ctx, nullptr);
    EXPECT_EQ(pool.loadedCount(), 0u);
    // Unloading non-existent model returns false
    EXPECT_FALSE(pool.unload("m.gguf"));
    EXPECT_EQ(pool.loadedCount(), 0u);
    EXPECT_EQ(manager->memoryUsageBytes(), 0u);
}

// T141/T146: 並行ロードテスト

TEST(ModelPoolTest, AcquireAsyncReturnsNullForInvalidFile) {
    TempModelPoolDir tmp;
    fs::path model = tmp.base / "m.gguf";
    fs::create_directories(model.parent_path());
    // Invalid GGUF file
    std::ofstream(model) << "GGUF";
    auto manager = std::make_shared<LlamaManager>(tmp.base.string());
    ModelPool pool(manager);

    auto future = pool.acquireAsync("m.gguf");
    auto ctx = future.get();
    EXPECT_EQ(ctx, nullptr);
    EXPECT_EQ(pool.loadedCount(), 0u);
    EXPECT_EQ(pool.loadingCount(), 0u);
}

TEST(ModelPoolTest, AcquireAsyncRespectsMemoryLimit) {
    TempModelPoolDir tmp;
    fs::path model = tmp.base / "m.gguf";
    fs::create_directories(model.parent_path());
    std::ofstream(model) << "GGUF";
    auto manager = std::make_shared<LlamaManager>(tmp.base.string());
    ModelPool pool(manager);

    // 非常に小さいメモリ制限を設定
    pool.setMemoryLimit(1024);
    pool.setEstimatedModelSize(512ull * 1024 * 1024);

    auto future = pool.acquireAsync("m.gguf");
    auto ctx = future.get();
    EXPECT_EQ(ctx, nullptr);  // メモリ制限超過で即座にnull返却
}

TEST(ModelPoolTest, ConcurrentAcquireAsyncForDifferentModels) {
    TempModelPoolDir tmp;
    fs::path model1 = tmp.base / "m1.gguf";
    fs::path model2 = tmp.base / "m2.gguf";
    fs::create_directories(model1.parent_path());
    std::ofstream(model1) << "GGUF";
    std::ofstream(model2) << "GGUF";
    auto manager = std::make_shared<LlamaManager>(tmp.base.string());
    ModelPool pool(manager);

    // 十分なメモリ制限
    pool.setMemoryLimit(4ull * 1024 * 1024 * 1024);
    pool.setEstimatedModelSize(512ull * 1024 * 1024);

    // 並行で異なるモデルをロード
    auto future1 = pool.acquireAsync("m1.gguf");
    auto future2 = pool.acquireAsync("m2.gguf");

    auto ctx1 = future1.get();
    auto ctx2 = future2.get();

    // 両方ともnull（無効ファイル）だが、並行実行できたことを確認
    EXPECT_EQ(ctx1, nullptr);
    EXPECT_EQ(ctx2, nullptr);
    EXPECT_EQ(pool.loadingCount(), 0u);
}

TEST(ModelPoolTest, ConcurrentAcquireAsyncForSameModelWaits) {
    TempModelPoolDir tmp;
    fs::path model = tmp.base / "m.gguf";
    fs::create_directories(model.parent_path());
    std::ofstream(model) << "GGUF";
    auto manager = std::make_shared<LlamaManager>(tmp.base.string());
    ModelPool pool(manager);

    pool.setMemoryLimit(4ull * 1024 * 1024 * 1024);
    pool.setEstimatedModelSize(512ull * 1024 * 1024);

    // 同一モデルの並行ロードは待機する
    std::vector<std::future<std::shared_ptr<LlamaContext>>> futures;
    for (int i = 0; i < 4; ++i) {
        futures.push_back(pool.acquireAsync("m.gguf"));
    }

    for (auto& f : futures) {
        auto ctx = f.get();
        EXPECT_EQ(ctx, nullptr);  // 無効ファイル
    }

    EXPECT_EQ(pool.loadingCount(), 0u);
}

TEST(ModelPoolTest, CanLoadConcurrentlyReflectsMemoryState) {
    TempModelPoolDir tmp;
    auto manager = std::make_shared<LlamaManager>(tmp.base.string());
    ModelPool pool(manager);

    // メモリ制限なし → 常にtrue
    EXPECT_TRUE(pool.canLoadConcurrently());

    // メモリ制限設定
    pool.setMemoryLimit(1024ull * 1024 * 1024);  // 1GB
    pool.setEstimatedModelSize(512ull * 1024 * 1024);  // 512MB

    // まだ余裕がある
    EXPECT_TRUE(pool.canLoadConcurrently());
}

TEST(ModelPoolTest, LoadingCountTracksInProgressLoads) {
    TempModelPoolDir tmp;
    auto manager = std::make_shared<LlamaManager>(tmp.base.string());
    ModelPool pool(manager);

    EXPECT_EQ(pool.loadingCount(), 0u);

    // 存在しないファイルへのロードを開始
    auto future = pool.acquireAsync("nonexistent.gguf");

    // ロード完了を待機
    future.get();

    EXPECT_EQ(pool.loadingCount(), 0u);
}

TEST(ModelPoolTest, EstimatedModelSizeGetterSetter) {
    TempModelPoolDir tmp;
    auto manager = std::make_shared<LlamaManager>(tmp.base.string());
    ModelPool pool(manager);

    EXPECT_EQ(pool.getEstimatedModelSize(), 0u);

    pool.setEstimatedModelSize(1ull * 1024 * 1024 * 1024);
    EXPECT_EQ(pool.getEstimatedModelSize(), 1ull * 1024 * 1024 * 1024);
}
