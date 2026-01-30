#pragma once

#include "core/image_manager.h"
#include <httplib.h>

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <memory>
#include <mutex>
#include <nlohmann/json.hpp>
#include <string>
#include <thread>


namespace xllm {

/// OpenAI Images API compatible endpoints
/// - POST /v1/images/generations (text-to-image)
/// - POST /v1/images/edits (inpainting)
/// - POST /v1/images/variations (image variations)
class ImageEndpoints {
public:
    ImageEndpoints(ImageManager& image_manager);
    ~ImageEndpoints();

    void registerRoutes(httplib::Server& server);

private:
    ImageManager& image_manager_;
    std::string image_dir_;
    std::chrono::seconds image_ttl_{3600};
    std::chrono::seconds cleanup_interval_{60};
    std::atomic<bool> stop_cleanup_{false};
    std::thread cleanup_thread_;
    mutable std::mutex cleanup_mutex_;
    std::condition_variable cleanup_cv_;

    // Helper methods
    static void setJson(httplib::Response& res, const nlohmann::json& body);
    void respondError(httplib::Response& res,
                      int status,
                      const std::string& code,
                      const std::string& message);
    void cleanupExpiredImages() const;
    std::string storeImageAndGetUrl(const httplib::Request& req,
                                    const std::vector<uint8_t>& data,
                                    std::string& error_message) const;
    std::string buildImageUrl(const httplib::Request& req,
                              const std::string& filename) const;

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

}  // namespace xllm
