# Nemotron CUDA PoC

> **注意**: このPoCは参考用であり、仕様・実装の正ではありません。\
> CUDA DLL（`gptoss_cuda.dll` / `nemotron_cuda.dll`）の管理ソースは `node/engines/gptoss/cuda/` と `node/engines/nemotron/cuda/` に移行中です。

safetensors形式のNemotronモデルをllama.cppに依存せずにCUDAで直接ロード・推論するPoC。

## 概要

- safetensors形式のモデルを直接読み込み（mmap）
- CUDAでGPUメモリに転送
- cuBLAS + カスタムカーネルで推論
- BF16精度でのテキスト生成

## 要件

- CUDA Toolkit 12.x以上
- CUDA対応GPU（Compute Capability 7.0以上）
- CMake 3.18以上
- C++17対応コンパイラ

### GPUメモリ要件

| モデル | VRAM |
|--------|------|
| Nemotron-Mini (4B) | 8GB以上 |
| Nemotron-Medium (15B) | 24GB以上 |

## ビルド

```bash
# Linux
mkdir build && cd build
cmake .. -DCMAKE_BUILD_TYPE=Release
make -j$(nproc)

# Windows (PowerShell) - 推奨
.\build_ps.ps1

# Windows (手動)
mkdir build && cd build
cmake .. -G "Visual Studio 17 2022" -A x64
cmake --build . --config Release
```

## 使用方法

```bash
# 基本実行
./nemotron-cuda-poc --model /path/to/nemotron-mini --prompt "Hello"

# オプション
./nemotron-cuda-poc \
    --model /path/to/model \
    --prompt "Once upon a time" \
    --max-tokens 100 \
    --device 0 \
    --verbose
```

### オプション一覧

| オプション | 説明 | デフォルト |
|-----------|------|-----------|
| `--model PATH` | モデルディレクトリ（必須） | - |
| `--prompt TEXT` | 入力プロンプト | - |
| `--prompt-file FILE` | UTF-8プロンプトファイル | - |
| `--max-tokens N` | 最大生成トークン数 | 100 |
| `--device N` | CUDAデバイスID | 0 |
| `--verbose` | 詳細ログ出力 | off |

**注**: `--prompt` または `--prompt-file` のいずれかが必須。

### 日本語/マルチリンガル入力

Windows環境でのUTF-8入力の問題を回避するため、`--prompt-file` を使用:

```powershell
# UTF-8ファイルにプロンプトを保存
[System.IO.File]::WriteAllText("prompt.txt", "日本の首都は", [System.Text.Encoding]::UTF8)

# ファイルからプロンプトを読み込んで実行
.\nemotron-cuda-poc.exe --model C:\models\minitron --prompt-file prompt.txt --max-tokens 20
```

**注意**: Baseモデルは多言語出力が混在する場合があります。日本語応答を期待する場合は
Instructモデルの使用を推奨。

### モデルディレクトリ構成

```text
model_dir/
├── config.json          # モデル設定
├── tokenizer.json       # トークナイザー
└── model.safetensors    # 重み（または model.safetensors.index.json + shards）
```

## 出力例

```text
[INFO] Loading model from: /path/to/nemotron-mini
[INFO] CUDA Device: NVIDIA GeForce RTX 4090
[INFO]   Compute Capability: 8.9
[INFO]   Total Memory: 24564 MB
[INFO] Model weights loaded: 7823 MB

=== Generation ===
Prompt: Hello
Output: , I'm a new user and I'm trying to...

=== Statistics ===
Load time:       12345 ms
Prompt tokens:   2
Prompt time:     45 ms
Generated:       50 tokens
Generation time: 1234 ms
Speed:           40.5 tokens/sec
```

## エラーメッセージ

| エラー | 原因 | 対処 |
|--------|------|------|
| `CUDA is not available` | CUDA未対応環境 | CUDAドライバをインストール |
| `Version Mismatch Detected` | ドライバー/ランタイム不一致 | 下記トラブルシューティング参照 |
| `Model directory does not exist` | パス不正 | モデルパスを確認 |
| `config.json not found` | 必須ファイル欠損 | HuggingFaceからダウンロード |
| `CUDA out of memory` | VRAM不足 | より小さいモデルを使用 |

### トラブルシューティング: CUDA バージョン不一致

診断出力で `Driver API Version: 0.0` や `Version Mismatch Detected` が表示される場合:

```text
[CUDA Diagnostic]
  Driver API Version:  12.4
  Runtime API Version: 13.1
  ...
[Version Mismatch Detected]
  CUDA Runtime 13.1 requires a newer driver.
```

**解決策**:

1. **ドライバー更新** (推奨)
   - CUDA 13.x には Driver >= 580 が必要
   - [NVIDIA Driver Downloads](https://www.nvidia.com/Download/index.aspx)

2. **CUDA Toolkitを変更してリビルド**
   - ドライバーが対応するCUDA Toolkitをインストール
   - 例: Driver 553.x → CUDA 12.4 Toolkit

## アーキテクチャ

```text
safetensors (mmap)
    ↓
CPU Memory (TensorInfo)
    ↓ cudaMemcpy
GPU Memory (GpuTensor)
    ↓
Transformer Layers (cuBLAS + custom kernels)
    ↓
Logits → Sampling → Token
```

## 制限事項（PoC）

- バッチサイズ: 1のみ
- 量子化: 未対応（BF16のみ）
- マルチGPU: 未対応
- ストリーミング出力: 未対応
- Flash Attention: 簡易実装（最適化なし）

## 関連SPEC

- SPEC-83825900: この機能の仕様
- SPEC-efff1da7: safetensors mmap PoC（基盤）
- SPEC-d7feaa2c: Nodeエンジンローダー抽象化
