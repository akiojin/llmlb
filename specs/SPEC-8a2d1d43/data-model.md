# データモデル: gptossアーキテクチャエイリアスサポート

## エンティティ定義

### llm_arch（アーキテクチャ列挙型）

```cpp
/// LLMアーキテクチャ列挙型
enum llm_arch {
    LLM_ARCH_LLAMA,
    LLM_ARCH_FALCON,
    LLM_ARCH_BAICHUAN,
    LLM_ARCH_GROK,
    LLM_ARCH_GPT2,
    LLM_ARCH_GPTJ,
    LLM_ARCH_GPTNEOX,
    LLM_ARCH_MPT,
    LLM_ARCH_STARCODER,
    LLM_ARCH_REFACT,
    LLM_ARCH_BLOOM,
    LLM_ARCH_STABLELM,
    LLM_ARCH_QWEN,
    LLM_ARCH_QWEN2,
    LLM_ARCH_QWEN2MOE,
    LLM_ARCH_PHI2,
    LLM_ARCH_PHI3,
    LLM_ARCH_PLAMO,
    LLM_ARCH_CODESHELL,
    LLM_ARCH_ORION,
    LLM_ARCH_INTERNLM2,
    LLM_ARCH_MINICPM,
    LLM_ARCH_GEMMA,
    LLM_ARCH_GEMMA2,
    LLM_ARCH_STARCODER2,
    LLM_ARCH_MAMBA,
    LLM_ARCH_XVERSE,
    LLM_ARCH_COMMAND_R,
    LLM_ARCH_DBRX,
    LLM_ARCH_OLMO,
    LLM_ARCH_OPENELM,
    LLM_ARCH_ARCTIC,
    LLM_ARCH_DEEPSEEK2,
    LLM_ARCH_CHATGLM,
    LLM_ARCH_BITNET,
    LLM_ARCH_T5,
    LLM_ARCH_JAIS,
    LLM_ARCH_OPENAI_MOE,    // ← gptoss / gpt-oss
    LLM_ARCH_UNKNOWN,
};
```

### LLM_ARCH_NAMES（アーキテクチャ名マッピング）

```cpp
/// アーキテクチャ名マッピング
/// GGUFメタデータの general.architecture と対応
static const std::map<llm_arch, const char*> LLM_ARCH_NAMES = {
    { LLM_ARCH_LLAMA,           "llama"            },
    { LLM_ARCH_FALCON,          "falcon"           },
    // ...
    { LLM_ARCH_OPENAI_MOE,      "gptoss"           },  // プライマリ名
    { LLM_ARCH_UNKNOWN,         "(unknown)"        },
};
```

### GGUFメタデータ構造

```cpp
/// GGUFメタデータキー（gptossアーキテクチャ）
struct GptossMetadata {
    // 一般情報
    std::string architecture;      // "gptoss"
    std::string name;              // モデル名

    // モデルパラメータ
    uint32_t context_length;       // コンテキスト長
    uint32_t embedding_length;     // 埋め込み次元
    uint32_t block_count;          // トランスフォーマーブロック数
    uint32_t head_count;           // アテンションヘッド数
    uint32_t head_count_kv;        // KVヘッド数
    uint32_t feed_forward_length;  // FFN次元

    // MoE関連
    uint32_t expert_count;         // エキスパート数
    uint32_t expert_used_count;    // 使用エキスパート数
};
```

## 検証ルール表

| フィールド | ルール | エラーメッセージ |
|-----------|--------|------------------|
| `architecture` | "gptoss" または "gpt-oss" | "Unsupported architecture" |
| `context_length` | 0より大きい | "Invalid context length" |
| `embedding_length` | 0より大きい | "Invalid embedding length" |
| `block_count` | 0より大きい | "Invalid block count" |

## 関係図

```text
┌─────────────────────────────────────────────────────────────┐
│                      GGUF ファイル                           │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ メタデータセクション                                  │    │
│  │  general.architecture = "gptoss"                     │    │
│  │  general.name = "gpt-oss-20b"                        │    │
│  │  gptoss.context_length = 32768                       │    │
│  │  gptoss.embedding_length = 4096                      │    │
│  │  ...                                                 │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                              │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ テンソルセクション                                    │    │
│  │  token_embd.weight                                   │    │
│  │  blk.0.attn_norm.weight                              │    │
│  │  blk.0.attn_q.weight                                 │    │
│  │  ...                                                 │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                              │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    llama.cpp ローダー                        │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  llm_arch_from_string("gptoss")                             │
│      │                                                       │
│      ▼                                                       │
│  LLM_ARCH_OPENAI_MOE                                        │
│      │                                                       │
│      ▼                                                       │
│  llm_load_arch() → OpenAI MOE ローダー                       │
│      │                                                       │
│      ▼                                                       │
│  llm_build_openai_moe_iswa() → 計算グラフ構築                │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## アーキテクチャ名マッピング

| GGUF値 | llm_arch | 備考 |
|--------|----------|------|
| `gptoss` | `LLM_ARCH_OPENAI_MOE` | プライマリ（LLM runtime生成） |
| `gpt-oss` | `LLM_ARCH_OPENAI_MOE` | エイリアス（後方互換） |

## テンソルマッピング

| テンソル名 | 説明 |
|-----------|------|
| `token_embd.weight` | トークン埋め込み |
| `output_norm.weight` | 出力正規化 |
| `output.weight` | 出力重み |
| `blk.N.attn_norm.weight` | アテンション正規化 |
| `blk.N.attn_q.weight` | Query重み |
| `blk.N.attn_k.weight` | Key重み |
| `blk.N.attn_v.weight` | Value重み |
| `blk.N.attn_output.weight` | アテンション出力 |
| `blk.N.attn_post_norm.weight` | アテンション後正規化 |
| `blk.N.ffn_norm.weight` | FFN正規化 |
| `blk.N.ffn_gate.weight` | FFNゲート |
| `blk.N.ffn_up.weight` | FFN Up射影 |
| `blk.N.ffn_down.weight` | FFN Down射影 |
