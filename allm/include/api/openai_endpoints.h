#pragma once

#include <httplib.h>
#include <string>
#include <memory>
#include <vector>
#include <cstdint>
#include <nlohmann/json.hpp>
#include "utils/config.h"
#include "system/gpu_detector.h"

namespace allm {

// OpenAI互換APIのトークン使用量
struct TokenUsage {
    int prompt_tokens{0};
    int completion_tokens{0};
    int total_tokens{0};
};

// 上位候補トークンの確率情報
struct TopLogprob {
    std::string token;
    float logprob{0.0f};
    std::vector<uint8_t> bytes;
};

// トークンの確率情報
struct LogprobInfo {
    std::string token;
    float logprob{0.0f};
    std::vector<uint8_t> bytes;
    std::vector<TopLogprob> top_logprobs;
};

// 一意のレスポンスIDを生成
std::string generate_response_id(const std::string& prefix);

// 現在のUNIXタイムスタンプを取得
int64_t get_current_timestamp();

class ModelRegistry;
class InferenceEngine;

class OpenAIEndpoints {
public:
    OpenAIEndpoints(ModelRegistry& registry, InferenceEngine& engine, const NodeConfig& config, GpuBackend backend);

    void registerRoutes(httplib::Server& server);

private:
    ModelRegistry& registry_;
    InferenceEngine& engine_;
    [[maybe_unused]] const NodeConfig& config_;
    GpuBackend backend_;

    static void setJson(httplib::Response& res, const nlohmann::json& body);
    void respondError(httplib::Response& res, int status, const std::string& code, const std::string& message);
    bool validateModel(const std::string& model, const std::string& capability, httplib::Response& res);
};

}  // namespace allm
