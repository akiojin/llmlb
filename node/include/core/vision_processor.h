#pragma once

#include <mutex>
#include <optional>
#include <string>
#include <unordered_map>
#include <vector>

#include "mtmd.h"

struct llama_model;

namespace llm_node {

class ModelStorage;

class VisionProcessor {
public:
    explicit VisionProcessor(const ModelStorage& model_storage);

    // Resolve or create a multimodal context for the model + mmproj pair.
    mtmd_context* getOrCreateContext(const std::string& model_name,
                                     const std::string& model_path,
                                     const llama_model* model,
                                     std::string& error) const;

    // Decode and prepare image bitmaps in marker order.
    bool prepareBitmaps(mtmd_context* ctx,
                        const std::vector<std::string>& image_urls,
                        mtmd::bitmaps& out,
                        std::string& error) const;

    static const char* marker();

private:
    const ModelStorage& model_storage_;
    mutable std::mutex mutex_;
    mutable std::unordered_map<std::string, mtmd::context_ptr> contexts_;

    std::optional<std::string> resolveMmprojPath(const std::string& model_name,
                                                 const std::string& model_path) const;
    bool loadImageData(const std::string& url, std::vector<uint8_t>& out, std::string& error) const;
    static bool decodeDataUrl(const std::string& url, std::vector<uint8_t>& out, std::string& error);
    static bool decodeBase64(const std::string& encoded, std::vector<uint8_t>& out, std::string& error);
    static bool fetchHttpUrl(const std::string& url, std::vector<uint8_t>& out, std::string& error);
};

}  // namespace llm_node
