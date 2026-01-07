# Quickstart: safetensors.cpp

**機能ID**: SPEC-69549000
**日付**: 2026-01-06

## 概要

safetensors.cppを使用してsafetensors形式のLLMを推論する手順。

## 前提条件

- CMake 3.18以上
- C++17対応コンパイラ
- GPU環境:
  - macOS: Apple Silicon + Metal
  - Windows/Linux: NVIDIA GPU + CUDA Toolkit 11.8以上
  - Linux: AMD GPU + ROCm 5.0以上
  - Vulkan SDK（オプション）

## ビルド

### サブモジュール取得

```bash
cd node/third_party/safetensors.cpp
git submodule update --init --recursive
```

### CMake設定

```bash
mkdir build && cd build

# macOS (Metal)
cmake .. -DSTCPP_METAL=ON

# Linux/Windows (CUDA)
cmake .. -DSTCPP_CUDA=ON

# Linux (ROCm)
cmake .. -DSTCPP_ROCM=ON

# Vulkan
cmake .. -DSTCPP_VULKAN=ON
```

### ビルド実行

```bash
cmake --build . --config Release
```

## 使用方法

### モデル準備

HuggingFaceからsafetensors形式のモデルをダウンロード:

```bash
# 例: gpt-oss-20b
huggingface-cli download openai/gpt-oss-20b --local-dir ./model
```

必要なファイル:

```text
model/
├── config.json                  # モデル設定
├── tokenizer.json               # トークナイザー
├── tokenizer_config.json        # チャットテンプレート
├── model.safetensors            # 重み（単一）
└── model-00001-of-00005.safetensors  # または分割ファイル
```

### CLIで推論

```bash
./build/examples/main \
    --model ./model \
    --prompt "Hello, world!" \
    --max-tokens 100 \
    --temperature 0.7
```

### C APIで推論

```c
#include "safetensors.h"
#include <stdio.h>

int main() {
    // 初期化
    stcpp_init();

    // モデルロード
    stcpp_model* model = stcpp_model_load("./model", NULL, NULL);

    // コンテキスト作成
    stcpp_context_params params = stcpp_context_default_params();
    stcpp_context* ctx = stcpp_context_new(model, params);

    // 生成
    char output[4096];
    stcpp_sampling_params sampling = stcpp_sampling_default_params();
    stcpp_generate(ctx, "Hello, ", sampling, 100, output, sizeof(output));
    printf("%s\n", output);

    // 解放
    stcpp_context_free(ctx);
    stcpp_model_free(model);
    stcpp_free();

    return 0;
}
```

## 検証チェックリスト

### MVP検証（Phase 1）

- [ ] gpt-oss-20bモデルがロードできる
- [ ] 単一GPUで推論が実行できる
- [ ] ストリーミング出力でトークンが順次表示される
- [ ] 推論速度がHuggingFace transformers以上

### GPU検証

- [ ] Metal: Apple Silicon Macで動作
- [ ] CUDA: NVIDIA GPUで動作
- [ ] ROCm: AMD GPUで動作
- [ ] Vulkan: 対応GPUで動作

### 機能検証

- [ ] 分割safetensorsのロード
- [ ] トークナイザーの正常動作
- [ ] チャットテンプレートの適用
- [ ] continuous batching
- [ ] LoRAアダプターの適用
- [ ] プロンプトキャッシュ

## トラブルシューティング

### VRAM不足

```text
Error: STCPP_ERROR_VRAM_INSUFFICIENT
```

対策:

1. より小さいモデルを使用
2. コンテキストサイズを削減: `params.n_ctx = 1024`
3. KVキャッシュ量子化を有効化: `params.kv_cache_quant = true`

### GPUが検出されない

```text
Error: STCPP_ERROR_GPU_NOT_FOUND
```

対策:

1. GPUドライバを更新
2. CUDA/ROCm/Vulkan SDKを確認
3. 環境変数を確認: `CUDA_VISIBLE_DEVICES`, `HIP_VISIBLE_DEVICES`

### 未対応アーキテクチャ

```text
Error: STCPP_ERROR_UNSUPPORTED_ARCH
```

対策:

1. config.jsonの`architectures`を確認
2. 対応アーキテクチャ: gpt-oss, nemotron
3. 将来的にプラグインで拡張可能

## ベンチマーク

### 実行方法

```bash
./build/examples/benchmark \
    --model ./model \
    --prompt "Explain quantum computing" \
    --n-runs 10
```

### 期待値

| 指標 | 目標 |
|------|------|
| tokens/sec | HuggingFace以上 |
| TTFT | < 1秒 |
| VRAM使用量 | モデルサイズ + 20%以内 |
