// ImageEndpoints integration tests (TDD)
#include <gtest/gtest.h>
#include <httplib.h>
#include <nlohmann/json.hpp>

#include "api/http_server.h"
#include "api/openai_endpoints.h"
#include "api/node_endpoints.h"
#include "api/image_endpoints.h"
#include "models/model_registry.h"
#include "core/inference_engine.h"
#include "core/image_manager.h"
#include "utils/config.h"

using namespace llm_node;
using json = nlohmann::json;

class ImageEndpointsTest : public ::testing::Test {
protected:
    void SetUp() override {
        registry = std::make_unique<ModelRegistry>();
        registry->setModels({"test-model"});
        engine = std::make_unique<InferenceEngine>();
        config = NodeConfig{};
        openai = std::make_unique<OpenAIEndpoints>(*registry, *engine, config);
        node_endpoints = std::make_unique<NodeEndpoints>();

        // Use empty scripts dir (no real Python execution in tests)
        image_manager = std::make_unique<ImageManager>("");
        image_endpoints = std::make_unique<ImageEndpoints>(*image_manager, config);

        server = std::make_unique<HttpServer>(18095, *openai, *node_endpoints);
        image_endpoints->registerRoutes(server->getServer());
        server->start();

        client = std::make_unique<httplib::Client>("127.0.0.1", 18095);
    }

    void TearDown() override {
        server->stop();
    }

    std::unique_ptr<ModelRegistry> registry;
    std::unique_ptr<InferenceEngine> engine;
    NodeConfig config;
    std::unique_ptr<OpenAIEndpoints> openai;
    std::unique_ptr<NodeEndpoints> node_endpoints;
    std::unique_ptr<ImageManager> image_manager;
    std::unique_ptr<ImageEndpoints> image_endpoints;
    std::unique_ptr<HttpServer> server;
    std::unique_ptr<httplib::Client> client;
};

// Test: POST /v1/images/generations with missing prompt returns 400
TEST_F(ImageEndpointsTest, GenerationsWithMissingPromptReturns400) {
    json body = {
        {"model", "z-image-turbo"}
        // Missing "prompt" field
    };

    auto res = client->Post("/v1/images/generations", body.dump(), "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 400);

    auto resp_json = json::parse(res->body, nullptr, false);
    ASSERT_FALSE(resp_json.is_discarded());
    EXPECT_TRUE(resp_json.contains("error"));
}

// Test: POST /v1/images/generations with empty prompt returns 400
TEST_F(ImageEndpointsTest, GenerationsWithEmptyPromptReturns400) {
    json body = {
        {"model", "z-image-turbo"},
        {"prompt", ""}
    };

    auto res = client->Post("/v1/images/generations", body.dump(), "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 400);

    auto resp_json = json::parse(res->body, nullptr, false);
    ASSERT_FALSE(resp_json.is_discarded());
    EXPECT_TRUE(resp_json.contains("error"));
}

// Test: POST /v1/images/generations with valid request (script not available)
TEST_F(ImageEndpointsTest, GenerationsWithValidRequestButNoScriptReturns500) {
    json body = {
        {"model", "z-image-turbo"},
        {"prompt", "A cute cat"}
    };

    auto res = client->Post("/v1/images/generations", body.dump(), "application/json");
    ASSERT_TRUE(res);
    // Should return 500 because Python script is not available
    EXPECT_EQ(res->status, 500);

    auto resp_json = json::parse(res->body, nullptr, false);
    ASSERT_FALSE(resp_json.is_discarded());
    EXPECT_TRUE(resp_json.contains("error"));
}

// Test: POST /v1/images/generations with invalid JSON returns 400
TEST_F(ImageEndpointsTest, GenerationsWithInvalidJsonReturns400) {
    auto res = client->Post("/v1/images/generations", "not json", "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 400);
}

// Test: POST /v1/images/generations accepts size parameter
TEST_F(ImageEndpointsTest, GenerationsAcceptsSizeParameter) {
    json body = {
        {"model", "z-image-turbo"},
        {"prompt", "A cute cat"},
        {"size", "1024x1024"}
    };

    auto res = client->Post("/v1/images/generations", body.dump(), "application/json");
    ASSERT_TRUE(res);
    // Request is valid, but script unavailable
    EXPECT_EQ(res->status, 500);
}

// Test: POST /v1/images/generations accepts optional parameters
TEST_F(ImageEndpointsTest, GenerationsAcceptsOptionalParameters) {
    json body = {
        {"model", "z-image-turbo"},
        {"prompt", "A cute cat"},
        {"n", 1},
        {"size", "512x512"},
        {"quality", "standard"},
        {"style", "vivid"},
        {"response_format", "b64_json"}
    };

    auto res = client->Post("/v1/images/generations", body.dump(), "application/json");
    ASSERT_TRUE(res);
    // Request is valid, but script unavailable
    EXPECT_EQ(res->status, 500);
}

// Test: Invalid size format returns 400
TEST_F(ImageEndpointsTest, GenerationsWithInvalidSizeReturns400) {
    json body = {
        {"model", "z-image-turbo"},
        {"prompt", "A cute cat"},
        {"size", "invalid"}  // Not WxH format
    };

    auto res = client->Post("/v1/images/generations", body.dump(), "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 400);
}
