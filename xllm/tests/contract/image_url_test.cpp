#include <gtest/gtest.h>
#include <httplib.h>
#include <nlohmann/json.hpp>

#include <chrono>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <string>

#include "api/http_server.h"
#include "api/image_endpoints.h"
#include "api/node_endpoints.h"
#include "api/openai_endpoints.h"
#include "core/inference_engine.h"
#include "core/sd_manager.h"
#include "models/model_registry.h"
#include "runtime/state.h"
#include "utils/config.h"

using namespace xllm;
using json = nlohmann::json;

namespace {
std::vector<uint8_t> samplePng() {
    return {
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00,
        0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01,
        0x00, 0x00, 0x00, 0x01, 0x08, 0x04, 0x00, 0x00, 0x00, 0xB5,
        0x1C, 0x0C, 0x02, 0x00, 0x00, 0x00, 0x0B, 0x49, 0x44, 0x41,
        0x54, 0x78, 0x9C, 0x63, 0xF8, 0x0F, 0x00, 0x01, 0x01, 0x01,
        0x00, 0x18, 0xDD, 0x8D, 0xBC, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    };
}

void setEnvVar(const char* key, const std::string& value) {
#ifdef _WIN32
    _putenv_s(key, value.c_str());
#else
    setenv(key, value.c_str(), 1);
#endif
}

void unsetEnvVar(const char* key) {
#ifdef _WIN32
    _putenv_s(key, "");
#else
    unsetenv(key);
#endif
}

std::string uniqueTempDir(const std::string& prefix) {
    auto base = std::filesystem::temp_directory_path();
    auto now = std::chrono::duration_cast<std::chrono::microseconds>(
                   std::chrono::system_clock::now().time_since_epoch())
                   .count();
    return (base / (prefix + std::to_string(now))).string();
}
}

class ImageContractFixture : public ::testing::Test {
protected:
    void SetUp() override {
        xllm::set_ready(true);
        temp_dir_ = uniqueTempDir("xllm-image-contract-");
        std::filesystem::create_directories(temp_dir_);

        prev_image_dir_ = std::getenv("XLLM_IMAGE_DIR") ? std::getenv("XLLM_IMAGE_DIR") : "";
        prev_ttl_ = std::getenv("XLLM_IMAGE_TTL_SECONDS") ? std::getenv("XLLM_IMAGE_TTL_SECONDS") : "";
        prev_interval_ = std::getenv("XLLM_IMAGE_CLEANUP_INTERVAL_SECONDS")
                             ? std::getenv("XLLM_IMAGE_CLEANUP_INTERVAL_SECONDS")
                             : "";

        setEnvVar("XLLM_IMAGE_DIR", temp_dir_);
        setEnvVar("XLLM_IMAGE_TTL_SECONDS", "1");
        setEnvVar("XLLM_IMAGE_CLEANUP_INTERVAL_SECONDS", "1");

        registry_.setModels({"sd-test"});
        image_manager_ = std::make_unique<SDManager>(temp_dir_);
        image_manager_->setGenerateHookForTest(
            [](const std::string&, const ImageGenParams& params) {
                ImageGenerationResult result;
                result.success = true;
                result.width = params.width;
                result.height = params.height;
                result.image_data = samplePng();
                return std::vector<ImageGenerationResult>{result};
            });

        openai_ = std::make_unique<OpenAIEndpoints>(registry_, engine_, config_, GpuBackend::Cpu);
        node_ = std::make_unique<NodeEndpoints>();
        server_ = std::make_unique<HttpServer>(18092, *openai_, *node_);
        image_endpoints_ = std::make_unique<ImageEndpoints>(*image_manager_);
        image_endpoints_->registerRoutes(server_->getServer());
        server_->start();
    }

    void TearDown() override {
        if (server_) {
            server_->stop();
        }
        image_endpoints_.reset();
        image_manager_.reset();
        server_.reset();
        openai_.reset();
        node_.reset();

        if (!prev_image_dir_.empty()) {
            setEnvVar("XLLM_IMAGE_DIR", prev_image_dir_);
        } else {
            unsetEnvVar("XLLM_IMAGE_DIR");
        }
        if (!prev_ttl_.empty()) {
            setEnvVar("XLLM_IMAGE_TTL_SECONDS", prev_ttl_);
        } else {
            unsetEnvVar("XLLM_IMAGE_TTL_SECONDS");
        }
        if (!prev_interval_.empty()) {
            setEnvVar("XLLM_IMAGE_CLEANUP_INTERVAL_SECONDS", prev_interval_);
        } else {
            unsetEnvVar("XLLM_IMAGE_CLEANUP_INTERVAL_SECONDS");
        }

        std::error_code ec;
        std::filesystem::remove_all(temp_dir_, ec);
    }

    std::string temp_dir_;
    std::string prev_image_dir_;
    std::string prev_ttl_;
    std::string prev_interval_;

    ModelRegistry registry_;
    InferenceEngine engine_;
    NodeConfig config_;

    std::unique_ptr<SDManager> image_manager_;
    std::unique_ptr<OpenAIEndpoints> openai_;
    std::unique_ptr<NodeEndpoints> node_;
    std::unique_ptr<HttpServer> server_;
    std::unique_ptr<ImageEndpoints> image_endpoints_;
};

TEST_F(ImageContractFixture, ImageGenerationReturnsUrl) {
    httplib::Client cli("127.0.0.1", 18092);
    std::string body = R"({"model":"sd-test","prompt":"cat","response_format":"url"})";
    auto res = cli.Post("/v1/images/generations", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);

    auto j = json::parse(res->body);
    ASSERT_TRUE(j.contains("data"));
    ASSERT_FALSE(j["data"].empty());
    ASSERT_TRUE(j["data"][0].contains("url"));

    std::string url = j["data"][0]["url"].get<std::string>();
    auto pos = url.find("/images/");
    ASSERT_NE(pos, std::string::npos);
    std::string path = url.substr(pos);

    auto img_res = cli.Get(path.c_str());
    ASSERT_TRUE(img_res);
    EXPECT_EQ(img_res->status, 200);
    EXPECT_FALSE(img_res->body.empty());
}

TEST_F(ImageContractFixture, ImageTtlRemovesExpiredFiles) {
    std::filesystem::path expired_path = std::filesystem::path(temp_dir_) / "expired.png";
    {
        std::ofstream out(expired_path, std::ios::binary);
        auto png = samplePng();
        out.write(reinterpret_cast<const char*>(png.data()),
                  static_cast<std::streamsize>(png.size()));
    }

    auto past = std::filesystem::file_time_type::clock::now() - std::chrono::seconds(5);
    std::filesystem::last_write_time(expired_path, past);
    ASSERT_TRUE(std::filesystem::exists(expired_path));

    httplib::Client cli("127.0.0.1", 18092);
    std::string body = R"({"model":"sd-test","prompt":"dog","response_format":"url"})";
    auto res = cli.Post("/v1/images/generations", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);

    auto j = json::parse(res->body);
    std::string url = j["data"][0]["url"].get<std::string>();
    auto pos = url.find("/images/");
    ASSERT_NE(pos, std::string::npos);
    std::string filename = url.substr(pos + std::string("/images/").size());
    std::filesystem::path new_path = std::filesystem::path(temp_dir_) / filename;

    EXPECT_TRUE(std::filesystem::exists(new_path));
    EXPECT_FALSE(std::filesystem::exists(expired_path));
}
