#include <gtest/gtest.h>
#include <httplib.h>

#include <filesystem>
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
        base_ = fs::temp_directory_path() / "xllm-audio-integration-XXXXXX";
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

class AudioSpeechIntegrationFixture : public ::testing::Test {
protected:
    void SetUp() override {
        xllm::set_ready(true);

        registry_.setModels({"dummy"});
        server_ = std::make_unique<xllm::HttpServer>(18097, openai_, node_);

        // Provide a large deterministic payload so multiple chunks are emitted
        tts_manager_.setSynthesizeHookForTest(
            [](const std::string&, const std::string&, const xllm::SpeechParams&) {
                xllm::SpeechResult result;
                result.success = true;
                result.format = "wav";
                result.sample_rate = 22050;
                result.channels = 1;
                result.bits_per_sample = 16;
                result.audio_data.resize(256 * 1024);
                for (size_t i = 0; i < result.audio_data.size(); ++i) {
                    result.audio_data[i] = static_cast<uint8_t>((i * 7) % 251);
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

// T309: TTS→ストリーミング受信の統合テスト
TEST_F(AudioSpeechIntegrationFixture, StreamingSpeechDeliversMultipleChunks) {
    httplib::Client cli("127.0.0.1", 18097);

    const std::string body = R"({
        "model": "vibevoice",
        "input": "integration streaming",
        "response_format": "wav",
        "stream": true
    })";

    size_t chunk_calls = 0;
    size_t total_bytes = 0;

    auto res = cli.Post(
        "/v1/audio/speech",
        httplib::Headers{},
        body,
        "application/json",
        [&chunk_calls, &total_bytes](const char* data, size_t len) {
            if (len > 0 && data != nullptr) {
                chunk_calls += 1;
                total_bytes += len;
            }
            return true;
        });

    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    EXPECT_EQ(res->get_header_value("Content-Type"), "audio/wav");

    // チャンク化の確認（1回だけではないこと）
    EXPECT_GT(chunk_calls, 1u);
    EXPECT_GE(total_bytes, 256u * 1024u);
}
