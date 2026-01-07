# 技術リサーチ: Nodeエンジンローダー抽象化

## リサーチ課題

1. プラグイン形式（動的ロード）によるエンジン抽象化方式
2. C ABIの設計とバージョン管理
3. GPU バックエンド別のバイナリ配布方式
4. ホットリロードとリソース監視の実装方法

## 1. プラグイン形式の選定

### 決定

共有ライブラリ（.so/.dylib/.dll）+ C ABI + manifest.json形式を採用。

### 理由

| 観点 | IPC/プロセス分離 | 共有ライブラリ |
|------|-----------------|---------------|
| レイテンシ | 高（シリアライズ） | 低（直接呼び出し） |
| 障害分離 | 高（独立プロセス） | 低（同一プロセス） |
| 実装複雑度 | 高 | 低 |
| VRAM共有 | 困難 | 容易 |

推論は低レイテンシが重要なため、共有ライブラリ方式を採用。
障害分離は定期再起動ポリシーで対処。

### 代替案

| 案 | 説明 | 却下理由 |
|----|------|----------|
| gRPCサービス | 各エンジンを独立サービス化 | レイテンシオーバーヘッド大 |
| WASM | ポータブルなプラグイン | GPU/VRAMアクセス制限 |
| Python埋め込み | PyTorch/Transformers活用 | Python依存禁止 |

### 実装方法

```cpp
// engine_host.h
class EngineHost {
public:
    void discover_plugins(const std::filesystem::path& plugins_dir);
    Engine* load_engine(const std::string& engine_id);
    void unload_engine(const std::string& engine_id);

private:
    std::map<std::string, void*> handles_;  // dlopen handles
    std::map<std::string, PluginManifest> manifests_;
};
```

## 2. C ABI 設計

### 決定

最小限のC関数インターフェースを定義し、ABIバージョンで互換性を管理。

### 理由

- C ABIは安定性が高く、言語間互換性が優秀
- C++のname manglingやvtable問題を回避
- ABIバージョンで破壊的変更を管理

### ABI定義

```c
// engine_api.h (C ABI)
#define ENGINE_ABI_VERSION 1

typedef struct {
    uint32_t abi_version;
    const char* engine_id;
    const char* version;
} EngineInfo;

typedef struct {
    void* ctx;
    // ... config fields
} EngineConfig;

typedef struct {
    uint32_t token_id;
    float* logprobs;
    size_t logprobs_len;
} TokenResult;

// 必須エクスポート関数
ENGINE_API EngineInfo* engine_get_info(void);
ENGINE_API int engine_init(EngineConfig* config);
ENGINE_API int engine_load_model(const char* model_path);
ENGINE_API int engine_generate(const char* prompt, TokenResult** results);
ENGINE_API void engine_shutdown(void);
```

### ABIバージョン管理

| バージョン | 変更内容 |
|-----------|---------|
| 1 | 初期リリース |
| 2 | generate()にコールバック追加（将来） |

不一致時はロード拒否 + アラートログ出力。

## 3. GPUバックエンド別バイナリ

### 決定

`engines/{engine_id}/{gpu_backend}/{binary}` の階層構造を採用。

### 理由

- 同一エンジンでもMetal/CUDA/CUDAで異なるバイナリが必要
- ディレクトリ分離で明確な管理
- manifest.jsonでgpu_backendを宣言

### ディレクトリ構造

```text
engines/
├── llama_cpp/
│   ├── metal/
│   │   ├── manifest.json
│   │   └── libllama_engine.dylib
│   ├── directml/
│   │   ├── manifest.json
│   │   └── llama_engine.dll
│   └── cuda/
│       ├── manifest.json
│       └── libllama_engine.so
└── nemotron/
    ├── metal/
    │   ├── manifest.json
    │   └── libnemotron_engine.dylib
    └── directml/
        ├── manifest.json
        └── nemotron_engine.dll
```

### manifest.json

```json
{
  "id": "llama_cpp",
  "version": "1.0.0",
  "abi_version": "1",
  "gpu_backend": "metal",
  "architectures": ["llama", "mistral", "gemma", "phi"],
  "formats": ["gguf", "safetensors"],
  "binary": "libllama_engine.dylib"
}
```

## 4. ホットリロード設計

### 決定

シャドウロード方式を採用。

### 理由

- ゼロダウンタイムでプラグイン更新
- 新旧プラグインが一時的に共存
- LRUで旧プラグインが自然に追い出される

### シーケンス

```text
1. 新プラグインバイナリを検出
2. 新プラグインをロード（旧と並行）
3. 新規リクエストは新プラグインへ
4. 旧プラグインの処理中リクエスト完了待ち
5. 旧プラグインをアンロード
```

### 実装方法

```cpp
class EngineHost {
    void hot_reload(const std::string& engine_id) {
        // 1. 新プラグインをシャドウロード
        auto new_handle = load_plugin(engine_id + "_new");

        // 2. 新規リクエストを新プラグインへルーティング
        active_engines_[engine_id] = new_handle;

        // 3. 旧プラグインの処理完了待ち
        wait_for_requests(old_handle);

        // 4. 旧プラグインをアンロード
        unload_plugin(old_handle);
    }
};
```

## 5. リソース監視設計

### 決定

1秒間隔ポーリングで VRAM/RAM 使用率を監視。

### 閾値設定

| リソース | 閾値 | アクション |
|---------|------|----------|
| VRAM | 90% | LRUでモデルアンロード |
| RAM | 90% | LRUでキャッシュクリア |

### 実装方法

```cpp
class ResourceMonitor {
    void poll() {
        auto vram_usage = get_vram_usage();
        if (vram_usage > 0.9) {
            engine_host_.evict_lru_model();
            logger_.warn("VRAM high: {}%, evicted LRU model", vram_usage * 100);
        }
    }

    std::thread monitor_thread_;
};
```

## 参考リソース

- [dlopen(3) - Linux man page](https://man7.org/linux/man-pages/man3/dlopen.3.html)
- [Dynamic Libraries in C++](https://en.cppreference.com/w/cpp/utility/program/shared_library)
- [Metal Performance Shaders](https://developer.apple.com/documentation/metalperformanceshaders)
- [DirectML Programming Guide](https://docs.microsoft.com/en-us/windows/ai/directml/)
