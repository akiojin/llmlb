#include "api/image_endpoints.h"

#include "core/image_manager.h"

#include <spdlog/spdlog.h>

#include <algorithm>
#include <chrono>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <random>
#include <regex>

namespace xllm {

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

std::string getHomeDir() {
    if (const char* home = std::getenv("HOME")) {
        return home;
    }
    if (const char* userprofile = std::getenv("USERPROFILE")) {
        return userprofile;
    }
    return "/tmp";
}

std::string getImageDirFromEnv() {
    if (const char* env = std::getenv("XLLM_IMAGE_DIR")) {
        if (env[0] != '\0') {
            return env;
        }
    }
    return (std::filesystem::path(getHomeDir()) / ".xllm" / "images").string();
}

std::chrono::seconds getImageTtlFromEnv() {
    constexpr int64_t kDefaultTtlSeconds = 60 * 60;
    if (const char* env = std::getenv("XLLM_IMAGE_TTL_SECONDS")) {
        try {
            int64_t value = std::stoll(env);
            if (value > 0) {
                return std::chrono::seconds(value);
            }
        } catch (...) {
        }
    }
    return std::chrono::seconds(kDefaultTtlSeconds);
}

std::chrono::seconds getCleanupInterval(std::chrono::seconds ttl) {
    if (const char* env = std::getenv("XLLM_IMAGE_CLEANUP_INTERVAL_SECONDS")) {
        try {
            int64_t value = std::stoll(env);
            if (value > 0) {
                return std::chrono::seconds(value);
            }
        } catch (...) {
        }
    }
    auto interval = std::chrono::seconds(
        std::max<int64_t>(1, std::min<int64_t>(60, ttl.count())));
    return interval;
}

std::string randomHex(size_t bytes = 8) {
    std::random_device rd;
    std::uniform_int_distribution<int> dist(0, 255);
    std::string out;
    out.reserve(bytes * 2);
    constexpr char kHex[] = "0123456789abcdef";
    for (size_t i = 0; i < bytes; ++i) {
        int v = dist(rd);
        out.push_back(kHex[(v >> 4) & 0xF]);
        out.push_back(kHex[v & 0xF]);
    }
    return out;
}

std::string makeImageFilename() {
    auto now = std::chrono::system_clock::now();
    auto seconds = std::chrono::duration_cast<std::chrono::seconds>(
        now.time_since_epoch()).count();
    return "img_" + std::to_string(seconds) + "_" + randomHex(6) + ".png";
}

bool isValidResponseFormat(const std::string& format) {
    return format == "url" || format == "b64_json";
}

}  // namespace

ImageEndpoints::ImageEndpoints(ImageManager& image_manager)
    : image_manager_(image_manager),
      image_dir_(getImageDirFromEnv()),
      image_ttl_(getImageTtlFromEnv()),
      cleanup_interval_(getCleanupInterval(image_ttl_)) {
    std::error_code ec;
    std::filesystem::create_directories(image_dir_, ec);
    if (ec) {
        spdlog::warn("Failed to create image directory {}: {}", image_dir_, ec.message());
    }

    cleanup_thread_ = std::thread([this]() {
        std::unique_lock<std::mutex> lock(cleanup_mutex_);
        while (!stop_cleanup_) {
            lock.unlock();
            cleanupExpiredImages();
            lock.lock();
            cleanup_cv_.wait_for(lock, cleanup_interval_, [this] {
                return stop_cleanup_.load();
            });
        }
    });
}

ImageEndpoints::~ImageEndpoints() {
    stop_cleanup_.store(true);
    cleanup_cv_.notify_all();
    if (cleanup_thread_.joinable()) {
        cleanup_thread_.join();
    }
}

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

    if (!server.set_mount_point("/images", image_dir_.c_str())) {
        spdlog::warn("Failed to mount /images to {}", image_dir_);
    }

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

