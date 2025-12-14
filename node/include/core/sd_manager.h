#pragma once

#include <chrono>
#include <memory>
#include <mutex>
#include <optional>
#include <string>
#include <unordered_map>
#include <vector>

// stable-diffusion.cpp forward declarations
struct sd_ctx_t;

namespace llm_node {

/// Image generation result
struct ImageGenerationResult {
    std::vector<uint8_t> image_data;  // PNG encoded image data
    int width{0};
    int height{0};
    bool success{false};
    std::string error;
};

/// Image generation parameters (OpenAI API compatible)
struct ImageGenParams {
    std::string prompt;
    std::string negative_prompt;
    int width{1024};
    int height{1024};
    int steps{20};
    float cfg_scale{7.0f};
    int64_t seed{-1};  // -1 for random
    int batch_count{1};
    std::string quality;  // "standard" or "hd"
    std::string style;    // "vivid" or "natural"
};

/// Image edit parameters (inpainting)
struct ImageEditParams {
    std::string prompt;
    std::vector<uint8_t> image_data;  // Original image (PNG)
    std::vector<uint8_t> mask_data;   // Mask image (PNG, optional)
    int width{1024};
    int height{1024};
    int steps{20};
    float cfg_scale{7.0f};
    float strength{0.75f};
    int64_t seed{-1};
    int batch_count{1};
};

/// Image variation parameters
struct ImageVariationParams {
    std::vector<uint8_t> image_data;  // Original image (PNG)
    int width{1024};
    int height{1024};
    int steps{20};
    float cfg_scale{7.0f};
    float strength{0.5f};
    int64_t seed{-1};
    int batch_count{1};
};

/// stable-diffusion.cpp manager for image generation
class SDManager {
public:
    explicit SDManager(std::string models_dir);
    ~SDManager();

    // Model loading
    bool loadModel(const std::string& model_path);

    // Check if model is loaded
    bool isLoaded(const std::string& model_path) const;

    // Get context for a model
    sd_ctx_t* getContext(const std::string& model_path) const;

    // Text-to-image generation
    std::vector<ImageGenerationResult> generateImages(
        const std::string& model_path,
        const ImageGenParams& params);

    // Image editing (inpainting)
    std::vector<ImageGenerationResult> editImages(
        const std::string& model_path,
        const ImageEditParams& params);

    // Image variations
    std::vector<ImageGenerationResult> generateVariations(
        const std::string& model_path,
        const ImageVariationParams& params);

    // Loaded model count
    size_t loadedCount() const;

    // Unload a model
    bool unloadModel(const std::string& model_path);

    // Get list of loaded models
    std::vector<std::string> getLoadedModels() const;

    // On-demand loading
    bool loadModelIfNeeded(const std::string& model_path);

    // Idle timeout settings
    void setIdleTimeout(std::chrono::milliseconds timeout);
    std::chrono::milliseconds getIdleTimeout() const;

    // Unload idle models
    size_t unloadIdleModels();

    // Max loaded models settings
    void setMaxLoadedModels(size_t max_models);
    size_t getMaxLoadedModels() const;

    // Check if can load more
    bool canLoadMore() const;

    // Get last access time
    std::optional<std::chrono::steady_clock::time_point> getLastAccessTime(
        const std::string& model_path) const;

private:
    std::string models_dir_;
    mutable std::mutex mutex_;
    std::unordered_map<std::string, sd_ctx_t*> loaded_models_;

    // On-demand loading settings
    std::chrono::milliseconds idle_timeout_{std::chrono::minutes(10)};
    size_t max_loaded_models_{1};  // SD models are very large (8GB+ VRAM)

    // Access time tracking
    std::unordered_map<std::string, std::chrono::steady_clock::time_point>
        last_access_;

    // Canonicalize path
    std::string canonicalizePath(const std::string& path) const;

    // Update access time
    void updateAccessTime(const std::string& model_path);

    // Decode PNG to image data
    bool decodePngToImage(const std::vector<uint8_t>& png_data,
                          std::vector<uint8_t>& out_data,
                          int& out_width,
                          int& out_height) const;
};

}  // namespace llm_node
