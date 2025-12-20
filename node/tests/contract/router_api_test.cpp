#include <gtest/gtest.h>
#include <httplib.h>
#include <thread>

#include "api/router_client.h"

using namespace llm_node;

class RouterContractFixture : public ::testing::Test {
protected:
    void SetUp() override {
        server.Post("/v0/nodes", [this](const httplib::Request& req, httplib::Response& res) {
            last_register = req.body;
            last_register_auth = req.get_header_value("Authorization");
            res.status = 200;
            res.set_content(R"({"node_id":"node-123","node_token":"test-token"})", "application/json");
        });
        server.Post("/v0/health", [this](const httplib::Request& req, httplib::Response& res) {
            last_heartbeat = req.body;
            last_heartbeat_token = req.get_header_value("X-Node-Token");
            last_heartbeat_auth = req.get_header_value("Authorization");
            res.status = 200;
            res.set_content("ok", "text/plain");
        });
        thread = std::thread([this]() { server.listen("127.0.0.1", 18091); });
        while (!server.is_running()) std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }

    void TearDown() override {
        server.stop();
        if (thread.joinable()) thread.join();
    }

    httplib::Server server;
    std::thread thread;
    std::string last_register;
    std::string last_register_auth;
    std::string last_heartbeat;
    std::string last_heartbeat_token;
    std::string last_heartbeat_auth;
};

TEST_F(RouterContractFixture, RegisterNodeReturnsId) {
    RouterClient client("http://127.0.0.1:18091");
    client.setApiKey("sk_contract_node");
    NodeInfo info;
    info.machine_name = "test-host";
    info.ip_address = "127.0.0.1";
    info.runtime_version = "1.0.0";
    info.runtime_port = 11434;
    info.gpu_available = true;
    info.gpu_devices = {{.model = "Test GPU", .count = 1, .memory = 4ull * 1024 * 1024 * 1024}};

    auto result = client.registerNode(info);
    EXPECT_TRUE(result.success);
    EXPECT_EQ(result.node_id, "node-123");
    EXPECT_EQ(result.node_token, "test-token");
    EXPECT_FALSE(last_register.empty());
    EXPECT_EQ(last_register_auth, "Bearer sk_contract_node");
}

TEST_F(RouterContractFixture, HeartbeatSendsStatus) {
    RouterClient client("http://127.0.0.1:18091");
    client.setApiKey("sk_contract_node");
    bool ok = client.sendHeartbeat("node-123", "test-token");
    EXPECT_TRUE(ok);
    EXPECT_NE(last_heartbeat.find("node-123"), std::string::npos);
    EXPECT_EQ(last_heartbeat_token, "test-token");
    EXPECT_EQ(last_heartbeat_auth, "Bearer sk_contract_node");
}
