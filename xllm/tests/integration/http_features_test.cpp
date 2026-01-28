#include <gtest/gtest.h>
#include <httplib.h>

#include "api/http_server.h"
#include "api/openai_endpoints.h"
#include "api/node_endpoints.h"
#include "core/inference_engine.h"
#include "models/model_registry.h"
#include "runtime/state.h"
#include "utils/config.h"

using namespace xllm;

namespace {
class TestServer {
public:
    TestServer(int port) : port_(port) {
        xllm::set_ready(true);
        registry_.setModels({"gpt-oss-7b"});
        server_ = std::make_unique<HttpServer>(port_, openai_, node_, "127.0.0.1");
    }

    void start() { server_->start(); }
    void stop() { server_->stop(); }

    HttpServer& server() { return *server_; }

private:
    int port_;
    ModelRegistry registry_{};
    InferenceEngine engine_{};
    NodeConfig config_{};
    OpenAIEndpoints openai_{registry_, engine_, config_, GpuBackend::Cpu};
    NodeEndpoints node_{};
    std::unique_ptr<HttpServer> server_{};
};
}  // namespace

TEST(HttpFeaturesTest, SetsCorsHeadersAndHandlesPreflight) {
    TestServer server(18111);
    server.server().enableCors(true);
    server.server().setCorsOrigin("https://example.com");
    server.server().setCorsMethods("GET, POST, OPTIONS");
    server.server().setCorsHeaders("Content-Type, Authorization");
    server.start();

    httplib::Client cli("127.0.0.1", 18111);
    auto res = cli.Options("/v1/models");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 204);
    EXPECT_EQ(res->get_header_value("Access-Control-Allow-Origin"), "https://example.com");
    EXPECT_EQ(res->get_header_value("Access-Control-Allow-Methods"), "GET, POST, OPTIONS");
    EXPECT_EQ(res->get_header_value("Access-Control-Allow-Headers"), "Content-Type, Authorization");

    server.stop();
}

TEST(HttpFeaturesTest, ReturnsRequestIdHeader) {
    TestServer server(18112);
    server.start();

    httplib::Client cli("127.0.0.1", 18112);
    httplib::Headers headers = { {"X-Request-Id", "req-123"} };
    auto res = cli.Get("/v1/models", headers);
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    EXPECT_EQ(res->get_header_value("X-Request-Id"), "req-123");

    server.stop();
}

TEST(HttpFeaturesTest, GzipCompressionEnabled) {
    TestServer server(18113);
    server.server().enableCompression(true);
    server.start();

    httplib::Client cli("127.0.0.1", 18113);
    cli.set_decompress(false);
    httplib::Headers headers = { {"Accept-Encoding", "gzip"} };
    auto res = cli.Get("/v1/models", headers);
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    EXPECT_EQ(res->get_header_value("Content-Encoding"), "gzip");

    server.stop();
}
