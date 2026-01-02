# クイックスタート: gptossアーキテクチャエイリアスサポート

## 前提条件

- LLM Router Node がビルド済み
- gptossアーキテクチャのGGUFモデルファイル

## 基本的な使用例

### 1. gptossモデルのダウンロード

```bash
# LLM runtimeでモデルをプル
llm-runtime pull gpt-oss:20b

# または直接GGUFファイルを配置
cp gpt-oss-20b.gguf ~/.llm-router/models/
```

### 2. モデルのロード確認

```bash
# Nodeを起動
./llm-router-node --model gpt-oss-20b

# ログ出力
# [INFO] Loading model: gpt-oss-20b
# [INFO] Architecture detected: gptoss (LLM_ARCH_OPENAI_MOE)
# [INFO] Model loaded successfully
```

### 3. 推論実行

```bash
curl -X POST http://localhost:8081/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-oss-20b",
    "messages": [
      {"role": "user", "content": "Hello!"}
    ]
  }'
```

## GGUFメタデータの確認

```bash
# GGUFファイルのメタデータを確認
llama-cli --info gpt-oss-20b.gguf

# 出力例:
# general.architecture: gptoss
# general.name: gpt-oss-20b
# gptoss.context_length: 32768
# gptoss.embedding_length: 4096
# gptoss.block_count: 32
```

## サポートされる形式

### アーキテクチャ名

| GGUFメタデータ値 | サポート | 備考 |
|-----------------|----------|------|
| `gptoss` | ✅ | LLM runtime生成の標準形式 |
| `gpt-oss` | ✅ | 後方互換（エイリアス） |

### モデルバリエーション

| モデル名 | サイズ | コンテキスト長 |
|---------|--------|---------------|
| gpt-oss-20b | 20B | 32768 |
| gpt-oss-7b | 7B | 32768 |
| gpt-oss-3b | 3B | 32768 |

## エラーハンドリング

### アーキテクチャ認識エラー

```text
[ERROR] Unknown architecture: gptossv2
```

対処法:

1. GGUFファイルのメタデータを確認
2. `general.architecture`が`gptoss`または`gpt-oss`であることを確認
3. 異なるアーキテクチャの場合、対応するローダーが必要

### ハイパーパラメータ不一致

```text
[ERROR] Missing required key: gptoss.context_length
```

対処法:

1. GGUFファイルが正しく生成されているか確認
2. LLM runtimeの最新版でモデルを再生成

## 制限事項表

| 項目 | 制限値 | 備考 |
|------|--------|------|
| サポートアーキテクチャ | gptoss, gpt-oss | 両方同一処理 |
| 最大コンテキスト長 | 128K | VRAM依存 |
| 量子化形式 | Q4_K_M, Q5_K_M, Q6_K, Q8_0, F16 | GGUF標準 |

## トラブルシューティング

### モデルがロードされない

1. GGUFファイルの整合性を確認

```bash
llama-cli --validate gpt-oss-20b.gguf
```

1. アーキテクチャ名を確認

```bash
llama-cli --info gpt-oss-20b.gguf | grep architecture
```

1. Nodeログで詳細エラーを確認

```bash
./llm-router-node --model gpt-oss-20b --verbose
```

### パフォーマンスが遅い

1. 適切な量子化形式を選択（Q4_K_M推奨）
2. VRAMに収まるサイズのモデルを使用
3. コンテキスト長を必要最小限に設定
