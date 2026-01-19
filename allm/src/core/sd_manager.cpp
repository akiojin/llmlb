#include "core/sd_manager.h"

#include <spdlog/spdlog.h>

#ifdef USE_SD

// Include stable-diffusion.h FIRST to ensure sd_image_t typedef is defined
// before the forward declaration in sd_manager.h is seen
#include <stable-diffusion.h>

#include <algorithm>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <limits>
#include <random>

// stb_image for PNG encoding/decoding
#define STB_IMAGE_IMPLEMENTATION
#define STB_IMAGE_WRITE_IMPLEMENTATION
#include "thirdparty/stb_image.h"
#include "thirdparty/stb_image_write.h"

namespace allm {

namespace {

// Callback for stb_image_write to write to vector
void stbiWriteCallback(void* context, void* data, int size) {
    auto* vec = static_cast<std::vector<uint8_t>*>(context);
    auto* bytes = static_cast<uint8_t*>(data);
    vec->insert(vec->end(), bytes, bytes + size);
}

// Get random seed if -1
int64_t getEffectiveSeed(int64_t seed) {
    if (seed < 0) {
        std::random_device rd;
        return static_cast<int64_t>(rd());
    }
    return seed;
}

// Map quality to steps
int getStepsForQuality(const std::string& quality, int default_steps) {
    if (quality == "hd") {
        return std::max(default_steps, 30);  // More steps for HD quality
    }
    return default_steps;
}

// Map style to cfg_scale adjustment
float getCfgScaleForStyle(const std::string& style, float default_cfg) {
    if (style == "natural") {
        return std::min(default_cfg, 5.0f);  // Lower CFG for more natural results
    }
    return default_cfg;  // "vivid" uses default or higher
}

// Convert sd_image_t to PNG
std::vector<uint8_t> encodeImageToPng(const sd_image_t* image) {
    std::vector<uint8_t> png_data;
    if (!image || !image->data) {
        spdlog::error("Invalid image pointer");
        return {};
    }

    int result = stbi_write_png_to_func(stbiWriteCallback,
                                        &png_data,
                                        static_cast<int>(image->width),
                                        static_cast<int>(image->height),
                                        static_cast<int>(image->channel),
                                        image->data,
                                        static_cast<int>(image->width * image->channel));

    if (result == 0) {
        spdlog::error("Failed to encode image to PNG");
        return {};
    }

    return png_data;
}

}  // namespace

SDManager::SDManager(std::string models_dir) : models_dir_(std::move(models_dir)) {
    spdlog::info("SDManager initialized with models dir: {}", models_dir_);
}

SDManager::~SDManager() {
    std::lock_guard<std::mutex> lock(mutex_);
    for (auto& [path, ctx] : loaded_models_) {
        if (ctx) {
            free_sd_ctx(ctx);
        }
    }
    loaded_models_.clear();
    spdlog::info("SDManager destroyed, all models unloaded");
}

std::string SDManager::canonicalizePath(const std::string& path) const {
    try {
        if (std::filesystem::path(path).is_absolute()) {
            return std::filesystem::canonical(path).string();
        }
        return std::filesystem::canonical(std::filesystem::path(models_dir_) / path)
            .string();
    } catch (const std::filesystem::filesystem_error& e) {
        if (std::filesystem::path(path).is_absolute()) {
            return path;
        }
        return (std::filesystem::path(models_dir_) / path).string();
    }
}

void SDManager::updateAccessTime(const std::string& model_path) {
    last_access_[model_path] = std::chrono::steady_clock::now();
}

bool SDManager::loadModel(const std::string& model_path) {
    std::lock_guard<std::mutex> lock(mutex_);

    std::string canonical_path = canonicalizePath(model_path);

    if (loaded_models_.find(canonical_path) != loaded_models_.end()) {
        spdlog::debug("SD model already loaded: {}", canonical_path);
        updateAccessTime(canonical_path);
        return true;
    }

    if (!canLoadMore()) {
        spdlog::warn("Cannot load more SD models, max limit reached: {}",
                     max_loaded_models_);
        return false;
    }

    const char* force_cpu_env = std::getenv("ALLM_SD_FORCE_CPU");
    if (force_cpu_env && std::string(force_cpu_env) != "0") {
#ifdef _WIN32
        _putenv_s("SD_FORCE_CPU", "1");
#else
        setenv("SD_FORCE_CPU", "1", 1);
#endif
        spdlog::info("SD backend forced to CPU via ALLM_SD_FORCE_CPU");
    }

    spdlog::info("Loading SD model: {}", canonical_path);

    // Initialize context parameters
    sd_ctx_params_t ctx_params;
    sd_ctx_params_init(&ctx_params);

    ctx_params.model_path = canonical_path.c_str();
    // Edits/variations require VAE encode, so keep full VAE enabled.
    ctx_params.vae_decode_only = false;
    // This server keeps SD contexts across requests; do not free params after use.
    ctx_params.free_params_immediately = false;
    const char* threads_env = std::getenv("LLM_SD_THREADS");
    if (threads_env && threads_env[0] != '\0') {
        char* end = nullptr;
        long value = std::strtol(threads_env, &end, 10);
        if (end != threads_env && *end == '\0' && value > 0 &&
            value <= static_cast<long>(std::numeric_limits<int>::max())) {
            ctx_params.n_threads = static_cast<int>(value);
            spdlog::info("SDManager using LLM_SD_THREADS={}", ctx_params.n_threads);
        } else {
            spdlog::warn("Invalid LLM_SD_THREADS='{}', using default {}",
                         threads_env,
                         ctx_params.n_threads);
        }
    }

    // Create context
    sd_ctx_t* ctx = new_sd_ctx(&ctx_params);

    if (!ctx) {
        spdlog::error("Failed to load SD model: {}", canonical_path);
        return false;
    }

    loaded_models_[canonical_path] = ctx;
    updateAccessTime(canonical_path);

    spdlog::info("SD model loaded successfully: {}", canonical_path);
    return true;
}

bool SDManager::isLoaded(const std::string& model_path) const {
    std::lock_guard<std::mutex> lock(mutex_);
    std::string canonical_path = canonicalizePath(model_path);
    return loaded_models_.find(canonical_path) != loaded_models_.end();
}

sd_ctx_t* SDManager::getContext(const std::string& model_path) const {
    std::lock_guard<std::mutex> lock(mutex_);
    std::string canonical_path = canonicalizePath(model_path);
    auto it = loaded_models_.find(canonical_path);
    if (it != loaded_models_.end()) {
        return it->second;
    }
    return nullptr;
}

bool SDManager::decodePngToImage(const std::vector<uint8_t>& png_data,
                                 std::vector<uint8_t>& out_data,
                                 int& out_width,
                                 int& out_height) const {
    int channels = 0;
    uint8_t* data = stbi_load_from_memory(png_data.data(),
                                          static_cast<int>(png_data.size()),
                                          &out_width,
                                          &out_height,
                                          &channels,
                                          3);  // Force RGB

    if (!data) {
        spdlog::error("Failed to decode PNG: {}", stbi_failure_reason());
        return false;
    }

    out_data.assign(data, data + (out_width * out_height * 3));
    stbi_image_free(data);
    return true;
}

std::vector<uint8_t> SDManager::toMaskChannel(const std::vector<uint8_t>& rgb,
                                              int width,
                                              int height) {
    if (width <= 0 || height <= 0) {
        return {};
    }
    const size_t expected = static_cast<size_t>(width) * height * 3;
    if (rgb.size() < expected) {
        return {};
    }

    std::vector<uint8_t> mask;
    mask.resize(static_cast<size_t>(width) * height);
    for (size_t i = 0; i < mask.size(); ++i) {
        const size_t idx = i * 3;
        const uint16_t r = rgb[idx];
        const uint16_t g = rgb[idx + 1];
        const uint16_t b = rgb[idx + 2];
        mask[i] = static_cast<uint8_t>((r + g + b) / 3);
    }
    return mask;
}

std::vector<uint8_t> SDManager::makeSolidMask(int width, int height, uint8_t value) {
    if (width <= 0 || height <= 0) {
        return {};
    }
    return std::vector<uint8_t>(static_cast<size_t>(width) * height, value);
}

std::vector<ImageGenerationResult> SDManager::generateImages(
    const std::string& model_path,
    const ImageGenParams& params) {
    std::vector<ImageGenerationResult> results;

    std::string canonical_path = canonicalizePath(model_path);

    sd_ctx_t* ctx = nullptr;
    {
        std::lock_guard<std::mutex> lock(mutex_);
        auto it = loaded_models_.find(canonical_path);
        if (it == loaded_models_.end()) {
            ImageGenerationResult error_result;
            error_result.error = "Model not loaded: " + canonical_path;
            results.push_back(error_result);
            return results;
        }
        ctx = it->second;
        updateAccessTime(canonical_path);
    }

    // Prepare generation parameters
    sd_img_gen_params_t gen_params;
    sd_img_gen_params_init(&gen_params);

    gen_params.prompt = params.prompt.c_str();
    gen_params.negative_prompt =
        params.negative_prompt.empty() ? "" : params.negative_prompt.c_str();
    gen_params.width = params.width;
    gen_params.height = params.height;
    gen_params.batch_count = params.batch_count;
    gen_params.seed = getEffectiveSeed(params.seed);

    // Apply quality and style settings
    int steps = getStepsForQuality(params.quality, params.steps);
    float cfg_scale = getCfgScaleForStyle(params.style, params.cfg_scale);

    gen_params.sample_params.sample_steps = steps;
    gen_params.sample_params.guidance.txt_cfg = cfg_scale;

    spdlog::info(
        "Generating {} images: {}x{}, steps={}, cfg={}, seed={}",
        params.batch_count, params.width, params.height, steps, cfg_scale,
        gen_params.seed);

    // Generate images
    sd_image_t* images = generate_image(ctx, &gen_params);

    if (!images) {
        ImageGenerationResult error_result;
        error_result.error = "Image generation failed";
        results.push_back(error_result);
        return results;
    }

    // Convert each generated image to PNG
    for (int i = 0; i < params.batch_count; ++i) {
        ImageGenerationResult result;

        if (images[i].data) {
            result.image_data = encodeImageToPng(&images[i]);
            result.width = static_cast<int>(images[i].width);
            result.height = static_cast<int>(images[i].height);
            result.success = !result.image_data.empty();

            if (!result.success) {
                result.error = "Failed to encode image to PNG";
            }

            // Free the image data
            free(images[i].data);
        } else {
            result.error = "Generated image data is null";
        }

        results.push_back(result);
    }

    // Free the images array
    free(images);

    spdlog::info("Generated {} images successfully", results.size());
    return results;
}

std::vector<ImageGenerationResult> SDManager::editImages(
    const std::string& model_path,
    const ImageEditParams& params) {
    std::vector<ImageGenerationResult> results;

    std::string canonical_path = canonicalizePath(model_path);

    sd_ctx_t* ctx = nullptr;
    {
        std::lock_guard<std::mutex> lock(mutex_);
        auto it = loaded_models_.find(canonical_path);
        if (it == loaded_models_.end()) {
            ImageGenerationResult error_result;
            error_result.error = "Model not loaded: " + canonical_path;
            results.push_back(error_result);
            return results;
        }
        ctx = it->second;
        updateAccessTime(canonical_path);
    }

    // Decode input image
    std::vector<uint8_t> image_rgb;
    int img_width = 0;
    int img_height = 0;
    if (!decodePngToImage(params.image_data, image_rgb, img_width, img_height)) {
        ImageGenerationResult error_result;
        error_result.error = "Failed to decode input image";
        results.push_back(error_result);
        return results;
    }

    // Prepare generation parameters for img2img/inpainting
    sd_img_gen_params_t gen_params;
    sd_img_gen_params_init(&gen_params);

    gen_params.prompt = params.prompt.c_str();
    gen_params.width = params.width;
    gen_params.height = params.height;
    gen_params.batch_count = params.batch_count;
    gen_params.seed = getEffectiveSeed(params.seed);
    gen_params.strength = params.strength;

    gen_params.sample_params.sample_steps = params.steps;
    gen_params.sample_params.guidance.txt_cfg = params.cfg_scale;

    // Set init image
    gen_params.init_image.width = static_cast<uint32_t>(img_width);
    gen_params.init_image.height = static_cast<uint32_t>(img_height);
    gen_params.init_image.channel = 3;
    gen_params.init_image.data = image_rgb.data();

    // Set mask (required by stable-diffusion.cpp img2img path)
    std::vector<uint8_t> mask_gray;
    if (!params.mask_data.empty()) {
        std::vector<uint8_t> mask_rgb;
        int mask_width = 0;
        int mask_height = 0;
        if (!decodePngToImage(params.mask_data, mask_rgb, mask_width, mask_height)) {
            ImageGenerationResult error_result;
            error_result.error = "Failed to decode mask image";
            results.push_back(error_result);
            return results;
        }
        if (mask_width != img_width || mask_height != img_height) {
            ImageGenerationResult error_result;
            error_result.error = "Mask size must match input image size";
            results.push_back(error_result);
            return results;
        }
        mask_gray = toMaskChannel(mask_rgb, mask_width, mask_height);
        if (mask_gray.empty()) {
            ImageGenerationResult error_result;
            error_result.error = "Invalid mask image data";
            results.push_back(error_result);
            return results;
        }
    } else {
        // Default mask: all zeros (no masked area)
        mask_gray = makeSolidMask(img_width, img_height, 0);
    }

    gen_params.mask_image.width = static_cast<uint32_t>(img_width);
    gen_params.mask_image.height = static_cast<uint32_t>(img_height);
    gen_params.mask_image.channel = 1;
    gen_params.mask_image.data = mask_gray.data();

    spdlog::info(
        "Editing image: {}x{} -> {}x{}, strength={}, steps={}",
        img_width, img_height, params.width, params.height,
        params.strength, params.steps);

    // Generate edited images
    sd_image_t* images = generate_image(ctx, &gen_params);

    if (!images) {
        ImageGenerationResult error_result;
        error_result.error = "Image editing failed";
        results.push_back(error_result);
        return results;
    }

    // Convert each edited image to PNG
    for (int i = 0; i < params.batch_count; ++i) {
        ImageGenerationResult result;

        if (images[i].data) {
            result.image_data = encodeImageToPng(&images[i]);
            result.width = static_cast<int>(images[i].width);
            result.height = static_cast<int>(images[i].height);
            result.success = !result.image_data.empty();

            if (!result.success) {
                result.error = "Failed to encode image to PNG";
            }

            free(images[i].data);
        } else {
            result.error = "Edited image data is null";
        }

        results.push_back(result);
    }

    free(images);

    spdlog::info("Edited {} images successfully", results.size());
    return results;
}

std::vector<ImageGenerationResult> SDManager::generateVariations(
    const std::string& model_path,
    const ImageVariationParams& params) {
    // Variations are implemented as img2img with moderate strength
    ImageEditParams edit_params;
    edit_params.prompt = "";  // No prompt for pure variations
    edit_params.image_data = params.image_data;
    edit_params.width = params.width;
    edit_params.height = params.height;
    edit_params.steps = params.steps;
    edit_params.cfg_scale = params.cfg_scale;
    edit_params.strength = params.strength;
    edit_params.seed = params.seed;
    edit_params.batch_count = params.batch_count;

    return editImages(model_path, edit_params);
}

size_t SDManager::loadedCount() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return loaded_models_.size();
}

