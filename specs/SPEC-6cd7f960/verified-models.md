# SPEC-6cd7f960: 検証済みモデル一覧

## 概要

このドキュメントは、対応モデルとして登録される前にローカル検証を通過したモデルの記録です。
SPEC-6cd7f960 FR-1に基づき、検証なしでのモデル追加は禁止されています。

## 検証基準

### パスA: GGUF + llama.cpp

- [ ] GGUFファイルがHuggingFaceに存在する
- [ ] llama.cppでモデルをロードできる
- [ ] 基本的なテキスト生成が動作する
- [ ] メモリ使用量を計測済み

### パスB: safetensors + 内蔵エンジン

- [ ] safetensorsファイルがHuggingFaceに存在する
- [ ] 対応エンジン（gptoss_cpp/nemotron_cpp）でロードできる
- [ ] Metal（macOS）で推論テストが成功する
- [ ] メモリ使用量を計測済み

## 検証済みモデル

### GGUF + llama.cpp

| ID | 表示名 | HFリポジトリ | 検証日 | プラットフォーム | 備考 |
|----|-------|-------------|--------|-----------------|------|
| qwen2.5-7b-instruct | Qwen2.5 7B Instruct | bartowski/Qwen2.5-7B-Instruct-GGUF | 2024-10-XX | macOS | 初期実装時に検証 |
| llama3.2-3b-instruct | Llama 3.2 3B Instruct | bartowski/Llama-3.2-3B-Instruct-GGUF | 2024-10-XX | macOS | 初期実装時に検証 |
| mistral-7b-instruct | Mistral 7B Instruct | bartowski/Mistral-7B-Instruct-v0.3-GGUF | 2024-10-XX | macOS | 初期実装時に検証 |
| phi-3-mini | Phi-3 Mini | bartowski/Phi-3-mini-4k-instruct-GGUF | 2024-10-XX | macOS | 初期実装時に検証 |
| gemma-2-9b | Gemma 2 9B | bartowski/gemma-2-9b-it-GGUF | 2024-10-XX | macOS | 初期実装時に検証 |

### safetensors + 内蔵エンジン

| ID | 表示名 | HFリポジトリ | エンジン | 検証日 | プラットフォーム | 備考 |
|----|-------|-------------|---------|--------|-----------------|------|
| gpt-oss-20b | GPT-OSS 20B | openai/gpt-oss-20b | gptoss_cpp | 2026-01-02 | macOS (Metal) | Metal最適化アーティファクト確認済み（推論スモークは別途） |
| gpt-oss-120b | GPT-OSS 120B | openai/gpt-oss-120b | gptoss_cpp | 2026-01-02 | macOS (Metal) | Metal最適化アーティファクト確認済み（推論スモークは別途） |

## 検証待ちモデル（Docker Desktop Models）

### TextGeneration（GGUF検証予定）

