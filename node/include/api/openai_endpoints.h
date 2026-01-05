#pragma once

#include <httplib.h>
#include <string>
#include <memory>
#include <nlohmann/json.hpp>
#include "utils/config.h"
#include "system/gpu_detector.h"

namespace llm_node {

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

}  // namespace llm_node
