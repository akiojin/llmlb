# 技術リサーチ: gptossアーキテクチャエイリアスサポート

## リサーチ課題

1. llama.cppのアーキテクチャ認識メカニズム
2. GGUFメタデータ形式とアーキテクチャ名
3. エイリアス実装方法

## 1. llama.cppのアーキテクチャ認識メカニズム

### 決定

`LLM_ARCH_NAMES`マッピングの変更 + `llm_arch_from_string`でのエイリアス対応。

### 理由

- 最小限のコード変更で後方互換性を維持
- GGUFファイルのメタデータキーとの整合性確保
- 既存の`gpt-oss`（ハイフン付き）モデルも引き続きサポート

### llama.cpp アーキテクチャ認識フロー

```text
GGUFファイル読み込み
    │
    ▼
general.architecture メタデータ取得
    │ "gptoss" or "gpt-oss"
    ▼
llm_arch_from_string() 呼び出し
    │
    ▼
LLM_ARCH_NAMES マッピング検索
    │ 見つかった
    ▼
LLM_ARCH_OPENAI_MOE 返却
    │
    ▼
対応するモデルローダー使用
    │
    ▼
ハイパーパラメータ読み込み
    │ "gptoss.context_length" など
    ▼
推論実行可能
```

## 2. GGUFメタデータ形式

### 決定

LLM runtimeが生成する`gptoss`形式に合わせる。

### 理由

- LLM runtimeが標準的に生成するGGUFと互換性確保
- ハイパーパラメータキーの一致が必要

### GGUF メタデータ例

```text
general.architecture = "gptoss"
general.name = "gpt-oss-20b"
gptoss.context_length = 32768
gptoss.embedding_length = 4096
gptoss.block_count = 32
gptoss.attention.head_count = 32
gptoss.attention.head_count_kv = 8
gptoss.feed_forward_length = 14336
```

### 問題点（修正前）

```cpp
// llama-arch.cpp の LLM_ARCH_NAMES
{ LLM_ARCH_OPENAI_MOE, "gpt-oss" }  // ← ハイフン付き

// しかしGGUFファイルは "gptoss" を使用
// → アーキテクチャ認識失敗
// → ハイパーパラメータキー不一致（gpt-oss.* vs gptoss.*）
```

## 3. エイリアス実装方法

### 決定

プライマリ名を`gptoss`に変更し、`gpt-oss`をエイリアスとして認識。

### 理由

- GGUFメタデータキーとプライマリ名の一致
- 既存の`gpt-oss`モデルの後方互換性維持
- コード変更の最小化

### 実装方法

```cpp
// llama-arch.cpp

// 1. プライマリ名を変更
static const std::map<llm_arch, const char*> LLM_ARCH_NAMES = {
    // ...
    { LLM_ARCH_OPENAI_MOE, "gptoss" },  // ← "gpt-oss" から変更
    // ...
};

// 2. エイリアス対応を追加
llm_arch llm_arch_from_string(const std::string& name) {
    // まず標準マッピングを検索
    for (const auto& [arch, arch_name] : LLM_ARCH_NAMES) {
        if (name == arch_name) {
            return arch;
        }
    }

    // エイリアス対応
    if (name == "gpt-oss") {
        return LLM_ARCH_OPENAI_MOE;
    }

    return LLM_ARCH_UNKNOWN;
}
```

### 代替案比較表

| 方式 | メリット | デメリット | 採用 |
|------|----------|------------|------|
| プライマリ名変更+エイリアス | 最小変更、両方サポート | なし | ✅ |
| 両方をマップに登録 | シンプル | マップ構造変更必要 | ❌ |
| GGUF生成側変更 | llama.cpp変更不要 | LLM runtime変更必要 | ❌ |

## 追加テンソル・グラフビルダー

### llama.cpp本家との同期

OpenAI MOEアーキテクチャの完全サポートのため、以下を追加:

**テンソル定義**:

- `LLM_TENSOR_ATTN_POST_NORM`: アテンション後の正規化
- `LLM_TENSOR_ATTN_SINKS`: アテンションシンク
- バイアステンソル: `bq`, `bk`, `bv`, `bo`, `ffn_*_b`

**グラフビルダー**:

- `llm_build_openai_moe_iswa`: ISWA (Interleaved Sparse-Window Attention) パターン
- SWA パターン設定の追加

## 参考リソース

- [llama.cpp GGUF仕様](https://github.com/ggerganov/llama.cpp/blob/master/docs/gguf.md)
- [llama.cpp アーキテクチャサポート](https://github.com/ggerganov/llama.cpp/blob/master/src/llama-arch.cpp)
- [LLM runtime GGUF生成](internal documentation)