void ImageEndpoints::cleanupExpiredImages() const {
    std::error_code ec;
    if (!std::filesystem::exists(image_dir_, ec)) {
        return;
    }

    auto now = std::filesystem::file_time_type::clock::now();
    auto cutoff = now - image_ttl_;

    for (const auto& entry : std::filesystem::directory_iterator(image_dir_, ec)) {
        if (ec) {
            return;
        }
        if (!entry.is_regular_file()) {
            continue;
        }
        std::error_code time_ec;
        auto write_time = entry.last_write_time(time_ec);
        if (time_ec) {
            continue;
        }
        if (write_time < cutoff) {
            std::error_code rm_ec;
            std::filesystem::remove(entry.path(), rm_ec);
        }
    }
}

std::string ImageEndpoints::buildImageUrl(const httplib::Request& req,
                                          const std::string& filename) const {
    std::string host = req.get_header_value("Host");
    if (host.empty()) {
        host = "localhost";
    }
    return "http://" + host + "/images/" + filename;
}

std::string ImageEndpoints::storeImageAndGetUrl(const httplib::Request& req,
                                                const std::vector<uint8_t>& data,
                                                std::string& error_message) const {
    std::error_code ec;
    std::filesystem::create_directories(image_dir_, ec);
    if (ec) {
        error_message = "Failed to create image directory";
        return {};
    }

    std::string filename = makeImageFilename();
    std::filesystem::path output_path = std::filesystem::path(image_dir_) / filename;

    std::ofstream out(output_path, std::ios::binary);
    if (!out.is_open()) {
        error_message = "Failed to open image file for writing";
        return {};
    }

    out.write(reinterpret_cast<const char*>(data.data()),
              static_cast<std::streamsize>(data.size()));
    out.close();

    if (!out) {
        error_message = "Failed to write image data";
        return {};
    }

    cleanupExpiredImages();
    return buildImageUrl(req, filename);
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
    if (!isValidResponseFormat(response_format)) {
        respondError(res, 400, "invalid_response_format",
                     "response_format must be 'url' or 'b64_json'");
        return;
    }
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
    if (!image_manager_.loadModelIfNeeded(model)) {
        respondError(res, 500, "model_load_failed",
                     "Failed to load model: " + model);
        return;
    }

    // Generate images
    auto results = image_manager_.generateImages(model, params);

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
            std::string error_message;
            std::string url = storeImageAndGetUrl(req, result.image_data, error_message);
            if (url.empty()) {
                spdlog::warn("Failed to store image: {}", error_message);
                continue;
            }
            image_obj["url"] = url;
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
    if (!isValidResponseFormat(response_format)) {
        respondError(res, 400, "invalid_response_format",
                     "response_format must be 'url' or 'b64_json'");
        return;
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
    if (!image_manager_.loadModelIfNeeded(model)) {
        respondError(res, 500, "model_load_failed",
                     "Failed to load model: " + model);
        return;
    }

    // Edit images
    auto results = image_manager_.editImages(model, params);

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
            std::string error_message;
            std::string url = storeImageAndGetUrl(req, result.image_data, error_message);
            if (url.empty()) {
                spdlog::warn("Failed to store edited image: {}", error_message);
                continue;
            }
            image_obj["url"] = url;
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
    if (!isValidResponseFormat(response_format)) {
        respondError(res, 400, "invalid_response_format",
                     "response_format must be 'url' or 'b64_json'");
        return;
    }

    // Prepare variation parameters
    ImageVariationParams params;
    params.image_data.assign(image_file.content.begin(), image_file.content.end());
    params.width = width;
    params.height = height;
    params.batch_count = n;

    // Load model if needed
    if (!image_manager_.loadModelIfNeeded(model)) {
        respondError(res, 500, "model_load_failed",
                     "Failed to load model: " + model);
        return;
    }

    // Generate variations
    auto results = image_manager_.generateVariations(model, params);

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
            std::string error_message;
            std::string url = storeImageAndGetUrl(req, result.image_data, error_message);
            if (url.empty()) {
                spdlog::warn("Failed to store image variation: {}", error_message);
                continue;
            }
            image_obj["url"] = url;
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

}  // namespace xllm
