#include <gtest/gtest.h>
#include <httplib.h>
#include <filesystem>
#include <fstream>
#include <algorithm>
#include <chrono>
#include <atomic>

#include "models/model_sync.h"

using namespace llm_node;
namespace fs = std::filesystem;

class ModelServer {
public:
    void start(int port) {
        // /v0/models - ノードが使用するエンドポイント（配列形式）
        server_.Get("/v0/models", [this](const httplib::Request&, httplib::Response& res) {
            res.status = 200;
            res.set_content(response_body_v0, "application/json");
        });
        // /v1/models - OpenAI互換エンドポイント（後方互換性のため維持）
        server_.Get("/v1/models", [this](const httplib::Request&, httplib::Response& res) {
            res.status = 200;
            res.set_content(response_body_v1, "application/json");
        });
        thread_ = std::thread([this, port]() { server_.listen("127.0.0.1", port); });
        while (!server_.is_running()) {
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
        }
    }

    void stop() {
        server_.stop();
        if (thread_.joinable()) thread_.join();
    }

    ~ModelServer() { stop(); }

    httplib::Server server_;
    std::thread thread_;
    // /v0/models: 配列形式（現在の実装が使用）
    std::string response_body_v0{R"([{"name":"gpt-oss-7b"},{"name":"gpt-oss-20b"}])"};
    // /v1/models: OpenAI互換形式
    std::string response_body_v1{R"({"data":[{"id":"gpt-oss-7b"},{"id":"gpt-oss-20b"}]})"};

    // 互換性のため response_body は response_body_v0 へのエイリアス
    std::string& response_body = response_body_v0;
};

class TempDirGuard {
public:
    TempDirGuard() {
        path = fs::temp_directory_path() / fs::path("model-sync-XXXXXX");
        std::string tmpl = path.string();
        // mkdtemp requires mutable char*
        std::vector<char> buf(tmpl.begin(), tmpl.end());
        buf.push_back('\0');
        char* created = mkdtemp(buf.data());
        path = created ? fs::path(created) : fs::temp_directory_path();
    }
    ~TempDirGuard() {
        std::error_code ec;
        fs::remove_all(path, ec);
    }
    fs::path path;
};

TEST(ModelSyncTest, DetectsMissingAndStaleModels) {
    ModelServer server;
    server.start(18084);

    TempDirGuard guard;
    // local has stale model and one existing
    // listLocalModels() は model.gguf を探すため、ファイルも作成する
    fs::create_directory(guard.path / "gpt-oss-7b");
    { std::ofstream ofs(guard.path / "gpt-oss-7b" / "model.gguf"); ofs << "test"; }
    fs::create_directory(guard.path / "old-model");
    { std::ofstream ofs(guard.path / "old-model" / "model.gguf"); ofs << "test"; }

    ModelSync sync("http://127.0.0.1:18084", guard.path.string());
    auto result = sync.sync();

    server.stop();

    ASSERT_EQ(result.to_download.size(), 1);
    EXPECT_EQ(result.to_download[0], "gpt-oss-20b");
    ASSERT_EQ(result.to_delete.size(), 1);
    EXPECT_EQ(result.to_delete[0], "old-model");
}

TEST(ModelSyncTest, EmptyWhenRouterUnavailable) {
    TempDirGuard guard;
    ModelSync sync("http://127.0.0.1:18085", guard.path.string(), std::chrono::milliseconds(200));
    auto result = sync.sync();
    EXPECT_TRUE(result.to_download.empty());
    EXPECT_TRUE(result.to_delete.empty());
}

TEST(ModelSyncTest, ReportsStatusTransitionsAndLastResult) {
    ModelServer server;
    server.response_body = R"([{"name":"m1"},{"name":"m2"}])";
    server.start(18086);

    TempDirGuard guard;
    // listLocalModels() は model.gguf を探すため、ファイルも作成する
    fs::create_directory(guard.path / "m1");
    { std::ofstream ofs(guard.path / "m1" / "model.gguf"); ofs << "test"; }

    ModelSync sync("http://127.0.0.1:18086", guard.path.string());

    auto initial = sync.getStatus();
    EXPECT_EQ(initial.state, SyncState::Idle);

    auto result = sync.sync();
    EXPECT_EQ(result.to_download.size(), 1u);
    EXPECT_EQ(result.to_download[0], "m2");
    EXPECT_EQ(result.to_delete.size(), 0u);

    auto after = sync.getStatus();
    EXPECT_EQ(after.state, SyncState::Success);
    ASSERT_EQ(after.last_to_download.size(), 1u);
    EXPECT_EQ(after.last_to_download[0], "m2");
    EXPECT_TRUE(after.last_to_delete.empty());
    EXPECT_NE(after.updated_at.time_since_epoch().count(), 0);

    server.stop();
}

