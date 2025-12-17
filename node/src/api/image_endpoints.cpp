#include "api/image_endpoints.h"

#include "core/image_manager.h"

#include <chrono>
#include <regex>

#include <spdlog/spdlog.h>

namespace llm_node {

// Base64エンコーディングテーブル
static const char* base64_chars =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
    "abcdefghijklmnopqrstuvwxyz"
    "0123456789+/";

ImageEndpoints::ImageEndpoints(ImageManager& image_manager, const NodeConfig& config)
    : image_manager_(image_manager), config_(config) {}

void ImageEndpoints::setJson(httplib::Response& res, const nlohmann::json& body) {
    res.set_content(body.dump(), "application/json");
}

void ImageEndpoints::respondError(httplib::Response& res,
                                  int status,
                                  const std::string& code,
                                  const std::string& message) {
    res.status = status;
    setJson(res,
            {{"error",
              {{"message", message},
               {"type", "invalid_request_error"},
               {"code", code}}}});
}

void ImageEndpoints::registerRoutes(httplib::Server& server) {
    // T2I endpoint
    server.Post("/v1/images/generations",
                [this](const httplib::Request& req, httplib::Response& res) {
                    handleGenerations(req, res);
                });

    spdlog::info("Image endpoints registered: /v1/images/generations");
}

std::string ImageEndpoints::base64Encode(const std::vector<uint8_t>& data) {
    std::string result;
    result.reserve(((data.size() + 2) / 3) * 4);

    size_t i = 0;
    while (i < data.size()) {
        uint32_t octet_a = i < data.size() ? data[i++] : 0;
        uint32_t octet_b = i < data.size() ? data[i++] : 0;
        uint32_t octet_c = i < data.size() ? data[i++] : 0;

        uint32_t triple = (octet_a << 16) + (octet_b << 8) + octet_c;

        result += base64_chars[(triple >> 18) & 0x3F];
        result += base64_chars[(triple >> 12) & 0x3F];
        result += base64_chars[(triple >> 6) & 0x3F];
        result += base64_chars[triple & 0x3F];
    }

    // Padding
    size_t mod = data.size() % 3;
    if (mod == 1) {
        result[result.size() - 2] = '=';
        result[result.size() - 1] = '=';
    } else if (mod == 2) {
        result[result.size() - 1] = '=';
    }

    return result;
}

bool ImageEndpoints::parseSize(const std::string& size_str, int& width, int& height) {
    // Format: "WIDTHxHEIGHT" (e.g., "1024x1024", "512x512")
    std::regex size_regex(R"((\d+)x(\d+))");
    std::smatch match;

    if (std::regex_match(size_str, match, size_regex)) {
        try {
            width = std::stoi(match[1].str());
            height = std::stoi(match[2].str());
            return width > 0 && height > 0 && width <= 4096 && height <= 4096;
        } catch (...) {
            return false;
        }
    }
    return false;
}

void ImageEndpoints::handleGenerations(const httplib::Request& req, httplib::Response& res) {
    spdlog::debug("Handling image generation request");

    // JSONボディのパース
    nlohmann::json body;
    try {
        body = nlohmann::json::parse(req.body);
    } catch (const nlohmann::json::parse_error&) {
        respondError(res, 400, "invalid_json", "Invalid JSON body");
        return;
    }

    // 必須フィールドの検証
    if (!body.contains("prompt") || !body["prompt"].is_string()) {
        respondError(res, 400, "missing_prompt", "Missing required field: prompt");
        return;
    }

    std::string prompt = body["prompt"].get<std::string>();
    if (prompt.empty()) {
        respondError(res, 400, "empty_prompt", "Prompt cannot be empty");
        return;
    }

    // オプションフィールドのパース
    std::string model = body.value("model", "z-image-turbo");
    int n = body.value("n", 1);
    std::string size_str = body.value("size", "512x512");
    std::string response_format = body.value("response_format", "b64_json");
    std::string negative_prompt = body.value("negative_prompt", "");

    // サイズのパース
    int width = 512, height = 512;
    if (!parseSize(size_str, width, height)) {
        respondError(res,
                     400,
                     "invalid_size",
                     "Invalid size format. Use WIDTHxHEIGHT (e.g., '512x512')");
        return;
    }

    // 生成枚数の検証
    if (n < 1 || n > 10) {
        respondError(res, 400, "invalid_n", "n must be between 1 and 10");
        return;
    }

    // 現在はn=1のみサポート
    if (n > 1) {
        spdlog::warn(
            "ImageEndpoints: n={} requested, but only n=1 supported. Using n=1.", n);
        n = 1;
    }

    // T2Iパラメータを構築
    T2IParams params;
    params.model = model;
    params.prompt = prompt;
    params.negative_prompt = negative_prompt;
    params.width = width;
    params.height = height;

    // シード値（オプション）
    if (body.contains("seed") && body["seed"].is_number_integer()) {
        params.seed = body["seed"].get<int64_t>();
    }

    spdlog::info(
        "ImageEndpoints: T2I request model={}, prompt_len={}, size={}x{}",
        model,
        prompt.size(),
        width,
        height);

    // 画像生成を実行
    T2IResult result = image_manager_.generateImage(params);

    if (!result.success) {
        spdlog::error("ImageEndpoints: T2I failed: {}", result.error);
        respondError(res, 500, "generation_failed", result.error);
        return;
    }

    // レスポンスを構築
    auto now = std::chrono::system_clock::now();
    auto epoch =
        std::chrono::duration_cast<std::chrono::seconds>(now.time_since_epoch()).count();

    nlohmann::json response = {
        {"created", epoch},
        {"data", nlohmann::json::array()},
    };

    nlohmann::json image_data;
    if (response_format == "b64_json") {
        image_data["b64_json"] = base64Encode(result.image_data);
    } else if (response_format == "url") {
        // URL形式は将来サポート予定
        // 現在はローカルファイルパスを返す（開発用）
        image_data["url"] = "file://" + result.image_path;
    } else {
        image_data["b64_json"] = base64Encode(result.image_data);
    }

    image_data["revised_prompt"] = prompt;
    response["data"].push_back(image_data);

    res.status = 200;
    setJson(res, response);

    spdlog::info("ImageEndpoints: T2I success, response_size={} bytes", res.body.size());
}

}  // namespace llm_node

