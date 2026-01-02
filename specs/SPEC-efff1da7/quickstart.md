# クイックスタート: Nemotron safetensors-cpp PoC

## 前提条件

- C++17 対応コンパイラ（GCC 8+, Clang 10+, MSVC 2019+）
- CMake 3.16+
- Nemotron safetensors ファイル（HuggingFace からダウンロード）

## 1. ビルド

```bash
cd poc/nemotron-safetensors-cpp

# ビルドディレクトリ作成
mkdir -p build && cd build

# CMake 設定
cmake ..

# ビルド
cmake --build . --config Release
```

## 2. モデルファイル準備

```bash
# HuggingFace CLI でダウンロード（1シャードのみ）
huggingface-cli download nvidia/nemotron-3-nano-30b-a3b \
  --include "model-00001-of-*.safetensors" \
  --local-dir ./model
```

**注意**: 完全なモデルは約60GBあります。PoC検証には1シャードで十分です。

## 3. 実行

```bash
# 基本実行
./nemotron_safetensors_poc ./model/model-00001-of-00003.safetensors

# テンソル数を制限して表示
./nemotron_safetensors_poc ./model/model-00001-of-00003.safetensors --limit 100

# 特定パターンにマッチするテンソルのみ表示
./nemotron_safetensors_poc ./model/model-00001-of-00003.safetensors --match "experts"
```

## 4. 出力例

```text
=== Nemotron safetensors-cpp PoC ===
File: ./model/model-00001-of-00003.safetensors

Summary:
  Total tensors: 412
  Dtype distribution:
    BF16: 400
    F32: 12

Expert tensors: 256
  (tensors containing 'experts' in name)

Known problematic tensors found:
  [!] backbone.layers.1.mixer.experts.0.down_proj.weight
  [!] backbone.layers.1.mixer.experts.0.gate_proj.weight
  [!] backbone.layers.1.mixer.experts.0.up_proj.weight

Conclusion:
  MoE structure detected. GGUF conversion requires additional tensor mapping.
```

## CLI オプション

| オプション | 説明 | 例 |
|-----------|------|-----|
| `--limit N` | 表示するテンソル数を制限 | `--limit 50` |
| `--match STR` | テンソル名のフィルタ | `--match "experts"` |
| `--json` | JSON形式で出力 | `--json` |
| `--verbose` | 詳細ログ出力 | `--verbose` |

## トラブルシューティング

### ファイルが大きすぎてロードできない

safetensors-cpp は mmap を使用するため、物理メモリより大きいファイルも読み込めます。
ただし、アドレス空間の制約があるため 32bit システムでは制限があります。

### テンソルが見つからない

シャーディングされたモデルの場合、目的のテンソルが別のシャードにある可能性があります。
`index.json` の `weight_map` を確認してください。