bool SDManager::unloadModel(const std::string& model_path) {
    std::lock_guard<std::mutex> lock(mutex_);
    std::string canonical_path = canonicalizePath(model_path);

    auto it = loaded_models_.find(canonical_path);
    if (it == loaded_models_.end()) {
        return false;
    }

    if (it->second) {
        free_sd_ctx(it->second);
    }
    loaded_models_.erase(it);
    last_access_.erase(canonical_path);

    spdlog::info("SD model unloaded: {}", canonical_path);
    return true;
}

std::vector<std::string> SDManager::getLoadedModels() const {
    std::lock_guard<std::mutex> lock(mutex_);
    std::vector<std::string> models;
    models.reserve(loaded_models_.size());
    for (const auto& [path, _] : loaded_models_) {
        models.push_back(path);
    }
    return models;
}

bool SDManager::loadModelIfNeeded(const std::string& model_path) {
    if (isLoaded(model_path)) {
        std::lock_guard<std::mutex> lock(mutex_);
        updateAccessTime(canonicalizePath(model_path));
        return true;
    }
    return loadModel(model_path);
}

void SDManager::setIdleTimeout(std::chrono::milliseconds timeout) {
    std::lock_guard<std::mutex> lock(mutex_);
    idle_timeout_ = timeout;
}

