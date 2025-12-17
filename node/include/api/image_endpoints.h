#pragma once

#include <httplib.h>
#include <nlohmann/json.hpp>
#include <string>
#include <vector>
#include "utils/config.h"

namespace llm_node {

class ImageManager;

/// OpenAI Images API互換エンドポイント
/// - POST /v1/images/generations (Text-to-Image via Python subprocess)
class ImageEndpoints {
public:
    ImageEndpoints(ImageManager& image_manager, const NodeConfig& config);

    void registerRoutes(httplib::Server& server);

private:
    ImageManager& image_manager_;
    const NodeConfig& config_;

    // ヘルパーメソッド
    static void setJson(httplib::Response& res, const nlohmann::json& body);
    void respondError(httplib::Response& res, int status,
                      const std::string& code, const std::string& message);

    // T2I エンドポイントハンドラ (POST /v1/images/generations)
    void handleGenerations(const httplib::Request& req, httplib::Response& res);

    // 画像データをBase64エンコード
    static std::string base64Encode(const std::vector<uint8_t>& data);

    // サイズ文字列をパース (例: "1024x1024" -> width, height)
    static bool parseSize(const std::string& size_str, int& width, int& height);
};

}  // namespace llm_node
