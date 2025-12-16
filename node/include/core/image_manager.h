#pragma once

#include <string>
#include <vector>
#include <mutex>
#include <optional>

namespace llm_node {

/// Text-to-Image パラメータ
struct T2IParams {
    std::string model{"z-image-turbo"};
    std::string prompt;
    std::string negative_prompt;
    int width{512};
    int height{512};
    int steps{9};
    float guidance_scale{0.0f};
    std::optional<int64_t> seed;
};

/// Text-to-Image 結果
struct T2IResult {
    bool success{false};
    std::string image_path;       // 生成画像のパス
    std::vector<uint8_t> image_data;  // PNG画像データ (base64用)
    std::string error;
};

/// Image-to-Text パラメータ
struct I2TParams {
    std::string model{"glm-4.6v-flash"};
    std::string image_path;       // 入力画像のパス
    std::string prompt{"describe this image"};
    int max_new_tokens{256};
};

/// Image-to-Text 結果
struct I2TResult {
    bool success{false};
    std::string caption;
    std::string error;
};

/// Python subprocessによる画像生成・キャプション生成マネージャー
///
/// HuggingFace配布モデルをそのまま使用（GGUF変換不要）:
/// - T2I: Tongyi-MAI/Z-Image-Turbo (diffusers)
/// - I2T: zai-org/GLM-4.6V-Flash (transformers)
class ImageManager {
public:
    explicit ImageManager(std::string scripts_dir = "");
    ~ImageManager() = default;

    // Disable copy
    ImageManager(const ImageManager&) = delete;
    ImageManager& operator=(const ImageManager&) = delete;

    /// Text-to-Image: プロンプトから画像を生成
    T2IResult generateImage(const T2IParams& params);

    /// Image-to-Text: 画像からキャプションを生成
    I2TResult captionImage(const I2TParams& params);

    /// 画像生成機能が利用可能か確認
    bool isT2IAvailable() const;

    /// キャプション機能が利用可能か確認
    bool isI2TAvailable() const;

    /// 環境変数から設定を読み込み
    void loadConfig();

private:
    std::string scripts_dir_;
    std::string python_bin_{"python3"};
    std::string t2i_runner_;
    std::string i2t_runner_;
    std::string device_{"auto"};
    mutable std::mutex mutex_;

    /// Python スクリプトを実行
    int runPython(const std::vector<std::string>& args) const;

    /// 一時ファイルパスを生成
    static std::string makeTempPath(const std::string& prefix, const std::string& ext);
};

}  // namespace llm_node