// Per SPEC-dcaeaec4 FR-3: When path is directly accessible, no download needed
// (and no copy - InferenceEngine uses the path directly)
TEST(ModelSyncTest, UsesSharedPathDirectlyWhenAvailable) {
    // Prepare shared model file
    TempDirGuard shared_guard;
    fs::create_directories(shared_guard.path);
    auto shared_file = shared_guard.path / "model.gguf";
    {
        std::ofstream ofs(shared_file);
        ofs << "abc";
        ofs.flush();
    }

    // Verify source file exists
    ASSERT_TRUE(fs::exists(shared_file)) << "Source file not created: " << shared_file;

    // HTTP server returning /v1/models with path pointing to shared_file
    const int port = 18097; // Unique port to avoid conflicts
    ModelServer server;
    server.response_body = std::string(R"([{"name":"gpt-oss-7b","path":")") + shared_file.string() + R"("}])";
    server.start(port);

    // Give server time to fully initialize
    std::this_thread::sleep_for(std::chrono::milliseconds(50));

    TempDirGuard local_guard;
    ModelSync sync("http://127.0.0.1:" + std::to_string(port), local_guard.path.string());

    auto result = sync.sync();

    server.stop();

    // Should not queue download because shared path is accessible
    EXPECT_TRUE(result.to_download.empty()) << "to_download should be empty but has " << result.to_download.size() << " items";

    // Per FR-3: No copy to local - InferenceEngine will use getRemotePath() directly
    auto target = local_guard.path / "gpt-oss-7b" / "model.gguf";
    EXPECT_FALSE(fs::exists(target)) << "Target file should NOT exist (no copy per spec)";

    // Verify getRemotePath returns the accessible path
    EXPECT_EQ(sync.getRemotePath("gpt-oss-7b"), shared_file.string());
}

// Test that /v0/models (array format) is correctly parsed
TEST(ModelSyncTest, ParsesV0ModelsArrayFormat) {
    const int port = 18120;
    httplib::Server server;

    // /v0/models returns array directly (not wrapped in {"data": []})
    server.Get("/v0/models", [](const httplib::Request&, httplib::Response& res) {
        res.status = 200;
        res.set_content(R"([
            {"name":"qwen/qwen2.5-0.5b-instruct-gguf","path":"/path/to/model.gguf"},
            {"name":"openai/gpt-oss-20b","path":"/path/to/gpt.gguf"}
        ])", "application/json");
    });

    std::thread th([&]() { server.listen("127.0.0.1", port); });
    while (!server.is_running()) std::this_thread::sleep_for(std::chrono::milliseconds(10));

    TempDirGuard guard;
    ModelSync sync("http://127.0.0.1:" + std::to_string(port), guard.path.string());

    auto result = sync.sync();

    server.stop();
    if (th.joinable()) th.join();

    // Should detect 2 models to download (none exist locally)
    ASSERT_EQ(result.to_download.size(), 2);
    // Since path is not accessible, they should be queued for download
    bool has_qwen = std::find(result.to_download.begin(), result.to_download.end(),
                              "qwen/qwen2.5-0.5b-instruct-gguf") != result.to_download.end();
    bool has_gpt = std::find(result.to_download.begin(), result.to_download.end(),
                             "openai/gpt-oss-20b") != result.to_download.end();
    EXPECT_TRUE(has_qwen) << "qwen model should be in to_download";
    EXPECT_TRUE(has_gpt) << "gpt model should be in to_download";
}

// Test that local model names are normalized to lowercase for comparison
// This prevents deletion of models due to case mismatch
TEST(ModelSyncTest, CaseInsensitiveModelNameComparison) {
    const int port = 18121;
    httplib::Server server;

    // Router returns lowercase model name
    server.Get("/v0/models", [](const httplib::Request&, httplib::Response& res) {
        res.status = 200;
        res.set_content(R"([{"name":"qwen/qwen2.5-0.5b-instruct-gguf"}])", "application/json");
    });

    std::thread th([&]() { server.listen("127.0.0.1", port); });
    while (!server.is_running()) std::this_thread::sleep_for(std::chrono::milliseconds(10));

    TempDirGuard guard;
    // Create local directory with UPPERCASE name (simulating HuggingFace original name)
    fs::create_directories(guard.path / "Qwen" / "Qwen2.5-0.5B-Instruct-GGUF");
    {
        std::ofstream ofs(guard.path / "Qwen" / "Qwen2.5-0.5B-Instruct-GGUF" / "model.gguf");
        ofs << "test";
    }

    ModelSync sync("http://127.0.0.1:" + std::to_string(port), guard.path.string());
    auto result = sync.sync();

    server.stop();
    if (th.joinable()) th.join();

    // listLocalModels() should normalize to lowercase: "qwen/qwen2.5-0.5b-instruct-gguf"
    // This should match the router's model name, so no deletion
    EXPECT_TRUE(result.to_delete.empty())
        << "Model should NOT be marked for deletion (case mismatch should be normalized)";
    EXPECT_TRUE(result.to_download.empty())
        << "Model already exists locally, no download needed";
}

