# クイックスタート: llm-router CLIコマンド

**機能ID**: `SPEC-58378000` | **日付**: 2026-01-08

## 前提条件

- llm-routerバイナリがビルド済み
- GPUが搭載されている
- HuggingFaceアカウント（gatedモデル利用時）

## セットアップ

### 1. サーバー起動

```bash
# ターミナル1: サーバーをフォアグラウンドで起動
llm-router node serve
```

### 2. モデルのダウンロード

```bash
# ターミナル2: モデルをダウンロード
llm-router node pull meta-llama/Llama-3.2-3B-Instruct
```

### 3. モデルとの対話

```bash
# REPLを起動
llm-router node run meta-llama/Llama-3.2-3B-Instruct
>>> こんにちは、何ができますか？
私はAIアシスタントです。質問に答えたり...
>>> /bye
```

## 基本操作

### モデル管理

```bash
# ローカルモデル一覧
llm-router node list

# モデル詳細表示
llm-router node show meta-llama/Llama-3.2-3B-Instruct

# モデル削除
llm-router node rm meta-llama/Llama-3.2-3B-Instruct
```

### 実行中モデルの管理

```bash
# 実行中モデル一覧
llm-router node ps

# モデルをアンロード
llm-router node stop meta-llama/Llama-3.2-3B-Instruct
```

### ルーター管理（分散構成時）

```bash
# ノード一覧
llm-router router nodes

# モデル配信状況
llm-router router models

# クラスタ状態
llm-router router status
```

## 高度な使い方

### Vision入力

```bash
llm-router node run llava
>>> /path/to/image.png この画像について説明して
この画像には...
```

### Reasoning表示

```bash
# 思考過程を表示
llm-router node run deepseek-r1 --think
>>> 複雑な数学の問題を解いて
<think>まず、問題を分析すると...</think>
答えは42です。

# 思考過程を非表示
llm-router node run deepseek-r1 --hidethinking
```

### ollamaモデルの参照

```bash
# ollamaでダウンロード済みモデルを参照
llm-router node run ollama:llama3.2
```

## 環境変数

```bash
# サーバー接続先を変更
export LLM_ROUTER_HOST=192.168.1.100:32769

# デバッグログを有効化
export LLM_ROUTER_DEBUG=true

# gatedモデル用HuggingFaceトークン
export HF_TOKEN=hf_xxxxxxxxxxxxx
```

## トラブルシューティング

### サーバー接続エラー

```
Error: Failed to connect to server at 127.0.0.1:32769
```

→ `llm-router node serve` でサーバーを起動してください。

### gatedモデルのダウンロードエラー

```
Error: Unauthorized. Set HF_TOKEN environment variable.
```

→ HuggingFaceトークンを設定してください:

```bash
export HF_TOKEN=hf_xxxxxxxxxxxxx
```

### モデルが見つからない

```
Error: Model 'xxx' not found
```

→ `llm-router node list` で利用可能なモデルを確認してください。

## テスト検証シナリオ

1. **モデルダウンロード**: `pull` → `list` でモデルが追加されていることを確認
2. **REPL対話**: `run` → プロンプト入力 → 応答確認 → `/bye` で終了
3. **モデル管理**: `show` → `rm` → `list` でモデルが削除されていることを確認
4. **サーバー制御**: `serve` 起動 → Ctrl+C でグレースフル終了を確認
