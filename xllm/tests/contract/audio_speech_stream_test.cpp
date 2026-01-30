#include <gtest/gtest.h>
#include <httplib.h>

#include <filesystem>
#include <fstream>
#include <memory>
#include <string>
#include <vector>

#include "api/audio_endpoints.h"
#include "api/http_server.h"
#include "api/node_endpoints.h"
#include "api/openai_endpoints.h"
#include "core/audio_manager.h"
#include "core/inference_engine.h"
#include "core/onnx_tts_manager.h"
#include "models/model_registry.h"
#include "runtime/state.h"
#include "utils/config.h"

namespace fs = std::filesystem;

namespace {

class TempDir {
public:
    TempDir() {
        base_ = fs::temp_directory_path() / "xllm-audio-contract-XXXXXX";
        std::string tmpl = base_.string();
        std::vector<char> buf(tmpl.begin(), tmpl.end());
        buf.push_back('\0');
        char* created = mkdtemp(buf.data());
        base_ = created ? fs::path(created) : fs::temp_directory_path();
        fs::create_directories(base_);
    }

    ~TempDir() {
        std::error_code ec;
        fs::remove_all(base_, ec);
    }

    const fs::path& path() const { return base_; }

private:
    fs::path base_;
};

class AudioSpeechContractFixture : public ::testing::Test {
protected:
    void SetUp() override {
        xllm::set_ready(true);

        // Minimal server components required by HttpServer
        registry_.setModels({"dummy"});
        server_ = std::make_unique<xllm::HttpServer>(18096, openai_, node_);

        // Provide deterministic TTS output for contract tests
        tts_manager_.setSynthesizeHookForTest(
            [](const std::string&, const std::string&, const xllm::SpeechParams&) {
                xllm::SpeechResult result;
                result.success = true;
                result.format = "wav";
                result.sample_rate = 22050;
                result.channels = 1;
                result.bits_per_sample = 16;
                result.audio_data.resize(48 * 1024);
                for (size_t i = 0; i < result.audio_data.size(); ++i) {
                    result.audio_data[i] = static_cast<uint8_t>(i % 251);
                }
                return result;
            });

        audio_endpoints_ = std::make_unique<xllm::AudioEndpoints>(audio_manager_, tts_manager_);
        audio_endpoints_->registerRoutes(server_->getServer());
        server_->start();
    }

    void TearDown() override {
        server_->stop();
    }

    TempDir tmp_;
    xllm::AudioManager audio_manager_{tmp_.path().string()};
    xllm::OnnxTtsManager tts_manager_{tmp_.path().string()};

    xllm::ModelRegistry registry_;
    xllm::InferenceEngine engine_{};
    xllm::NodeConfig config_{};
    xllm::OpenAIEndpoints openai_{registry_, engine_, config_, xllm::GpuBackend::Cpu};
    xllm::NodeEndpoints node_{};

    std::unique_ptr<xllm::HttpServer> server_;
    std::unique_ptr<xllm::AudioEndpoints> audio_endpoints_;
};

}  // namespace

// T301: TTSストリーミングの契約テスト
TEST_F(AudioSpeechContractFixture, SpeechEndpointStreamsAudioWhenRequested) {
    httplib::Client cli("127.0.0.1", 18096);
    const std::string body = R"({
        "model": "vibevoice",
        "input": "hello streaming",
        "response_format": "wav",
        "stream": true
    })";

    auto res = cli.Post("/v1/audio/speech", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    EXPECT_EQ(res->get_header_value("Content-Type"), "audio/wav");

    // Contract: streaming still yields the complete audio payload
    EXPECT_GE(res->body.size(), 48u * 1024u);
}

TEST_F(AudioSpeechContractFixture, SpeechEndpointReturnsAudioWithoutStreaming) {
    httplib::Client cli("127.0.0.1", 18096);
    const std::string body = R"({
        "model": "vibevoice",
        "input": "hello",
        "response_format": "wav"
    })";

    auto res = cli.Post("/v1/audio/speech", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    EXPECT_EQ(res->get_header_value("Content-Type"), "audio/wav");
    EXPECT_GE(res->body.size(), 48u * 1024u);
}