std::chrono::milliseconds SDManager::getIdleTimeout() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return idle_timeout_;
}

size_t SDManager::unloadIdleModels() {
    std::lock_guard<std::mutex> lock(mutex_);

    auto now = std::chrono::steady_clock::now();
    std::vector<std::string> to_unload;

    for (const auto& [path, last_time] : last_access_) {
        auto idle_duration =
            std::chrono::duration_cast<std::chrono::milliseconds>(now - last_time);
        if (idle_duration >= idle_timeout_) {
            to_unload.push_back(path);
        }
    }

    for (const auto& path : to_unload) {
        auto it = loaded_models_.find(path);
        if (it != loaded_models_.end()) {
            if (it->second) {
                free_sd_ctx(it->second);
            }
            loaded_models_.erase(it);
            last_access_.erase(path);
            spdlog::info("Unloaded idle SD model: {}", path);
        }
    }

    return to_unload.size();
}

void SDManager::setMaxLoadedModels(size_t max_models) {
    std::lock_guard<std::mutex> lock(mutex_);
    max_loaded_models_ = max_models;
}

size_t SDManager::getMaxLoadedModels() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return max_loaded_models_;
}

bool SDManager::canLoadMore() const {
    if (max_loaded_models_ == 0) {
        return true;
    }
    return loaded_models_.size() < max_loaded_models_;
}

