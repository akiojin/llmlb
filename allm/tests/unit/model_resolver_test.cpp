// SPEC-48678000: ModelResolver unit tests (updated)
#include <gtest/gtest.h>
#include <chrono>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <memory>
#include <string>
#include <thread>
#include <vector>
#include <httplib.h>
#include <nlohmann/json.hpp>

#include "models/model_resolver.h"
#include "models/model_sync.h"

using namespace allm;
namespace fs = std::filesystem;

static void wait_for_server(httplib::Server& server, std::chrono::milliseconds timeout) {
    const auto start = std::chrono::steady_clock::now();
    while (!server.is_running()) {
        if (std::chrono::steady_clock::now() - start > timeout) {
            FAIL() << "Server failed to start within timeout";
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
}

class TempModelDirs {
public:
    TempModelDirs() {
        local = fs::temp_directory_path() / "model-resolver-local-XXXXXX";

        std::string local_tmpl = local.string();
        std::vector<char> local_buf(local_tmpl.begin(), local_tmpl.end());
        local_buf.push_back('\0');
        char* local_created = mkdtemp(local_buf.data());
        local = local_created ? fs::path(local_created) : fs::temp_directory_path() / "local";

        fs::create_directories(local);
    }

    ~TempModelDirs() {
        std::error_code ec;
        fs::remove_all(local, ec);
    }

    fs::path local;
};

// RegistryServer - minimal test server for model registry
struct RegistryServer {
    httplib::Server server;
    std::thread thread;
    int port{0};
    std::string model_name;
    std::string manifest_body;
    std::string file_body;
    std::vector<std::pair<std::string, std::string>> files;

    void setManifestBody(std::string body) { manifest_body = std::move(body); }
    void setFileBody(std::string body) { file_body = std::move(body); }
    void setFiles(std::vector<std::pair<std::string, std::string>> f) { files = std::move(f); }

    // Start server with explicit serve_manifest parameter (default true)
    void start(int p, const std::string& mn, bool serve = true) {
        port = p;
        model_name = mn;

        // Register manifest endpoint
        std::string manifest_path = "/v0/models/registry/" + model_name + "/manifest.json";
        std::string mbody = manifest_body;
        std::string base = baseUrl();
        server.Get(manifest_path.c_str(), [serve, mbody, base](const httplib::Request&, httplib::Response& res) {
            if (!serve) {
                res.status = 404;
                return;
            }
            std::string body = mbody;
            if (body.empty()) {
                body = std::string("{\"files\":[{\"name\":\"model.gguf\",\"url\":\"") +
                       base + "/files/model.gguf\"}]}";
            }
            res.status = 200;
            res.set_content(body, "application/json");
        });

        // Register file endpoints
        if (files.empty()) {
            std::string fb = file_body;
            server.Get("/files/model.gguf", [fb](const httplib::Request&, httplib::Response& res) {
                std::string body = fb.empty() ? std::string("GGUF test") : fb;
                res.status = 200;
                res.set_content(body, "application/octet-stream");
            });
        } else {
            for (const auto& entry : files) {
                std::string fpath = "/files/" + entry.first;
                std::string fbody = entry.second;
                server.Get(fpath, [fbody](const httplib::Request&, httplib::Response& res) {
                    res.status = 200;
                    res.set_content(fbody, "application/octet-stream");
                });
            }
        }

        thread = std::thread([this]() { server.listen("127.0.0.1", port); });
        wait_for_server(server, std::chrono::seconds(5));
    }

    void stop() {
        server.stop();
        if (thread.joinable()) thread.join();
    }

    std::string baseUrl() const {
        return "http://127.0.0.1:" + std::to_string(port);
    }
};

// Helper: create model directory with model.gguf
static void create_model(const fs::path& models_dir, const std::string& dir_name) {
    auto model_dir = models_dir / dir_name;
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "dummy gguf content";
}

// ===========================================================================
// Local resolution tests
// ===========================================================================

TEST(ModelResolverTest, LocalPathTakesPriority) {
    TempModelDirs tmp;
    create_model(tmp.local, "gpt-oss-7b");

    ModelResolver resolver(tmp.local.string(), "");
    auto result = resolver.resolve("gpt-oss-7b");

    EXPECT_TRUE(result.success) << result.error_message;
    EXPECT_TRUE(result.path.find(tmp.local.string()) != std::string::npos);
    EXPECT_FALSE(result.router_attempted);
}

// ===========================================================================
// Debug: Basic httplib server test
// ===========================================================================

TEST(ModelResolverTest, BasicHttplibServer) {
    // Simple test to verify httplib::Server works in this test context
    httplib::Server svr;
    svr.Get("/ping", [](const httplib::Request&, httplib::Response& res) {
        res.set_content("pong", "text/plain");
    });
    std::thread thread([&svr]() { svr.listen("127.0.0.1", 19898); });
    std::this_thread::sleep_for(std::chrono::milliseconds(100));
    EXPECT_TRUE(svr.is_running());
    svr.stop();
    thread.join();
}

TEST(ModelResolverTest, RegistryServerOnly) {
    // Test RegistryServer heap allocation
    auto server = std::make_unique<RegistryServer>();
    server->start(19897, "test-model");

    httplib::Client client("127.0.0.1", 19897);
    auto res = client.Get("/v0/models/registry/test-model/manifest.json");
    ASSERT_TRUE(res) << "HTTP request failed";
    EXPECT_EQ(res->status, 200) << "Expected 200, got " << res->status;

    server->stop();
}

// ===========================================================================
// Registry manifest download tests
// ===========================================================================

TEST(ModelResolverTest, DownloadFromRegistryWhenNotLocal) {
    TempModelDirs tmp;
    auto server = std::make_unique<RegistryServer>();
    server->start(20001, "registry-model");

    ModelResolver resolver(tmp.local.string(), server->baseUrl());
    resolver.setOriginAllowlist({"127.0.0.1/*"});
    auto result = resolver.resolve("registry-model");

    server->stop();

    EXPECT_TRUE(result.success) << result.error_message;
    EXPECT_TRUE(result.router_attempted);
    EXPECT_FALSE(result.origin_attempted);
    EXPECT_TRUE(result.path.find(tmp.local.string()) != std::string::npos);
    EXPECT_TRUE(fs::exists(result.path));
}

TEST(ModelResolverTest, ReportsSyncProgressDuringRegistryDownload) {
    TempModelDirs tmp;
    auto server = std::make_unique<RegistryServer>();
    server->start(20006, "progress-model");

    ModelSync sync(server->baseUrl(), tmp.local.string());
    ModelResolver resolver(tmp.local.string(), server->baseUrl());
    resolver.setOriginAllowlist({"127.0.0.1/*"});
    resolver.setSyncReporter(&sync);
    auto result = resolver.resolve("progress-model");

    server->stop();

    EXPECT_TRUE(result.success) << result.error_message;
    auto status = sync.getStatus();
    EXPECT_NE(status.state, SyncState::Idle);
    ASSERT_TRUE(status.current_download.has_value());
    EXPECT_EQ(status.current_download->model_id, "progress-model");
    EXPECT_EQ(status.current_download->file, "model.gguf");
    EXPECT_GT(status.current_download->downloaded_bytes, 0u);
}

TEST(ModelResolverTest, DownloadBlockedByAllowlist) {
    TempModelDirs tmp;
    auto server = std::make_unique<RegistryServer>();
    server->start(20002, "blocked-model");

    ModelResolver resolver(tmp.local.string(), server->baseUrl());
    resolver.setOriginAllowlist({"example.com/*"});
    auto result = resolver.resolve("blocked-model");

    server->stop();

    EXPECT_FALSE(result.success);
    EXPECT_TRUE(result.router_attempted);
    EXPECT_FALSE(result.origin_attempted);
}

TEST(ModelResolverTest, MissingManifestReturnsError) {
    TempModelDirs tmp;
    auto server = std::make_unique<RegistryServer>();
    server->start(20003, "missing-model", false);  // serve_manifest = false

    ModelResolver resolver(tmp.local.string(), server->baseUrl());
    auto result = resolver.resolve("missing-model");

    server->stop();

    EXPECT_FALSE(result.success);
    EXPECT_FALSE(result.error_message.empty());
    EXPECT_TRUE(result.error_message.find("missing-model") != std::string::npos);
    EXPECT_TRUE(result.router_attempted);
}

// Error response should be within 1 second
TEST(ModelResolverTest, ErrorResponseWithinOneSecond) {
    TempModelDirs tmp;

    ModelResolver resolver(tmp.local.string(), "");

    auto start = std::chrono::steady_clock::now();
    auto result = resolver.resolve("nonexistent-model");
    auto end = std::chrono::steady_clock::now();

    auto duration = std::chrono::duration_cast<std::chrono::milliseconds>(end - start);
    EXPECT_LT(duration.count(), 1000) << "Error response took longer than 1 second";
    EXPECT_FALSE(result.success);
}

// Clarification: Registry download timeout (recommended: 5 minutes)
TEST(ModelResolverTest, RouterDownloadHasTimeout) {
    TempModelDirs tmp;

    ModelResolver resolver(tmp.local.string(), "");

    EXPECT_TRUE(resolver.getDownloadTimeoutMs() > 0)
        << "Should have a download timeout configured";
    EXPECT_LE(resolver.getDownloadTimeoutMs(), 5 * 60 * 1000)
        << "Default timeout should be at most 5 minutes";
}

TEST(ModelResolverTest, SupportsSafetensorsAndGgufFormats) {
    TempModelDirs tmp;
    auto server = std::make_unique<RegistryServer>();
    const int port = 20004;
    const std::string base_url = "http://127.0.0.1:" + std::to_string(port);
    server->setFiles({
        {"model.gguf", "gguf"},
        {"config.json", "{}"},
        {"tokenizer.json", "{}"},
        {"model.safetensors", "safetensors"}
    });
    // Set manifest BEFORE start() since lambda captures manifest_body at start time
    nlohmann::json manifest = {
        {"files", {
            {{"name", "model.gguf"}, {"url", base_url + "/files/model.gguf"}},
            {{"name", "config.json"}, {"url", base_url + "/files/config.json"}},
            {{"name", "tokenizer.json"}, {"url", base_url + "/files/tokenizer.json"}},
            {{"name", "model.safetensors"}, {"url", base_url + "/files/model.safetensors"}}
        }}
    };
    server->setManifestBody(manifest.dump());
    server->start(port, "mixed-format-model");

    ModelResolver resolver(tmp.local.string(), server->baseUrl());
    resolver.setOriginAllowlist({"127.0.0.1/*"});
    auto result = resolver.resolve("mixed-format-model");

    server->stop();

    EXPECT_TRUE(result.success) << result.error_message;
    EXPECT_TRUE(fs::exists(result.path));
    EXPECT_EQ(fs::path(result.path).filename(), "model.gguf");
}

TEST(ModelResolverTest, MetalArtifactIsOptional) {
    TempModelDirs tmp;
    auto server = std::make_unique<RegistryServer>();
    const int port = 20005;
    const std::string base_url = "http://127.0.0.1:" + std::to_string(port);
    server->setFiles({
        {"config.json", R"({"architectures":["LlamaForCausalLM"]})"},
        {"tokenizer.json", "{}"},
        {"model.safetensors", "safetensors"}
    });
    // Set manifest BEFORE start() since lambda captures manifest_body at start time
    nlohmann::json manifest = {
        {"files", {
            {{"name", "config.json"}, {"url", base_url + "/files/config.json"}},
            {{"name", "tokenizer.json"}, {"url", base_url + "/files/tokenizer.json"}},
            {{"name", "model.safetensors"}, {"url", base_url + "/files/model.safetensors"}}
        }}
    };
    server->setManifestBody(manifest.dump());
    server->start(port, "llama-safetensors");

    ModelResolver resolver(tmp.local.string(), server->baseUrl());
    resolver.setOriginAllowlist({"127.0.0.1/*"});
    auto result = resolver.resolve("llama-safetensors");

    server->stop();

    EXPECT_TRUE(result.success) << result.error_message;
    EXPECT_TRUE(fs::exists(result.path));
    EXPECT_EQ(fs::path(result.path).filename(), "model.safetensors");
}

// Clarification: Concurrent download limit (recommended: 1 per node)
TEST(ModelResolverTest, ConcurrentDownloadLimit) {
    TempModelDirs tmp;

    ModelResolver resolver(tmp.local.string(), "");

    EXPECT_EQ(resolver.getMaxConcurrentDownloads(), 1)
        << "Should limit to 1 concurrent download per node";
}