| ID | 表示名 | HFリポジトリ | GGUFファイル | 量子化 | 検証パス | ステータス |
|----|-------|-------------|-------------|--------|---------|----------|
| kimi-k2 | Kimi K2 | TBD | TBD | TBD | A (GGUF) | 未検証 |
| ministral3 | Ministral 3 | bartowski/mistralai_Ministral-3-3B-Instruct-2512-GGUF | mistralai_Ministral-3-3B-Instruct-2512-Q4_K_M.gguf | Q4_K_M | A (GGUF) | 未検証 |
| qwen3 | Qwen3 | bartowski/Qwen_Qwen3-0.6B-GGUF | Qwen_Qwen3-0.6B-Q4_K_M.gguf | Q4_K_M | A (GGUF) | 未検証 |
| granite-4.0-nano | Granite 4.0 Nano | TBD | TBD | TBD | A (GGUF) | 未検証 |
| granite-4.0-h-nano | Granite 4.0 H Nano | TBD | TBD | TBD | A (GGUF) | 未検証 |
| smollm2 | SmolLM2 | TBD | TBD | TBD | A (GGUF) | 未検証 |
| granite-4.0-h-small | Granite 4.0 H Small | TBD | TBD | TBD | A (GGUF) | 未検証 |
| granite-4.0-h-tiny | Granite 4.0 H Tiny | TBD | TBD | TBD | A (GGUF) | 未検証 |
| granite-4.0-h-micro | Granite 4.0 H Micro | TBD | TBD | TBD | A (GGUF) | 未検証 |
| granite-4.0-micro | Granite 4.0 Micro | TBD | TBD | TBD | A (GGUF) | 未検証 |
| devstral-small | Devstral Small | TBD | TBD | TBD | A (GGUF) | 未検証 |
| magistral-small-3.2 | Magistral Small 3.2 | TBD | TBD | TBD | A (GGUF) | 未検証 |
| gemma3-qat | Gemma 3 QAT | TBD | TBD | TBD | A (GGUF) | 未検証 |
| gemma3 | Gemma 3 | TBD | TBD | TBD | A (GGUF) | 未検証 |
| qwen3-coder | Qwen3 Coder | TBD | TBD | TBD | A (GGUF) | 未検証 |
| deepseek-r1-distill-llama | DeepSeek R1 Distill | TBD | TBD | TBD | A (GGUF) | 未検証 |
| llama3.3 | Llama 3.3 | TBD | TBD | TBD | A (GGUF) | 未検証 |
| llama3.1 | Llama 3.1 | TBD | TBD | TBD | A (GGUF) | 未検証 |
| phi4 | Phi-4 | TBD | TBD | TBD | A (GGUF) | 未検証 |
| qwq | QwQ | TBD | TBD | TBD | A (GGUF) | 未検証 |
| deepcoder-preview | DeepCoder Preview | TBD | TBD | TBD | A (GGUF) | 未検証 |
| mistral-nemo | Mistral Nemo | TBD | TBD | TBD | A (GGUF) | 未検証 |

### TextGeneration（safetensors検証予定）

| ID | 表示名 | エンジン | ステータス |
|----|-------|---------|----------|
| gpt-oss-safeguard | GPT-OSS Safeguard | gptoss_cpp | 未検証（Metalアーティファクト無し） |
| seed-oss | Seed OSS | gptoss_cpp | 未検証（Metal） |

### Vision（将来対応）

| ID | 表示名 | ステータス |
|----|-------|----------|
| qwen3-vl | Qwen3 VL | 検証不可（Vision未実装） |
| smolvlm | SmolVLM | 検証不可（Vision未実装） |
| granite-docling | Granite Docling | 検証不可（Vision未実装） |
| moondream2 | Moondream2 | 検証不可（Vision未実装） |
| gemma3n | Gemma 3N | 検証不可（Vision未実装） |

### Embedding（将来対応）

| ID | 表示名 | ステータス |
|----|-------|----------|
| qwen3-reranker | Qwen3 Reranker | 検証不可（Embedding未実装） |
| qwen3-embedding | Qwen3 Embedding | 検証不可（Embedding未実装） |
| mxbai-embed-large | MxBAI Embed Large | 検証不可（Embedding未実装） |

## 検証手順テンプレート

### GGUF検証

```bash
# 1. GGUFファイルをダウンロード
huggingface-cli download bartowski/MODEL-GGUF MODEL-Q4_K_M.gguf --local-dir ./models

# 2. llama.cppでロード確認
./llama-cli -m ./models/MODEL-Q4_K_M.gguf -p "Hello" -n 50

# 3. メモリ使用量を確認
# macOS: Activity Monitor または `top -pid <PID>`
```

### safetensors検証（Metal）

```bash
# 1. safetensorsファイルをダウンロード
huggingface-cli download openai/gpt-oss-120b --local-dir ./models/gpt-oss

# 2. ルーターに登録（メタデータのみ）
curl -X POST http://localhost:32768/v0/models/register \
  -H "Content-Type: application/json" \
  -d '{"repo": "openai/gpt-oss-120b"}'

# Node が同期してダウンロードするまで待機

# 3. 推論テスト
curl http://localhost:32768/v1/chat/completions \
  -H "Authorization: Bearer sk_debug" \
  -d '{"model": "gpt-oss", "messages": [{"role": "user", "content": "Hello"}]}'
```

## 更新履歴

| 日付 | 更新内容 |
|------|---------|
| 2025-12-30 | 初版作成、Docker Desktop Modelsの検証待ちリストを追加 |
| 2026-01-02 | GPT-OSS 20B/120BのMetalアーティファクト確認結果を追記 |
