# Data Model: safetensors.cpp

**機能ID**: SPEC-69549000
**日付**: 2026-01-06

## 概要

safetensors.cppのデータモデル定義。ggmlテンソルを中心とした設計。

## 主要エンティティ

### Model

safetensors形式のLLMモデルを表す。

```text
Model
├── path: string              # モデルファイルパス
├── config: ModelConfig       # config.jsonから読み込んだ設定
├── architecture: Architecture # アーキテクチャ定義（in-process）
├── tensors: map<string, ggml_tensor*>  # 重みテンソル
├── vocab_size: int           # 語彙サイズ
├── n_layers: int             # レイヤー数
├── n_heads: int              # アテンションヘッド数
├── n_kv_heads: int           # KVヘッド数（GQA/MQA用）
├── hidden_size: int          # 隠れ層サイズ
├── max_position_embeddings: int  # 最大コンテキスト長
├── rope_scaling: RopeScaling # Rope Scaling設定
└── dtype: DataType           # データ型（FP16/BF16/Q4_K_M等）
```

### ModelConfig

HuggingFaceのconfig.jsonから読み込む設定。

```text
ModelConfig
├── model_type: string        # モデルタイプ（gpt-oss, nemotron等）
├── architectures: []string   # アーキテクチャ名リスト
├── vocab_size: int
├── hidden_size: int
├── num_hidden_layers: int
├── num_attention_heads: int
├── num_key_value_heads: int  # GQA/MQA用
├── intermediate_size: int
├── max_position_embeddings: int
├── rope_theta: float         # RoPE基底周波数
├── rope_scaling: RopeScalingConfig  # Rope Scaling設定
├── sliding_window: int       # Sliding Window Attention
└── torch_dtype: string       # 元のデータ型
```

### Context

推論実行のためのコンテキスト。

```text
Context
├── model: Model*             # 関連モデル
├── backend: Backend          # GPUバックエンド
├── kv_cache: KVCache         # KVキャッシュ
├── batch: Batch              # バッチ管理
├── n_ctx: int                # コンテキストサイズ
├── n_batch: int              # バッチサイズ
├── n_threads: int            # CPUスレッド数
├── seed: int                 # 乱数シード
└── log_callback: LogCallback # ログコールバック
```

### Backend

GPUバックエンド情報。

```text
Backend
├── type: BackendType         # METAL / CUDA / ROCM / VULKAN
├── device_id: int            # デバイスID
├── device_name: string       # デバイス名
├── vram_total: size_t        # 総VRAM
├── vram_free: size_t         # 空きVRAM
└── compute_capability: string # 計算能力（CUDA用）
```

### KVCache

KVキャッシュ管理。

```text
KVCache
├── k: ggml_tensor*           # Keyキャッシュ
├── v: ggml_tensor*           # Valueキャッシュ
├── n_ctx: int                # キャッシュサイズ
├── head: int                 # 現在位置
├── quantization: KVQuantType # 量子化タイプ（NONE/INT8/FP8）
└── prompt_cache: PromptCache # プロンプトキャッシュ
```

### PromptCache

プロンプトキャッシュ（KV再利用）。

```text
PromptCache
├── hash: uint64              # プロンプトハッシュ
├── tokens: []int             # キャッシュ済みトークン
├── k_cache: ggml_tensor*     # Keyキャッシュスナップショット
├── v_cache: ggml_tensor*     # Valueキャッシュスナップショット
└── n_tokens: int             # トークン数
```

### Tokenizer

HuggingFace互換トークナイザー。

```text
Tokenizer
├── vocab: map<string, int>   # 語彙マップ
├── merges: []BPEMerge        # BPEマージルール
├── special_tokens: SpecialTokens  # 特殊トークン
├── chat_template: string     # Jinja2テンプレート
└── add_bos_token: bool       # BOSトークン追加フラグ
```

### SpecialTokens

特殊トークン定義。

```text
SpecialTokens
├── bos_token: Token          # 開始トークン
├── eos_token: Token          # 終了トークン
├── pad_token: Token          # パディングトークン
├── unk_token: Token          # 未知トークン
└── additional: map<string, Token>  # 追加特殊トークン
```

