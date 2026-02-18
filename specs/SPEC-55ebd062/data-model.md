# データモデル: Nemotron CUDA PoC

## エンティティ定義

### SafetensorsFile

```rust
/// safetensorsファイル形式のモデル重み
pub struct SafetensorsFile {
    /// ファイルパス
    pub path: PathBuf,
    /// シャーディングの場合のインデックスファイル
    pub index_path: Option<PathBuf>,
    /// シャードファイル一覧（シャーディングの場合）
    pub shard_files: Vec<PathBuf>,
    /// ファイルサイズ（バイト）
    pub file_size: u64,
}
```

### SafetensorsIndex

```rust
/// シャーディングされたsafetensorsのインデックス
/// model.safetensors.index.json の内容
pub struct SafetensorsIndex {
    /// メタデータ
    pub metadata: SafetensorsMetadata,
    /// テンソル名からシャードファイルへのマッピング
    pub weight_map: HashMap<String, String>,
}

pub struct SafetensorsMetadata {
    /// 総パラメータ数
    pub total_size: Option<u64>,
}
```

### NemotronConfig

```rust
/// Nemotronモデル設定（config.json）
pub struct NemotronConfig {
    /// 隠れ層の次元数
    pub hidden_size: usize,
    /// アテンションヘッド数
    pub num_attention_heads: usize,
    /// Key-Valueヘッド数（GQA用）
    pub num_key_value_heads: usize,
    /// Transformer層数
    pub num_hidden_layers: usize,
    /// ボキャブラリサイズ
    pub vocab_size: usize,
    /// 中間層の次元数
    pub intermediate_size: usize,
    /// RMSNormのイプシロン
    pub rms_norm_eps: f32,
    /// RoPEのベース周波数
    pub rope_theta: f32,
    /// 最大シーケンス長
    pub max_position_embeddings: usize,
}
```

### CudaDevice

```rust
/// CUDAデバイス情報
pub struct CudaDevice {
    /// デバイスID（0始まり）
    pub device_id: i32,
    /// デバイス名
    pub name: String,
    /// Compute Capability
    pub compute_capability: (i32, i32),
    /// 総VRAMサイズ（バイト）
    pub total_memory: usize,
    /// 空きVRAMサイズ（バイト）
    pub free_memory: usize,
}
```

### ModelWeights

```rust
/// GPUにロードされたモデル重み
pub struct ModelWeights {
    /// Embedding層
    pub embed_tokens: CudaTensor,
    /// Transformer層
    pub layers: Vec<TransformerLayer>,
    /// 出力正規化
    pub norm: CudaTensor,
    /// LM Head（出力層）
    pub lm_head: CudaTensor,
}

pub struct TransformerLayer {
    /// 入力正規化
    pub input_layernorm: CudaTensor,
    /// Self-Attention Q/K/V/O
    pub self_attn_q_proj: CudaTensor,
    pub self_attn_k_proj: CudaTensor,
    pub self_attn_v_proj: CudaTensor,
    pub self_attn_o_proj: CudaTensor,
    /// Post-Attention正規化
    pub post_attention_layernorm: CudaTensor,
    /// FFN Gate/Up/Down
    pub mlp_gate_proj: CudaTensor,
    pub mlp_up_proj: CudaTensor,
    pub mlp_down_proj: CudaTensor,
}

/// GPU上のテンソル
pub struct CudaTensor {
    /// デバイスメモリポインタ
    pub ptr: *mut f16,
    /// テンソル形状
    pub shape: Vec<usize>,
    /// データ型（bf16/fp16/fp32）
    pub dtype: TensorDtype,
}
```

### InferenceSession

```rust
/// 推論セッション
pub struct InferenceSession {
    /// モデル設定
    pub config: NemotronConfig,
    /// GPUにロードされた重み
    pub weights: ModelWeights,
    /// KVキャッシュ
    pub kv_cache: Option<KvCache>,
    /// 現在のシーケンス位置
    pub position: usize,
}

pub struct KvCache {
    /// Key キャッシュ（各層）
    pub keys: Vec<CudaTensor>,
    /// Value キャッシュ（各層）
    pub values: Vec<CudaTensor>,
    /// 現在のキャッシュ長
    pub seq_len: usize,
}
```

### InferenceMetrics

```rust
/// 推論メトリクス
pub struct InferenceMetrics {
    /// モデルロード時間（ミリ秒）
    pub load_time_ms: f64,
    /// 総生成トークン数
    pub total_tokens: usize,
    /// トークン生成速度（トークン/秒）
    pub tokens_per_second: f64,
    /// ピークVRAM使用量（バイト）
    pub peak_memory_usage: usize,
}
```

## 検証ルール

| エンティティ | フィールド | ルール |
|-------------|-----------|--------|
| SafetensorsFile | path | ファイルが存在すること |
| SafetensorsFile | shard_files | すべてのシャードファイルが存在すること |
| NemotronConfig | hidden_size | 0より大きいこと |
| NemotronConfig | num_hidden_layers | 1以上であること |
| CudaDevice | compute_capability | (7, 0)以上であること |
| CudaDevice | free_memory | モデルサイズ以上であること |
| ModelWeights | layers | config.num_hidden_layersと一致すること |
| InferenceSession | position | max_position_embeddings以下であること |

## 関係図

```text
┌─────────────────────────────────────────────────────────────────┐
│                        PoC Program                               │
└────────────────────────────┬────────────────────────────────────┘
                             │
          ┌──────────────────┼──────────────────┐
          │                  │                  │
          ▼                  ▼                  ▼
┌──────────────────┐ ┌──────────────┐ ┌──────────────────┐
│ SafetensorsFile  │ │ NemotronConfig│ │   CudaDevice     │
│ - path           │ │ - hidden_size │ │ - device_id      │
│ - index_path     │ │ - num_layers  │ │ - total_memory   │
│ - shard_files    │ │ - vocab_size  │ │ - free_memory    │
└────────┬─────────┘ └───────┬───────┘ └────────┬─────────┘
         │                   │                  │
         └───────────────────┼──────────────────┘
                             │
                             ▼
                  ┌──────────────────────┐
                  │    ModelWeights      │
                  │ - embed_tokens       │
                  │ - layers[]           │
                  │ - norm               │
                  │ - lm_head            │
                  └──────────┬───────────┘
                             │
                             ▼
                  ┌──────────────────────┐
                  │  InferenceSession    │
                  │ - config             │
                  │ - weights            │
                  │ - kv_cache           │
                  │ - position           │
                  └──────────┬───────────┘
                             │
                             ▼
                  ┌──────────────────────┐
                  │  InferenceMetrics    │
                  │ - load_time_ms       │
                  │ - tokens_per_second  │
                  │ - peak_memory_usage  │
                  └──────────────────────┘
```

## ファイル構成（safetensors）

### 単一ファイル形式

```text
model/
├── config.json
├── tokenizer.json
├── tokenizer_config.json
└── model.safetensors
```

### シャーディング形式

```text
model/
├── config.json
├── tokenizer.json
├── tokenizer_config.json
├── model.safetensors.index.json
├── model-00001-of-00003.safetensors
├── model-00002-of-00003.safetensors
└── model-00003-of-00003.safetensors
```