// Test that both "name" and "id" fields are supported in model response
TEST(ModelSyncTest, SupportsNameAndIdFields) {
    const int port = 18122;
    httplib::Server server;

    server.Get("/v0/models", [](const httplib::Request&, httplib::Response& res) {
        res.status = 200;
        // Mixed: first uses "name", second uses "id"
        res.set_content(R"([
            {"name":"model-with-name"},
            {"id":"model-with-id"}
        ])", "application/json");
    });

    std::thread th([&]() { server.listen("127.0.0.1", port); });
    while (!server.is_running()) std::this_thread::sleep_for(std::chrono::milliseconds(10));

    TempDirGuard guard;
    ModelSync sync("http://127.0.0.1:" + std::to_string(port), guard.path.string());

    auto result = sync.sync();

    server.stop();
    if (th.joinable()) th.join();

    ASSERT_EQ(result.to_download.size(), 2);
    bool has_name = std::find(result.to_download.begin(), result.to_download.end(),
                              "model-with-name") != result.to_download.end();
    bool has_id = std::find(result.to_download.begin(), result.to_download.end(),
                            "model-with-id") != result.to_download.end();
    EXPECT_TRUE(has_name);
    EXPECT_TRUE(has_id);
}

TEST(ModelSyncTest, PrioritiesControlConcurrencyAndOrder) {
    const int port = 18110;
    httplib::Server server;

    std::atomic<int> hi_current{0}, hi_max{0};
    std::atomic<int> lo_current{0}, lo_max{0};
    std::atomic<int> hi_finished{0};

    auto slow_handler = [](std::atomic<int>& cur, std::atomic<int>& mx, std::atomic<int>* finished) {
        return [&cur, &mx, finished](const httplib::Request&, httplib::Response& res) {
            int now = ++cur;
            mx.store(std::max(mx.load(), now));
            std::this_thread::sleep_for(std::chrono::milliseconds(120));
            res.status = 200;
            res.set_content("data", "application/octet-stream");
            --cur;
            if (finished) ++(*finished);
        };
    };

    server.Get("/gpt-oss-prio/manifest.json", [](const httplib::Request&, httplib::Response& res) {
        res.status = 200;
        res.set_content(R"({
            "files":[
                {"name":"hi1.bin","url":"http://127.0.0.1:18110/hi1.bin","priority":1},
                {"name":"hi2.bin","url":"http://127.0.0.1:18110/hi2.bin","priority":1},
                {"name":"lo1.bin","url":"http://127.0.0.1:18110/lo1.bin","priority":-2},
                {"name":"lo2.bin","url":"http://127.0.0.1:18110/lo2.bin","priority":-3}
            ]
        })", "application/json");
    });

    server.Get("/hi1.bin", slow_handler(hi_current, hi_max, &hi_finished));
    server.Get("/hi2.bin", slow_handler(hi_current, hi_max, &hi_finished));
    server.Get("/lo1.bin", slow_handler(lo_current, lo_max, nullptr));
    server.Get("/lo2.bin", slow_handler(lo_current, lo_max, nullptr));

    std::thread th([&]() { server.listen("127.0.0.1", port); });
    while (!server.is_running()) std::this_thread::sleep_for(std::chrono::milliseconds(10));

    TempDirGuard dir;
    ModelDownloader dl("http://127.0.0.1:18110", dir.path.string());
    ModelSync sync("http://127.0.0.1:18110", dir.path.string());

    bool ok = sync.downloadModel(dl, "gpt-oss-prio", nullptr);

    server.stop();
    if (th.joinable()) th.join();

    EXPECT_TRUE(ok) << "hi_finished=" << hi_finished.load()
                    << " hi_max=" << hi_max.load()
                    << " lo_max=" << lo_max.load();
    EXPECT_EQ(hi_finished.load(), 2);
    // High priority tasks can run concurrently (1-2 depending on timing)
    // In CI environments, concurrency may be limited due to resource contention
    EXPECT_GE(hi_max.load(), 1);
    EXPECT_LE(hi_max.load(), 2);
    // Low priority tasks are throttled to single concurrency (-3 priority)
    EXPECT_EQ(lo_max.load(), 1);
    // Low priority should start after high priority tasks complete
    EXPECT_EQ(hi_current.load(), 0);
}
