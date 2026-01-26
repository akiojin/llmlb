# 技術リサーチ: LLM-Load Balancer独自モデルストレージ

## リサーチ課題

1. 既存LLMランタイムのストレージ形式（manifest/blob）からの脱却方法
2. クロスプラットフォームでのファイルパス正規化
3. モデルアーティファクト形式の判定方法
4. 外部ソース（HuggingFace）からの直接ダウンロード方式

## 1. ストレージ形式の選定

### 決定

シンプルなディレクトリベース構造を採用。

```text
~/.llmlb/models/<model-name>/<artifacts>
```

### 理由

| 観点 | manifest/blob形式 | ディレクトリ形式 |
|------|------------------|------------------|
| 複雑さ | 高（SHA256ハッシュ解決） | 低（パス直接参照） |
| 外部依存 | Ollama/Docker形式に依存 | 独立 |
| デバッグ容易性 | 低（ハッシュ名ファイル） | 高（意味のある名前） |
| 移植性 | 他ツールと共有可能 | 独立した管理 |

### 代替案

| 案 | 説明 | 却下理由 |
|----|------|----------|
| Ollamaストレージ互換 | `~/.ollama/models/` を参照 | 外部アプリ依存、仕様変更リスク |
| SQLite内蔵 | モデルをBLOBとして保存 | ファイルサイズ制限、複雑性 |
| S3互換オブジェクト | MinIO等でオブジェクト管理 | オーバーエンジニアリング |

### 実装方法

```cpp
// model_storage.h
class ModelStorage {
public:
    explicit ModelStorage(const std::filesystem::path& models_dir);
    std::optional<std::filesystem::path> resolve(const std::string& model_id);
    std::vector<std::string> list_available();
private:
    std::filesystem::path models_dir_;
};
```

## 2. パス正規化

### 決定

`std::filesystem::path` を使用し、プラットフォーム差異を吸収。

### 理由

- C++17標準ライブラリで追加依存なし
- Windows/Linux/macOSのパスセパレータを自動処理
- `canonical()` で相対パス・シンボリックリンクを解決

### 実装方法

```cpp
std::filesystem::path normalize_model_path(const std::string& model_id) {
    // 危険な文字のチェック
    if (model_id.find("..") != std::string::npos) {
        throw std::invalid_argument("Invalid model ID: contains '..'");
    }

    // 小文字正規化
    std::string normalized = to_lowercase(model_id);
    return models_dir_ / normalized;
}
```

## 3. アーティファクト形式判定

### 決定

拡張子ベースの判定を採用。

| 拡張子 | 形式 | エンジン |
|--------|------|----------|
| `.gguf` | GGUF | llama.cpp |
| `.safetensors` | SafeTensors | Nemotron/CUDA |
| `.metal.bin` | Metal最適化 | Apple Silicon |

### 理由

- シンプルで高速
- 追加のファイル読み込み不要
- 既存のファイル命名規則と整合

### 実装方法

```cpp
enum class ModelFormat { Gguf, Safetensors, Metal, Unknown };

ModelFormat detect_format(const std::filesystem::path& dir) {
    for (const auto& entry : std::filesystem::directory_iterator(dir)) {
        if (entry.path().extension() == ".gguf") return ModelFormat::Gguf;
        if (entry.path().extension() == ".safetensors") return ModelFormat::Safetensors;
        if (entry.path().string().ends_with(".metal.bin")) return ModelFormat::Metal;
    }
    return ModelFormat::Unknown;
}
```

## 4. 外部ソースダウンロード

### 決定

HuggingFace Hub APIを使用した直接ダウンロード。

### 理由

- HuggingFaceは事実上の標準リポジトリ
- REST APIでファイル一覧・ダウンロードが可能
- 認証トークン対応で非公開モデルもアクセス可能

### 実装方法

```cpp
// hf_downloader.h
class HfDownloader {
public:
    explicit HfDownloader(const std::string& base_url = "https://huggingface.co");

    // ファイル一覧を取得
    std::vector<HfFile> list_files(const std::string& repo_id);

    // ファイルをダウンロード
    void download(const std::string& repo_id,
                  const std::string& filename,
                  const std::filesystem::path& dest);
private:
    std::string base_url_;
    std::optional<std::string> token_; // HF_TOKEN環境変数から取得
};
```

### API エンドポイント

```text
GET https://huggingface.co/api/models/{repo_id}/tree/main
GET https://huggingface.co/{repo_id}/resolve/main/{filename}
```

## 参考リソース

- [HuggingFace Hub API Documentation](https://huggingface.co/docs/hub/api)
- [C++17 Filesystem Library](https://en.cppreference.com/w/cpp/filesystem)
- [GGUF Format Specification](https://github.com/ggerganov/ggml/blob/master/docs/gguf.md)
- [SafeTensors Format](https://huggingface.co/docs/safetensors)
