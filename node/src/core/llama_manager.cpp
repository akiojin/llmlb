#include "core/llama_manager.h"
#include "include/llama.h"

#include <spdlog/spdlog.h>
#include <filesystem>
#include <utility>

namespace fs = std::filesystem;

namespace ollama_node {

// LlamaContext デストラクタ: リソース解放
LlamaContext::~LlamaContext() {
    if (ctx) {
        llama_free(ctx);
        ctx = nullptr;
    }
    if (model) {
        llama_model_free(model);
        model = nullptr;
    }
}

// LlamaContext ムーブコンストラクタ
LlamaContext::LlamaContext(LlamaContext&& other) noexcept
    : model_path(std::move(other.model_path))
    , model(other.model)
    , ctx(other.ctx)
    , gpu_layers(other.gpu_layers) {
    other.model = nullptr;
    other.ctx = nullptr;
}

// LlamaContext ムーブ代入演算子
LlamaContext& LlamaContext::operator=(LlamaContext&& other) noexcept {
    if (this != &other) {
        // 既存リソースを解放
        if (ctx) llama_free(ctx);
        if (model) llama_model_free(model);

        model_path = std::move(other.model_path);
        model = other.model;
        ctx = other.ctx;
        gpu_layers = other.gpu_layers;

        other.model = nullptr;
        other.ctx = nullptr;
    }
    return *this;
}

// LlamaManager コンストラクタ
LlamaManager::LlamaManager(std::string models_dir)
    : models_dir_(std::move(models_dir)) {}

// LlamaManager デストラクタ
LlamaManager::~LlamaManager() {
    // loaded_models_ が自動的にクリーンアップされる（unique_ptr）
}

// バックエンド初期化（プログラム開始時に1回呼び出し）
void LlamaManager::initBackend() {
    spdlog::info("Initializing llama.cpp backend");
    llama_backend_init();
}

// バックエンド終了（プログラム終了時に1回呼び出し）
void LlamaManager::freeBackend() {
    spdlog::info("Freeing llama.cpp backend");
    llama_backend_free();
}

// パス正規化
std::string LlamaManager::canonicalizePath(const std::string& path) const {
    fs::path p = path;
    if (p.is_relative()) {
        p = fs::path(models_dir_) / p;
    }
    return fs::weakly_canonical(p).string();
}

// モデルロード（llama.cpp API使用）
bool LlamaManager::loadModel(const std::string& model_path) {
    std::string canonical = canonicalizePath(model_path);

    // 拡張子チェック
    fs::path p(canonical);
    if (p.extension() != ".gguf") {
        spdlog::error("Invalid model file extension (expected .gguf): {}", canonical);
        return false;
    }

    // ファイル存在チェック
    if (!fs::exists(canonical)) {
        spdlog::error("Model file not found: {}", canonical);
        return false;
    }

    std::lock_guard<std::mutex> lock(mutex_);

    // 既にロード済みか確認
    if (loaded_models_.count(canonical) > 0) {
        spdlog::debug("Model already loaded: {}", canonical);
        return true;
    }

    spdlog::info("Loading model: {} (gpu_layers={})", canonical, gpu_layers_);

    // モデルパラメータ設定
    llama_model_params model_params = llama_model_default_params();
    model_params.n_gpu_layers = static_cast<int32_t>(gpu_layers_);

    // モデルロード
    llama_model* model = llama_model_load_from_file(canonical.c_str(), model_params);
    if (!model) {
        spdlog::error("Failed to load model: {}", canonical);
        return false;
    }

    // コンテキストパラメータ設定
    llama_context_params ctx_params = llama_context_default_params();
    ctx_params.n_ctx = 4096;  // コンテキストサイズ
    ctx_params.n_batch = 512; // バッチサイズ

    // コンテキスト作成
    llama_context* ctx = llama_init_from_model(model, ctx_params);
    if (!ctx) {
        spdlog::error("Failed to create context for model: {}", canonical);
        llama_model_free(model);
        return false;
    }

    // LlamaContext構造体に格納
    auto llama_ctx = std::make_unique<LlamaContext>();
    llama_ctx->model_path = canonical;
    llama_ctx->model = model;
    llama_ctx->ctx = ctx;
    llama_ctx->gpu_layers = gpu_layers_;

    // 実メモリ使用量を取得
    uint64_t model_size = llama_model_size(model);
    memory_bytes_ += model_size;

    spdlog::info("Model loaded successfully: {} ({} bytes)", canonical, model_size);

    loaded_models_[canonical] = std::move(llama_ctx);
    return true;
}

// モデルがロード済みか確認
bool LlamaManager::isLoaded(const std::string& model_path) const {
    std::string canonical = canonicalizePath(model_path);
    std::lock_guard<std::mutex> lock(mutex_);
    return loaded_models_.count(canonical) > 0;
}

// コンテキスト取得
llama_context* LlamaManager::getContext(const std::string& model_path) const {
    std::string canonical = canonicalizePath(model_path);
    std::lock_guard<std::mutex> lock(mutex_);
    auto it = loaded_models_.find(canonical);
    if (it == loaded_models_.end()) {
        return nullptr;
    }
    return it->second->ctx;
}

// モデル取得
llama_model* LlamaManager::getModel(const std::string& model_path) const {
    std::string canonical = canonicalizePath(model_path);
    std::lock_guard<std::mutex> lock(mutex_);
    auto it = loaded_models_.find(canonical);
    if (it == loaded_models_.end()) {
        return nullptr;
    }
    return it->second->model;
}

// 旧APIとの互換性: コンテキスト生成
// 注意: 返されるshared_ptrはリソースを所有しない（カスタムデリーター使用）
std::shared_ptr<LlamaContext> LlamaManager::createContext(const std::string& model) const {
    std::string canonical = canonicalizePath(model);
    std::lock_guard<std::mutex> lock(mutex_);
    auto it = loaded_models_.find(canonical);
    if (it == loaded_models_.end()) {
        return nullptr;
    }
    // カスタムデリーター（何もしない）で二重解放を防ぐ
    // 実際のリソースはLlamaManagerが管理
    return std::shared_ptr<LlamaContext>(it->second.get(), [](LlamaContext*) {});
}

size_t LlamaManager::loadedCount() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return loaded_models_.size();
}

void LlamaManager::setGpuLayerSplit(size_t layers) {
    gpu_layers_ = layers;
}

size_t LlamaManager::getGpuLayerSplit() const {
    return gpu_layers_;
}

size_t LlamaManager::memoryUsageBytes() const {
    return memory_bytes_;
}

bool LlamaManager::unloadModel(const std::string& model_path) {
    std::string canonical = canonicalizePath(model_path);
    std::lock_guard<std::mutex> lock(mutex_);

    auto it = loaded_models_.find(canonical);
    if (it == loaded_models_.end()) {
        return false;
    }

    // メモリ使用量を減算
    if (it->second->model) {
        uint64_t model_size = llama_model_size(it->second->model);
        if (memory_bytes_ >= model_size) {
            memory_bytes_ -= model_size;
        } else {
            memory_bytes_ = 0;
        }
    }

    spdlog::info("Unloading model: {}", canonical);
    loaded_models_.erase(it);
    return true;
}

std::vector<std::string> LlamaManager::getLoadedModels() const {
    std::lock_guard<std::mutex> lock(mutex_);
    std::vector<std::string> models;
    models.reserve(loaded_models_.size());
    for (const auto& pair : loaded_models_) {
        models.push_back(pair.first);
    }
    return models;
}

}  // namespace ollama_node
