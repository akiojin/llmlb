# リサーチ: Nemotron safetensors-cpp PoC

## 調査目的

Nemotron モデルの safetensors 構造を解析し、GGUF 変換可否を判断するための技術調査。

## safetensors-cpp

### 概要

| 項目 | 内容 |
|------|------|
| リポジトリ | [syoyo/safetensors-cpp](https://github.com/syoyo/safetensors-cpp) |
| ライセンス | MIT |
| 実装形式 | ヘッダーオンリー |
| 依存関係 | なし（STLのみ） |

### 機能

- mmap によるメモリ効率的なファイル読み込み
- テンソルメタデータ（名前、dtype、shape）の解析
- ゼロコピーでのテンソルデータアクセス

### API

```cpp
#include "safetensors.hh"

safetensors::safetensors_t st;
std::string err;
bool ret = safetensors::load_from_mmap(filename, &st, &err);

for (const auto& [name, tensor] : st.tensors) {
    // tensor.dtype: F16, BF16, F32, etc.
    // tensor.shape: vector<size_t>
    // tensor.data: const uint8_t*
}
```

## Nemotron テンソル構造

### 調査対象

- Nemotron 3 Nano 30B A3B (BF16)
- safetensors シャーディング形式

### 既知の問題点

llama.cpp の `convert_hf_to_gguf.py` で変換失敗するテンソル:

```text
backbone.layers.1.mixer.experts.0.down_proj.weight
backbone.layers.1.mixer.experts.0.gate_proj.weight
backbone.layers.1.mixer.experts.0.up_proj.weight
...
```

### MoE (Mixture of Experts) 構造

```text
backbone.layers.{layer_idx}.mixer.experts.{expert_idx}.{proj_type}.weight
```

- `layer_idx`: レイヤー番号
- `expert_idx`: エキスパート番号
- `proj_type`: down_proj, gate_proj, up_proj

## PoC 検証項目

1. safetensors ファイルの mmap ロード
2. テンソル総数のカウント
3. dtype 別の集計
4. `experts` を含むテンソル名の検出
5. 既知の問題テンソルの存在確認

## 期待される出力例

```text
Loaded: model-00001-of-00003.safetensors
Total tensors: 1234
  BF16: 1200
  F32: 34
Tensors containing 'experts': 768
Found known problematic tensor: backbone.layers.1.mixer.experts.0.down_proj.weight
```

## 参考資料

- [safetensors-cpp](https://github.com/syoyo/safetensors-cpp)
- [Hugging Face safetensors](https://huggingface.co/docs/safetensors)
- [Nemotron Architecture](https://developer.nvidia.com/blog/nvidia-nemotron-architecture/)
