#pragma once

#include <httplib.h>

#include <memory>
#include <nlohmann/json.hpp>
#include <string>

#include "utils/config.h"

namespace llm_node {

class SDManager;

/// OpenAI Images API compatible endpoints
/// - POST /v1/images/generations (text-to-image)
/// - POST /v1/images/edits (inpainting)
/// - POST /v1/images/variations (image variations)
class ImageEndpoints {
public:
    ImageEndpoints(SDManager& sd_manager, const NodeConfig& config);

    void registerRoutes(httplib::Server& server);

private:
    SDManager& sd_manager_;
    [[maybe_unused]] const NodeConfig& config_;

    // Helper methods
    static void setJson(httplib::Response& res, const nlohmann::json& body);
    void respondError(httplib::Response& res,
                      int status,
                      const std::string& code,
                      const std::string& message);

    // Endpoint handlers
    void handleGenerations(const httplib::Request& req, httplib::Response& res);
    void handleEdits(const httplib::Request& req, httplib::Response& res);
    void handleVariations(const httplib::Request& req, httplib::Response& res);

    // Parse image size string (e.g., "1024x1024") to width and height
    bool parseImageSize(const std::string& size_str, int& width, int& height);

    // Encode image data to base64
    std::string encodeBase64(const std::vector<uint8_t>& data);

    // Get current Unix timestamp
    static int64_t getCurrentTimestamp();
};

}  // namespace llm_node
