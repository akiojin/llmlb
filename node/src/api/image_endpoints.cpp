#include "api/image_endpoints.h"

#include "core/sd_manager.h"

#include <spdlog/spdlog.h>

#include <algorithm>
#include <chrono>
#include <regex>

namespace llm_node {

namespace {

// Base64 encoding table
const char kBase64Chars[] =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

std::string extractFirstError(const std::vector<ImageGenerationResult>& results,
                              const std::string& default_message) {
    for (const auto& result : results) {
        if (!result.error.empty()) {
            return result.error;
        }
    }
    return default_message;
}

}  // namespace

ImageEndpoints::ImageEndpoints(SDManager& sd_manager, const NodeConfig& config)
    : sd_manager_(sd_manager), config_(config) {}

void ImageEndpoints::setJson(httplib::Response& res, const nlohmann::json& body) {
    res.set_content(body.dump(), "application/json");
}

void ImageEndpoints::respondError(httplib::Response& res,
                                  int status,
                                  const std::string& code,
                                  const std::string& message) {
    res.status = status;
    setJson(res, {{"error",
                   {{"message", message},
                    {"type", "invalid_request_error"},
                    {"code", code}}}});
}

void ImageEndpoints::registerRoutes(httplib::Server& server) {
    // Text-to-image generation
    server.Post(
        "/v1/images/generations",
        [this](const httplib::Request& req, httplib::Response& res) {
            handleGenerations(req, res);
        });

    // Image editing (inpainting)
    server.Post("/v1/images/edits",
                [this](const httplib::Request& req, httplib::Response& res) {
                    handleEdits(req, res);
                });

    // Image variations
    server.Post("/v1/images/variations",
                [this](const httplib::Request& req, httplib::Response& res) {
                    handleVariations(req, res);
                });

    spdlog::info(
        "Image endpoints registered: /v1/images/generations, "
        "/v1/images/edits, /v1/images/variations");
}

bool ImageEndpoints::parseImageSize(const std::string& size_str,
                                    int& width,
                                    int& height) {
    // Parse "WIDTHxHEIGHT" format (e.g., "1024x1024")
    std::regex size_regex(R"(^(\d+)x(\d+)$)");
    std::smatch match;

    if (std::regex_match(size_str, match, size_regex)) {
        width = std::stoi(match[1]);
        height = std::stoi(match[2]);
        return true;
    }
    return false;
}

std::string ImageEndpoints::encodeBase64(const std::vector<uint8_t>& data) {
    std::string result;
    result.reserve(((data.size() + 2) / 3) * 4);

    size_t i = 0;
    while (i < data.size()) {
        uint32_t octet_a = i < data.size() ? data[i++] : 0;
        uint32_t octet_b = i < data.size() ? data[i++] : 0;
        uint32_t octet_c = i < data.size() ? data[i++] : 0;

        uint32_t triple = (octet_a << 16) | (octet_b << 8) | octet_c;

        result += kBase64Chars[(triple >> 18) & 0x3F];
        result += kBase64Chars[(triple >> 12) & 0x3F];
        result += kBase64Chars[(triple >> 6) & 0x3F];
        result += kBase64Chars[triple & 0x3F];
    }

    // Add padding
    size_t padding = data.size() % 3;
    if (padding > 0) {
        for (size_t p = 0; p < 3 - padding; ++p) {
            result[result.size() - 1 - p] = '=';
        }
    }

    return result;
}

int64_t ImageEndpoints::getCurrentTimestamp() {
    return std::chrono::duration_cast<std::chrono::seconds>(
               std::chrono::system_clock::now().time_since_epoch())
        .count();
}

void ImageEndpoints::handleGenerations(const httplib::Request& req,
                                       httplib::Response& res) {
    spdlog::debug("Handling image generation request");

    // Parse JSON request body
    nlohmann::json body;
    try {
        body = nlohmann::json::parse(req.body);
    } catch (const std::exception& e) {
        respondError(res, 400, "invalid_json",
                     std::string("Invalid JSON: ") + e.what());
        return;
    }

    // Required: prompt
    if (!body.contains("prompt") || !body["prompt"].is_string()) {
        respondError(res, 400, "missing_prompt", "Missing required field: prompt");
        return;
    }
    std::string prompt = body["prompt"];

    if (prompt.empty()) {
        respondError(res, 400, "empty_prompt", "Prompt cannot be empty");
        return;
    }

    // Required: model
    if (!body.contains("model") || !body["model"].is_string()) {
        respondError(res, 400, "missing_model", "Missing required field: model");
        return;
    }
    std::string model = body["model"];

    // Optional parameters
    int n = body.value("n", 1);
    if (n < 1 || n > 10) {
        respondError(res, 400, "invalid_n", "n must be between 1 and 10");
        return;
    }

    std::string size_str = body.value("size", "1024x1024");
    int width = 1024;
    int height = 1024;
    if (!parseImageSize(size_str, width, height)) {
        respondError(res, 400, "invalid_size",
                     "Invalid size format. Expected WIDTHxHEIGHT (e.g., 1024x1024)");
        return;
    }

    std::string quality = body.value("quality", "standard");
    std::string style = body.value("style", "vivid");
    std::string response_format = body.value("response_format", "url");
    int steps = body.value("steps", 20);
    if (steps < 1 || steps > 100) {
        respondError(res, 400, "invalid_steps", "steps must be between 1 and 100");
        return;
    }

    // Prepare generation parameters
    ImageGenParams params;
    params.prompt = prompt;
    params.width = width;
    params.height = height;
    params.batch_count = n;
    params.quality = quality;
    params.style = style;
    params.steps = steps;

    // Load model if needed
    if (!sd_manager_.loadModelIfNeeded(model)) {
        respondError(res, 500, "model_load_failed",
                     "Failed to load model: " + model);
        return;
    }

    // Generate images
    auto results = sd_manager_.generateImages(model, params);

    if (results.empty()) {
        respondError(res, 500, "generation_failed", "Image generation failed");
        return;
    }

    // Build response
    nlohmann::json data_array = nlohmann::json::array();
    for (const auto& result : results) {
        if (!result.success) {
            spdlog::warn("Image generation partially failed: {}", result.error);
            continue;
        }

        nlohmann::json image_obj;
        if (response_format == "b64_json") {
            image_obj["b64_json"] = encodeBase64(result.image_data);
        } else {
            // For URL format, we would need to save the image and return a URL
            // For now, fall back to base64 in the response
            // TODO: Implement image storage and URL generation
            image_obj["b64_json"] = encodeBase64(result.image_data);
            spdlog::warn("URL response format not yet implemented, returning base64");
        }
        data_array.push_back(image_obj);
    }

    if (data_array.empty()) {
        respondError(res,
                     500,
                     "all_generations_failed",
                     extractFirstError(results, "All image generations failed"));
        return;
    }

    nlohmann::json response = {
        {"created", getCurrentTimestamp()},
        {"data", data_array},
    };

    setJson(res, response);
    spdlog::info("Generated {} images for prompt: {}",
                 data_array.size(), prompt.substr(0, 50));
}

void ImageEndpoints::handleEdits(const httplib::Request& req,
                                 httplib::Response& res) {
    spdlog::debug("Handling image edit request");

    // multipart/form-data validation
    if (!req.form.has_file("image")) {
        respondError(res, 400, "missing_image", "Missing required field: image");
        return;
    }

    const auto image_file = req.form.get_file("image");
    if (image_file.content.empty()) {
        respondError(res, 400, "empty_image", "Image file is empty");
        return;
    }

    // Check file size (max 4MB)
    if (image_file.content.size() > 4 * 1024 * 1024) {
        respondError(res, 400, "image_too_large",
                     "Image file exceeds 4MB limit");
        return;
    }

    // Required: prompt
    std::string prompt;
    if (req.form.has_field("prompt")) {
        prompt = req.form.get_field("prompt");
    } else {
        respondError(res, 400, "missing_prompt", "Missing required field: prompt");
        return;
    }

    // Required: model
    std::string model;
    if (req.form.has_field("model")) {
        model = req.form.get_field("model");
    } else {
        respondError(res, 400, "missing_model", "Missing required field: model");
        return;
    }

    // Optional: mask
    std::vector<uint8_t> mask_data;
    if (req.form.has_file("mask")) {
        const auto mask_file = req.form.get_file("mask");
        mask_data.assign(mask_file.content.begin(), mask_file.content.end());
    }

    // Optional parameters
    int n = 1;
    if (req.form.has_field("n")) {
        n = std::stoi(req.form.get_field("n"));
        if (n < 1 || n > 10) {
            respondError(res, 400, "invalid_n", "n must be between 1 and 10");
            return;
        }
    }

    int width = 1024;
    int height = 1024;
    if (req.form.has_field("size")) {
        if (!parseImageSize(req.form.get_field("size"), width, height)) {
            respondError(
                res, 400, "invalid_size",
                "Invalid size format. Expected WIDTHxHEIGHT (e.g., 1024x1024)");
            return;
        }
    }

    std::string response_format = "url";
    if (req.form.has_field("response_format")) {
        response_format = req.form.get_field("response_format");
    }

    // Prepare edit parameters
    ImageEditParams params;
    params.prompt = prompt;
    params.image_data.assign(image_file.content.begin(), image_file.content.end());
    params.mask_data = mask_data;
    params.width = width;
    params.height = height;
    params.batch_count = n;

    // Load model if needed
    if (!sd_manager_.loadModelIfNeeded(model)) {
        respondError(res, 500, "model_load_failed",
                     "Failed to load model: " + model);
        return;
    }

    // Edit images
    auto results = sd_manager_.editImages(model, params);

    if (results.empty()) {
        respondError(res, 500, "edit_failed", "Image editing failed");
        return;
    }

    // Build response
    nlohmann::json data_array = nlohmann::json::array();
    for (const auto& result : results) {
        if (!result.success) {
            spdlog::warn("Image editing partially failed: {}", result.error);
            continue;
        }

        nlohmann::json image_obj;
        if (response_format == "b64_json") {
            image_obj["b64_json"] = encodeBase64(result.image_data);
        } else {
            image_obj["b64_json"] = encodeBase64(result.image_data);
            spdlog::warn("URL response format not yet implemented, returning base64");
        }
        data_array.push_back(image_obj);
    }

    if (data_array.empty()) {
        respondError(res,
                     500,
                     "all_edits_failed",
                     extractFirstError(results, "All image edits failed"));
        return;
    }

    nlohmann::json response = {
        {"created", getCurrentTimestamp()},
        {"data", data_array},
    };

    setJson(res, response);
    spdlog::info("Edited {} images", data_array.size());
}

void ImageEndpoints::handleVariations(const httplib::Request& req,
                                      httplib::Response& res) {
    spdlog::debug("Handling image variation request");

    // multipart/form-data validation
    if (!req.form.has_file("image")) {
        respondError(res, 400, "missing_image", "Missing required field: image");
        return;
    }

    const auto image_file = req.form.get_file("image");
    if (image_file.content.empty()) {
        respondError(res, 400, "empty_image", "Image file is empty");
        return;
    }

    // Check file size (max 4MB)
    if (image_file.content.size() > 4 * 1024 * 1024) {
        respondError(res, 400, "image_too_large",
                     "Image file exceeds 4MB limit");
        return;
    }

    // Required: model
    std::string model;
    if (req.form.has_field("model")) {
        model = req.form.get_field("model");
    } else {
        respondError(res, 400, "missing_model", "Missing required field: model");
        return;
    }

    // Optional parameters
    int n = 1;
    if (req.form.has_field("n")) {
        n = std::stoi(req.form.get_field("n"));
        if (n < 1 || n > 10) {
            respondError(res, 400, "invalid_n", "n must be between 1 and 10");
            return;
        }
    }

    int width = 1024;
    int height = 1024;
    if (req.form.has_field("size")) {
        if (!parseImageSize(req.form.get_field("size"), width, height)) {
            respondError(
                res, 400, "invalid_size",
                "Invalid size format. Expected WIDTHxHEIGHT (e.g., 1024x1024)");
            return;
        }
    }

    std::string response_format = "url";
    if (req.form.has_field("response_format")) {
        response_format = req.form.get_field("response_format");
    }

    // Prepare variation parameters
    ImageVariationParams params;
    params.image_data.assign(image_file.content.begin(), image_file.content.end());
    params.width = width;
    params.height = height;
    params.batch_count = n;

    // Load model if needed
    if (!sd_manager_.loadModelIfNeeded(model)) {
        respondError(res, 500, "model_load_failed",
                     "Failed to load model: " + model);
        return;
    }

    // Generate variations
    auto results = sd_manager_.generateVariations(model, params);

    if (results.empty()) {
        respondError(res, 500, "variation_failed",
                     "Image variation generation failed");
        return;
    }

    // Build response
    nlohmann::json data_array = nlohmann::json::array();
    for (const auto& result : results) {
        if (!result.success) {
            spdlog::warn("Image variation partially failed: {}", result.error);
            continue;
        }

        nlohmann::json image_obj;
        if (response_format == "b64_json") {
            image_obj["b64_json"] = encodeBase64(result.image_data);
        } else {
            image_obj["b64_json"] = encodeBase64(result.image_data);
            spdlog::warn("URL response format not yet implemented, returning base64");
        }
        data_array.push_back(image_obj);
    }

    if (data_array.empty()) {
        respondError(res,
                     500,
                     "all_variations_failed",
                     extractFirstError(results, "All image variations failed"));
        return;
    }

    nlohmann::json response = {
        {"created", getCurrentTimestamp()},
        {"data", data_array},
    };

    setJson(res, response);
    spdlog::info("Generated {} image variations", data_array.size());
}

}  // namespace llm_node
