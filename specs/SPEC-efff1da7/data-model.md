# データモデル: Nemotron safetensors-cpp PoC

## テンソルメタデータ

### TensorInfo

safetensors から読み取るテンソル情報。

```cpp
struct TensorInfo {
    std::string name;        // テンソル名
    std::string dtype;       // データ型（BF16, F16, F32等）
    std::vector<size_t> shape;  // 形状
    size_t data_size;        // データサイズ（バイト）
};
```

### dtype 一覧

| dtype | サイズ | 用途 |
|-------|--------|------|
| BF16 | 2バイト | Nemotron の主要重み |
| F16 | 2バイト | 一部の重み |
| F32 | 4バイト | LayerNorm等 |
| I32 | 4バイト | 整数パラメータ |

## Nemotron テンソル命名規則

### 基本構造

```text
backbone.layers.{layer_idx}.{component}.{subcomponent}.{param_type}
```

### コンポーネント一覧

| コンポーネント | 説明 |
|---------------|------|
| `mixer` | Attention + MoE |
| `mlp` | Feed Forward |
| `norm` | LayerNorm |

### MoE テンソル

```text
backbone.layers.{L}.mixer.experts.{E}.{proj}.weight
```

- `L`: レイヤー番号（0-indexed）
- `E`: エキスパート番号（0-indexed）
- `proj`: down_proj, gate_proj, up_proj

## PoC 出力形式

### 集計結果

```cpp
struct AnalysisResult {
    size_t total_tensors;              // 総テンソル数
    std::map<std::string, size_t> dtype_counts;  // dtype別カウント
    size_t expert_tensors;             // experts含むテンソル数
    std::vector<std::string> problematic_tensors;  // 問題テンソル一覧
};
```

### JSON 出力例

```json
{
  "total_tensors": 1234,
  "dtype_counts": {
    "BF16": 1200,
    "F32": 34
  },
  "expert_tensors": 768,
  "problematic_tensors": [
    "backbone.layers.1.mixer.experts.0.down_proj.weight"
  ]
}
```

## シャーディング構造

### index.json

```json
{
  "weight_map": {
    "backbone.layers.0.mixer.weight": "model-00001-of-00003.safetensors",
    "backbone.layers.1.mixer.weight": "model-00001-of-00003.safetensors",
    "backbone.layers.2.mixer.weight": "model-00002-of-00003.safetensors"
  },
  "metadata": {
    "total_size": 60000000000
  }
}
```