### Sampler

サンプリングパラメータ。

```text
Sampler
├── temperature: float        # 温度（デフォルト: 1.0）
├── top_p: float              # Top-p（デフォルト: 1.0）
├── top_k: int                # Top-k（デフォルト: -1 = 無効）
├── min_p: float              # Min-p（デフォルト: 0.0）
├── repeat_penalty: float     # 繰り返しペナルティ
├── presence_penalty: float   # 存在ペナルティ
├── frequency_penalty: float  # 頻度ペナルティ
└── seed: int                 # 乱数シード
```

### Batch

continuous batching用リクエストバッチ。

```text
Batch
├── requests: []Request       # リクエストリスト
├── n_tokens: int             # 総トークン数
├── max_tokens: int           # 最大トークン数
└── is_full: bool             # バッチが満杯か
```

### Request

単一の推論リクエスト。

```text
Request
├── id: uint64                # リクエストID
├── tokens: []int             # 入力トークン
├── n_past: int               # 処理済みトークン数
├── n_predict: int            # 生成するトークン数
├── sampler: Sampler          # サンプリング設定
├── callback: StreamCallback  # ストリーミングコールバック
├── state: RequestState       # 状態（PENDING/RUNNING/DONE/CANCELLED）
└── output: []int             # 出力トークン
```

### LoRA

LoRAアダプター。

```text
LoRA
├── path: string              # アダプターファイルパス
├── name: string              # アダプター名
├── scale: float              # スケール係数（デフォルト: 1.0）
├── tensors: map<string, ggml_tensor*>  # LoRAテンソル
└── is_applied: bool          # 適用済みフラグ
```

### Architecture

モデルアーキテクチャ（in-process）。

```text
Architecture
├── name: string              # アーキテクチャ名
├── build_graph: Function     # 計算グラフ構築関数
├── supports_embeddings: bool # 埋め込み対応
├── supports_vision: bool     # Vision対応
└── default_rope_scaling: RopeScalingType  # デフォルトRoPE
```

## Enum定義

### BackendType

```text
METAL   = 0   # Apple Metal
CUDA    = 1   # NVIDIA CUDA
ROCM    = 2   # AMD ROCm/HIP
VULKAN  = 3   # Vulkan
```

### DataType

```text
F32     = 0   # 32-bit float
F16     = 1   # 16-bit float
BF16    = 2   # Brain float 16
Q8_0    = 3   # 8-bit quantization
Q4_0    = 4   # 4-bit quantization
Q4_K_M  = 5   # K-quants 4-bit medium
Q5_K_M  = 6   # K-quants 5-bit medium
Q6_K    = 7   # K-quants 6-bit
```

### KVQuantType

```text
NONE    = 0   # 量子化なし
INT8    = 1   # INT8量子化
FP8     = 2   # FP8量子化
```

### RequestState

```text
PENDING   = 0   # 待機中
RUNNING   = 1   # 実行中
DONE      = 2   # 完了
CANCELLED = 3   # キャンセル
ERROR     = 4   # エラー
```

### RopeScalingType

```text
NONE    = 0   # スケーリングなし
LINEAR  = 1   # 線形スケーリング
NTK     = 2   # NTKスケーリング
YARN    = 3   # YaRNスケーリング
```

## 関係図

```text
┌─────────┐      ┌─────────┐
│  Model  │◄─────│ Context │
└────┬────┘      └────┬────┘
     │                │
     │                ├───► Backend
     │                ├───► KVCache ───► PromptCache
     │                └───► Batch ───► Request[]
     │
     ├───► ModelConfig
     ├───► Architecture
     ├───► Tokenizer ───► SpecialTokens
     └───► LoRA[]
```

## 状態遷移

### Request状態

```text
┌─────────┐
│ PENDING │
└────┬────┘
     │ batch_add()
     ▼
┌─────────┐     cancel()     ┌───────────┐
│ RUNNING │──────────────────►│ CANCELLED │
└────┬────┘                  └───────────┘
     │
     ├── success ──► DONE
     └── error ────► ERROR
```