std::optional<std::chrono::steady_clock::time_point> SDManager::getLastAccessTime(
    const std::string& model_path) const {
    std::lock_guard<std::mutex> lock(mutex_);
    std::string canonical_path = canonicalizePath(model_path);

    auto it = last_access_.find(canonical_path);
    if (it != last_access_.end()) {
        return it->second;
    }
    return std::nullopt;
}

}  // namespace allm

#else

namespace allm {

SDManager::SDManager(std::string models_dir) : models_dir_(std::move(models_dir)) {
    spdlog::warn("SDManager: stable-diffusion.cpp support is disabled (BUILD_WITH_SD=OFF)");
}

SDManager::~SDManager() = default;

bool SDManager::loadModel(const std::string&) { return false; }
bool SDManager::isLoaded(const std::string&) const { return false; }
sd_ctx_t* SDManager::getContext(const std::string&) const { return nullptr; }

std::vector<ImageGenerationResult> SDManager::generateImages(const std::string&, const ImageGenParams&) {
    ImageGenerationResult r;
    r.success = false;
    r.error = "stable-diffusion.cpp support is disabled";
    return {r};
}

std::vector<ImageGenerationResult> SDManager::editImages(const std::string&, const ImageEditParams&) {
    ImageGenerationResult r;
    r.success = false;
    r.error = "stable-diffusion.cpp support is disabled";
    return {r};
}

std::vector<ImageGenerationResult> SDManager::generateVariations(const std::string&, const ImageVariationParams&) {
    ImageGenerationResult r;
    r.success = false;
    r.error = "stable-diffusion.cpp support is disabled";
    return {r};
}

size_t SDManager::loadedCount() const { return 0; }
bool SDManager::unloadModel(const std::string&) { return false; }
std::vector<std::string> SDManager::getLoadedModels() const { return {}; }
bool SDManager::loadModelIfNeeded(const std::string&) { return false; }

void SDManager::setIdleTimeout(std::chrono::milliseconds timeout) { idle_timeout_ = timeout; }
std::chrono::milliseconds SDManager::getIdleTimeout() const { return idle_timeout_; }
size_t SDManager::unloadIdleModels() { return 0; }

void SDManager::setMaxLoadedModels(size_t max_models) { max_loaded_models_ = max_models; }
size_t SDManager::getMaxLoadedModels() const { return max_loaded_models_; }
bool SDManager::canLoadMore() const { return false; }

std::optional<std::chrono::steady_clock::time_point> SDManager::getLastAccessTime(const std::string&) const {
    return std::nullopt;
}

}  // namespace allm

#endif
