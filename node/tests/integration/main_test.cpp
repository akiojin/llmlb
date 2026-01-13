#include <gtest/gtest.h>
#include <httplib.h>
#include <thread>
#include <atomic>
#include <cstdlib>
#include <filesystem>
#include <csignal>

#include "runtime/state.h"

extern "C" int llm_node_run_for_test();

using namespace std::chrono_literals;

class TempDir {
public:
    TempDir() {
        auto base = std::filesystem::temp_directory_path();
        path = base / ("llm-main-" + std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
        std::filesystem::create_directories(path);
    }
    ~TempDir() {
        std::error_code ec;
        std::filesystem::remove_all(path, ec);
    }
    std::filesystem::path path;
};

TEST(MainTest, RunsWithStubRouterAndShutsDownOnFlag) {
    const int router_port = 18130;
    const int node_port = 18131;
    const std::string expected_auth = "Bearer sk_test_node";

    // Stub router that accepts register/heartbeat and lists one model
    httplib::Server router;
    router.Post("/v0/nodes", [&](const httplib::Request& req, httplib::Response& res) {
        if (req.get_header_value("Authorization") != expected_auth) {
            res.status = 401;
            res.set_content("missing auth", "text/plain");
            return;
        }
        res.status = 200;
        res.set_content(R"({"node_id":"test-node","node_token":"test-token"})", "application/json");
    });
    router.Post("/v0/health", [&](const httplib::Request& req, httplib::Response& res) {
        if (req.get_header_value("Authorization") != expected_auth) {
            res.status = 401;
            res.set_content("missing auth", "text/plain");
            return;
        }
        res.status = 200;
        res.set_content("ok", "text/plain");
    });
    router.Get("/v1/models", [](const httplib::Request&, httplib::Response& res) {
        res.status = 200;
        res.set_content(R"({"data":[{"id":"gpt-oss-7b"}]})", "application/json");
    });

    std::thread router_thread([&]() { router.listen("127.0.0.1", router_port); });
    while (!router.is_running()) std::this_thread::sleep_for(10ms);

    TempDir models;
    setenv("LLM_ROUTER_URL", ("http://127.0.0.1:" + std::to_string(router_port)).c_str(), 1);
    setenv("LLM_NODE_PORT", std::to_string(node_port).c_str(), 1);
    setenv("LLM_MODELS_DIR", models.path.string().c_str(), 1);
    setenv("LLM_HEARTBEAT_SECS", "1", 1);
    setenv("LLM_NODE_API_KEY", "sk_test_node", 1);

    std::atomic<int> exit_code{0};
    std::thread node_thread([&]() { exit_code = llm_node_run_for_test(); });

    // wait for node to start and accept a health check
    {
        httplib::Client cli("127.0.0.1", node_port);
        for (int i = 0; i < 50; ++i) {
            if (auto res = cli.Get("/health")) {
                if (res->status == 200) break;
            }
            std::this_thread::sleep_for(50ms);
        }
    }

    llm_node::request_shutdown();
    node_thread.join();

    router.stop();
    if (router_thread.joinable()) router_thread.join();

    EXPECT_EQ(exit_code.load(), 0);
}

TEST(MainTest, FailsWhenRouterRegistrationFails) {
    const int router_port = 18132;
    const int node_port = 18133;

    httplib::Server router;
    router.Post("/v0/nodes", [](const httplib::Request&, httplib::Response& res) {
        res.status = 500;
        res.set_content("error", "text/plain");
    });
    std::thread router_thread([&]() { router.listen("127.0.0.1", router_port); });
    while (!router.is_running()) std::this_thread::sleep_for(10ms);

    TempDir models;
    setenv("LLM_ROUTER_URL", ("http://127.0.0.1:" + std::to_string(router_port)).c_str(), 1);
    setenv("LLM_NODE_PORT", std::to_string(node_port).c_str(), 1);
    setenv("LLM_MODELS_DIR", models.path.string().c_str(), 1);
    setenv("LLM_HEARTBEAT_SECS", "1", 1);
    setenv("LLM_NODE_API_KEY", "sk_test_node", 1);

    std::atomic<int> exit_code{0};
    std::atomic<bool> node_exited{false};
    std::thread node_thread([&]() {
        exit_code = llm_node_run_for_test();
        node_exited = true;
    });

    // Wait up to 10 seconds for node to exit on registration failure
    for (int i = 0; i < 100 && !node_exited; ++i) {
        std::this_thread::sleep_for(100ms);
    }

    // If node didn't exit, force shutdown and fail
    if (!node_exited) {
        llm_node::request_shutdown();
        node_thread.join();
        router.stop();
        if (router_thread.joinable()) router_thread.join();
        FAIL() << "Node did not exit on registration failure within 10 seconds";
        return;
    }

    node_thread.join();

    router.stop();
    if (router_thread.joinable()) router_thread.join();

    EXPECT_NE(exit_code.load(), 0);
}
