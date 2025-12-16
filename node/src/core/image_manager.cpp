#include "core/image_manager.h"

#include <spdlog/spdlog.h>
#include <filesystem>
#include <fstream>
#include <cstdlib>
#include <random>
#include <chrono>

#if defined(__APPLE__) || defined(__linux__)
#include <spawn.h>
#include <sys/wait.h>
#include <unistd.h>
extern char** environ;
#endif

namespace llm_node {

namespace {

#if defined(__APPLE__) || defined(__linux__)
int run_command(const std::vector<std::string>& args) {
    if (args.empty()) {
        return -1;
    }

    std::vector<char*> argv;
    argv.reserve(args.size() + 1);
    for (const auto& a : args) {
        argv.push_back(const_cast<char*>(a.c_str()));
    }
    argv.push_back(nullptr);

    pid_t pid;
    int spawn_result = posix_spawnp(&pid, argv[0], nullptr, nullptr, argv.data(), environ);
    if (spawn_result != 0) {
        spdlog::error("ImageManager: posix_spawnp failed: {}", spawn_result);
        return spawn_result;
    }

    int status = 0;
    if (waitpid(pid, &status, 0) == -1) {
        spdlog::error("ImageManager: waitpid failed");
        return -1;
    }
    if (WIFEXITED(status)) {
        return WEXITSTATUS(status);
    }
    if (WIFSIGNALED(status)) {
        return 128 + WTERMSIG(status);
    }
    return -1;
}
#endif

std::vector<uint8_t> read_file_bytes(const std::filesystem::path& path) {
    std::ifstream in(path, std::ios::binary);
    if (!in) {
        throw std::runtime_error("Failed to open file: " + path.string());
    }

    in.seekg(0, std::ios::end);
    std::streamsize size = in.tellg();
    if (size < 0) {
        throw std::runtime_error("Failed to stat file size: " + path.string());
    }
    in.seekg(0, std::ios::beg);

    std::vector<uint8_t> data(static_cast<size_t>(size));
    if (!in.read(reinterpret_cast<char*>(data.data()), size)) {
        throw std::runtime_error("Failed to read file: " + path.string());
    }
    return data;
}

std::string read_file_text(const std::filesystem::path& path) {
    std::ifstream in(path);
    if (!in) {
        throw std::runtime_error("Failed to open file: " + path.string());
    }
    std::string content((std::istreambuf_iterator<char>(in)),
                         std::istreambuf_iterator<char>());
    return content;
}

}  // namespace

ImageManager::ImageManager(std::string scripts_dir)
    : scripts_dir_(std::move(scripts_dir)) {
    loadConfig();
}

void ImageManager::loadConfig() {
    // Python インタプリタ
    if (const char* env = std::getenv("LLM_NODE_IMAGE_PYTHON")) {
        python_bin_ = env;
    }

    // T2I スクリプト
    if (const char* env = std::getenv("LLM_NODE_T2I_RUNNER")) {
        t2i_runner_ = env;
    } else if (!scripts_dir_.empty()) {
        t2i_runner_ = scripts_dir_ + "/generate_z_image_turbo.py";
    }

    // I2T スクリプト
    if (const char* env = std::getenv("LLM_NODE_I2T_RUNNER")) {
        i2t_runner_ = env;
    } else if (!scripts_dir_.empty()) {
        i2t_runner_ = scripts_dir_ + "/glm4v_flash_caption.py";
    }

    // デバイス設定
    if (const char* env = std::getenv("LLM_NODE_IMAGE_DEVICE")) {
        device_ = env;
    }

    spdlog::info("ImageManager config: python={}, t2i={}, i2t={}, device={}",
                 python_bin_, t2i_runner_, i2t_runner_, device_);
}

bool ImageManager::isT2IAvailable() const {
    if (t2i_runner_.empty()) {
        return false;
    }
    return std::filesystem::exists(t2i_runner_);
}

bool ImageManager::isI2TAvailable() const {
    if (i2t_runner_.empty()) {
        return false;
    }
    return std::filesystem::exists(i2t_runner_);
}

std::string ImageManager::makeTempPath(const std::string& prefix, const std::string& ext) {
    static std::random_device rd;
    static std::mt19937 gen(rd());
    static std::uniform_int_distribution<> dis(0, 999999);

    auto now = std::chrono::system_clock::now();
    auto epoch = now.time_since_epoch();
    auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(epoch).count();

    std::filesystem::path tmp_dir = "/tmp/llm_router_image";
    std::filesystem::create_directories(tmp_dir);

    std::string filename = prefix + "_" + std::to_string(ms) + "_" +
                          std::to_string(dis(gen)) + ext;
    return (tmp_dir / filename).string();
}

int ImageManager::runPython(const std::vector<std::string>& args) const {
#if defined(__APPLE__) || defined(__linux__)
    return run_command(args);
#else
    spdlog::error("ImageManager: Python subprocess not supported on this platform");
    return -1;
#endif
}

T2IResult ImageManager::generateImage(const T2IParams& params) {
    std::lock_guard<std::mutex> lock(mutex_);

    T2IResult result;

    if (!isT2IAvailable()) {
        result.error = "T2I runner not available: " + t2i_runner_;
        spdlog::error("ImageManager: {}", result.error);
        return result;
    }

    if (params.prompt.empty()) {
        result.error = "Prompt is required";
        return result;
    }

    // 出力パスを生成
    std::string out_path = makeTempPath("t2i", ".png");

    // RAII で一時ファイルをクリーンアップ
    struct Cleanup {
        std::string path;
        bool keep{false};
        ~Cleanup() {
            if (!keep && !path.empty()) {
                std::filesystem::remove(path);
            }
        }
    } cleanup{out_path, false};

    // Python コマンドを構築
    std::vector<std::string> args = {
        python_bin_,
        t2i_runner_,
        "--require-gpu",
        "--prompt", params.prompt,
        "--out", out_path,
        "--device", device_,
        "--height", std::to_string(params.height),
        "--width", std::to_string(params.width),
        "--steps", std::to_string(params.steps),
        "--guidance", std::to_string(params.guidance_scale),
    };

    if (!params.negative_prompt.empty()) {
        args.push_back("--negative-prompt");
        args.push_back(params.negative_prompt);
    }

    if (params.seed.has_value()) {
        args.push_back("--seed");
        args.push_back(std::to_string(params.seed.value()));
    }

    spdlog::info("ImageManager: T2I prompt='{}', size={}x{}, out={}",
                 params.prompt.substr(0, 50), params.width, params.height, out_path);

    // Python スクリプト実行
    int rc = runPython(args);
    if (rc != 0) {
        result.error = "T2I script failed with exit code: " + std::to_string(rc);
        spdlog::error("ImageManager: {}", result.error);
        return result;
    }

    // 出力ファイル確認
    if (!std::filesystem::exists(out_path)) {
        result.error = "T2I output file not found: " + out_path;
        spdlog::error("ImageManager: {}", result.error);
        return result;
    }

    // 画像データを読み込み
    try {
        result.image_data = read_file_bytes(out_path);
        result.image_path = out_path;
        result.success = true;
        cleanup.keep = true;  // 成功時は一時ファイルを保持
        spdlog::info("ImageManager: T2I success, size={} bytes", result.image_data.size());
    } catch (const std::exception& e) {
        result.error = std::string("Failed to read output: ") + e.what();
        spdlog::error("ImageManager: {}", result.error);
    }

    return result;
}

I2TResult ImageManager::captionImage(const I2TParams& params) {
    std::lock_guard<std::mutex> lock(mutex_);

    I2TResult result;

    if (!isI2TAvailable()) {
        result.error = "I2T runner not available: " + i2t_runner_;
        spdlog::error("ImageManager: {}", result.error);
        return result;
    }

    if (params.image_path.empty()) {
        result.error = "Image path is required";
        return result;
    }

    if (!std::filesystem::exists(params.image_path)) {
        result.error = "Image file not found: " + params.image_path;
        return result;
    }

    // 出力パスを生成
    std::string out_path = makeTempPath("i2t", ".txt");

    // RAII で一時ファイルをクリーンアップ
    struct Cleanup {
        std::string path;
        ~Cleanup() {
            if (!path.empty()) {
                std::filesystem::remove(path);
            }
        }
    } cleanup{out_path};

    // Python コマンドを構築
    std::vector<std::string> args = {
        python_bin_,
        i2t_runner_,
        "--require-gpu",
        "--image", params.image_path,
        "--prompt", params.prompt,
        "--out", out_path,
        "--device", device_,
        "--max-new-tokens", std::to_string(params.max_new_tokens),
    };

    spdlog::info("ImageManager: I2T image='{}', prompt='{}'",
                 params.image_path, params.prompt.substr(0, 50));

    // Python スクリプト実行
    int rc = runPython(args);
    if (rc != 0) {
        result.error = "I2T script failed with exit code: " + std::to_string(rc);
        spdlog::error("ImageManager: {}", result.error);
        return result;
    }

    // 出力ファイル確認
    if (!std::filesystem::exists(out_path)) {
        result.error = "I2T output file not found: " + out_path;
        spdlog::error("ImageManager: {}", result.error);
        return result;
    }

    // キャプションを読み込み
    try {
        result.caption = read_file_text(out_path);
        result.success = true;
        spdlog::info("ImageManager: I2T success, caption_len={}", result.caption.size());
    } catch (const std::exception& e) {
        result.error = std::string("Failed to read output: ") + e.what();
        spdlog::error("ImageManager: {}", result.error);
    }

    return result;
}

}  // namespace llm_node
