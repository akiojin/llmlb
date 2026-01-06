# クイックスタート: Nemotron CUDA PoC

## 前提条件

| 項目 | 要件 |
|------|------|
| GPU | NVIDIA CUDA対応（Compute Capability 7.0以上） |
| CUDA Toolkit | 12.x以上 |
| OS | Linux / Windows（macOSは対象外） |
| GPUメモリ | Nemotron-Mini: 8GB以上、Medium: 24GB以上 |
| ディスク | モデルファイル用（10GB以上推奨） |

## 環境準備

### 1. CUDA Toolkitの確認

```bash
# CUDAバージョン確認
nvcc --version

# GPU情報確認
nvidia-smi
```

### 2. モデルのダウンロード

```bash
# Hugging Face CLIでダウンロード
huggingface-cli download nvidia/Nemotron-Mini-4B-Instruct \
  --local-dir ./models/nemotron-mini

# または手動でダウンロード
# https://huggingface.co/nvidia/Nemotron-Mini-4B-Instruct
```

### 3. PoCプログラムのビルド

```bash
cd poc/nemotron-cuda-cpp

# CMakeでビルド
mkdir build && cd build
cmake .. -DCMAKE_BUILD_TYPE=Release
make -j$(nproc)
```

## 基本的な使用例

### モデルのロードと推論

```bash
# 基本実行
./nemotron-poc --model ./models/nemotron-mini

# プロンプト指定
./nemotron-poc --model ./models/nemotron-mini --prompt "Hello, world!"

# トークン数指定
./nemotron-poc --model ./models/nemotron-mini \
  --prompt "Explain quantum computing" \
  --max-tokens 100
```

### 期待される出力

```text
[INFO] Loading model from: ./models/nemotron-mini
[INFO] Model config: hidden_size=2048, layers=24, vocab=128256
[INFO] Loading safetensors... (3 shards)
[INFO] Transferring to GPU... (device: NVIDIA RTX 4090)
[INFO] Model loaded in 2.34s (peak memory: 7.2GB)

Prompt: Hello, world!
Generated:
Hello, world! How can I assist you today? I'm here to help with any
questions or tasks you might have.

[INFO] Generated 25 tokens in 0.89s (28.1 tokens/sec)
```

## エラーハンドリング

### よくあるエラーと対処法

| エラーメッセージ | 原因 | 対処法 |
|-----------------|------|--------|
| `CUDA not available` | CUDAドライバ未インストール | CUDA Toolkitをインストール |
| `File not found: model.safetensors` | モデルパスが不正 | パスを確認 |
| `Out of memory` | VRAMが不足 | より小さいモデルを使用 |
| `Unsupported compute capability` | GPUが古い | CC 7.0以上のGPUを使用 |
| `Failed to parse config.json` | config.jsonが不正 | モデルを再ダウンロード |

### デバッグモード

```bash
# 詳細ログを有効化
./nemotron-poc --model ./models/nemotron-mini --verbose

# GPU情報のみ表示
./nemotron-poc --gpu-info
```

## 制限事項

| 項目 | 制限 |
|------|------|
| 対応モデル | Nemotronアーキテクチャのみ |
| 対応形式 | safetensors（BF16/FP16） |
| マルチGPU | 非対応（単一GPUのみ） |
| ストリーミング | 非対応 |
| 量子化 | 非対応（フル精度のみ） |
| Metal | 非対応（CUDA限定） |

## トラブルシューティング

### CUDA関連

```bash
# CUDAライブラリのパス確認
echo $LD_LIBRARY_PATH
export LD_LIBRARY_PATH=/usr/local/cuda/lib64:$LD_LIBRARY_PATH

# GPUメモリ使用状況確認
nvidia-smi --query-gpu=memory.used,memory.free --format=csv
```

### モデルロード失敗時

```bash
# safetensorsファイルの整合性確認
python3 -c "
from safetensors import safe_open
with safe_open('./models/nemotron-mini/model.safetensors', framework='pt') as f:
    print(f.keys())
"

# config.jsonの確認
cat ./models/nemotron-mini/config.json | jq .
```

## 次のステップ

- 本番統合: SPEC-d7feaa2c（Nodeエンジンローダー抽象化）
- Metal対応: 別途SPECで対応予定
- 量子化対応: 別途SPECで対応予定
