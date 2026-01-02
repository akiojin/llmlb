// SPEC-48678000: ModelResolver unit tests (updated)
#include <gtest/gtest.h>
#include <chrono>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <string>
#include <thread>
#include <vector>
#include <httplib.h>
#include <nlohmann/json.hpp>

#include "models/model_resolver.h"
#include "models/model_sync.h"

using namespace llm_node;
namespace fs = std::filesystem;

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

class RegistryServer {
public:
    void setManifestBody(std::string body) { manifest_body_ = std::move(body); }
    void setFileBody(std::string body) { file_body_ = std::move(body); }
    void setFiles(std::vector<std::pair<std::string, std::string>> files) { files_ = std::move(files); }
    void setServeManifest(bool enable) { serve_manifest_ = enable; }

    void start(int port, const std::string& model_name) {
        port_ = port;
        model_name_ = model_name;

        const std::string manifest_path = "/v0/models/registry/" + model_name_ + "/manifest.json";
        server_.Get(manifest_path.c_str(), [this](const httplib::Request&, httplib::Response& res) {
            if (!serve_manifest_) {
                res.status = 404;
                return;
            }
            std::string body = manifest_body_;
            if (body.empty()) {
                body = std::string("{\"files\":[{\"name\":\"model.gguf\",\"url\":\"") +
                       baseUrl() + "/files/model.gguf\"}]}";
            }
            res.status = 200;
            res.set_content(body, "application/json");
        });

        if (files_.empty()) {
            server_.Get("/files/model.gguf", [this](const httplib::Request&, httplib::Response& res) {
                std::string body = file_body_.empty() ? std::string("GGUF test") : file_body_;
                res.status = 200;
                res.set_content(body, "application/octet-stream");
            });
        } else {
            for (const auto& entry : files_) {
                const auto path = "/files/" + entry.first;
                server_.Get(path.c_str(), [body = entry.second](const httplib::Request&, httplib::Response& res) {
                    res.status = 200;
                    res.set_content(body, "application/octet-stream");
                });
            }
        }

        thread_ = std::thread([this, port]() { server_.listen("127.0.0.1", port); });
        while (!server_.is_running()) {
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
        }
    }

    void stop() {
        server_.stop();
        if (thread_.joinable()) thread_.join();
    }

    ~RegistryServer() { stop(); }

    std::string baseUrl() const {
        return "http://127.0.0.1:" + std::to_string(port_);
    }

private:
    httplib::Server server_;
    std::thread thread_;
    int port_{0};
    std::string model_name_;
    std::string manifest_body_;
    std::string file_body_;
    std::vector<std::pair<std::string, std::string>> files_;
    bool serve_manifest_{true};
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
// Registry manifest download tests
// ===========================================================================

TEST(ModelResolverTest, DownloadFromRegistryWhenNotLocal) {
    TempModelDirs tmp;
    RegistryServer server;
    server.start(20001, "registry-model");

    ModelResolver resolver(tmp.local.string(), server.baseUrl());
    resolver.setOriginAllowlist({"127.0.0.1/*"});
    auto result = resolver.resolve("registry-model");

    server.stop();

    EXPECT_TRUE(result.success) << result.error_message;
    EXPECT_TRUE(result.router_attempted);
    EXPECT_FALSE(result.origin_attempted);
    EXPECT_TRUE(result.path.find(tmp.local.string()) != std::string::npos);
    EXPECT_TRUE(fs::exists(result.path));
}

TEST(ModelResolverTest, ReportsSyncProgressDuringRegistryDownload) {
    TempModelDirs tmp;
    RegistryServer server;
    server.start(20006, "progress-model");

    ModelSync sync(server.baseUrl(), tmp.local.string());
    ModelResolver resolver(tmp.local.string(), server.baseUrl());
    resolver.setOriginAllowlist({"127.0.0.1/*"});
    resolver.setSyncReporter(&sync);
    auto result = resolver.resolve("progress-model");

    server.stop();

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
    RegistryServer server;
    server.start(20002, "blocked-model");

    ModelResolver resolver(tmp.local.string(), server.baseUrl());
    resolver.setOriginAllowlist({"example.com/*"});
    auto result = resolver.resolve("blocked-model");

    server.stop();

    EXPECT_FALSE(result.success);
    EXPECT_TRUE(result.router_attempted);
    EXPECT_FALSE(result.origin_attempted);
}

TEST(ModelResolverTest, MissingManifestReturnsError) {
    TempModelDirs tmp;
    RegistryServer server;
    server.setServeManifest(false);
    server.start(20003, "missing-model");

    ModelResolver resolver(tmp.local.string(), server.baseUrl());
    auto result = resolver.resolve("missing-model");

    server.stop();

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
    RegistryServer server;
    server.setFiles({
        {"model.gguf", "gguf"},
        {"config.json", "{}"},
        {"tokenizer.json", "{}"},
        {"model.safetensors", "safetensors"}
    });
    server.start(20004, "mixed-format-model");
    nlohmann::json manifest = {
        {"files", {
            {{"name", "model.gguf"}, {"url", server.baseUrl() + "/files/model.gguf"}},
            {{"name", "config.json"}, {"url", server.baseUrl() + "/files/config.json"}},
            {{"name", "tokenizer.json"}, {"url", server.baseUrl() + "/files/tokenizer.json"}},
            {{"name", "model.safetensors"}, {"url", server.baseUrl() + "/files/model.safetensors"}}
        }}
    };
    server.setManifestBody(manifest.dump());

    ModelResolver resolver(tmp.local.string(), server.baseUrl());
    resolver.setOriginAllowlist({"127.0.0.1/*"});
    auto result = resolver.resolve("mixed-format-model");

    server.stop();

    EXPECT_TRUE(result.success) << result.error_message;
    EXPECT_TRUE(fs::exists(result.path));
    EXPECT_EQ(fs::path(result.path).filename(), "model.gguf");
}

TEST(ModelResolverTest, MetalArtifactIsOptional) {
    TempModelDirs tmp;
    RegistryServer server;
    server.setFiles({
        {"config.json", R"({"architectures":["GptOssForCausalLM"]})"},
        {"tokenizer.json", "{}"},
        {"model.safetensors", "safetensors"}
    });
    server.start(20005, "gptoss-safetensors");
    nlohmann::json manifest = {
        {"files", {
            {{"name", "config.json"}, {"url", server.baseUrl() + "/files/config.json"}},
            {{"name", "tokenizer.json"}, {"url", server.baseUrl() + "/files/tokenizer.json"}},
            {{"name", "model.safetensors"}, {"url", server.baseUrl() + "/files/model.safetensors"}}
        }}
    };
    server.setManifestBody(manifest.dump());

    ModelResolver resolver(tmp.local.string(), server.baseUrl());
    resolver.setOriginAllowlist({"127.0.0.1/*"});
    auto result = resolver.resolve("gptoss-safetensors");

    server.stop();

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
