# PoC: openai/gpt-oss-20b (Metal / Apple Silicon)

このPoCは、`openai/gpt-oss-20b` を **llm-router + llm-node (Metal)** で実際にロードして `chat/completions` が返ることを確認します。

このPoCの推論は safetensors を直接ロードするのではなく、Hugging Face の `metal/model.bin`（Routerでは `model.metal.bin` としてキャッシュ）を gpt-oss Metal エンジンで実行します。

## 前提

- macOS (Apple Silicon)
- Xcode + Metal toolchain (`xcrun -sdk macosx -find metal` が成功すること)
- `HF_TOKEN` が環境変数に設定済み（Hugging Face で `openai/gpt-oss-20b` にアクセス可能）

## 実行

まずビルド:

```bash
cmake -S node -B node/build
cmake --build node/build -j

cargo build -p llm-router
```

PoC実行:

```bash
./poc/gpt-oss-metal/run.sh
```

### 入力/リクエストの差し替え

1) 1往復のメッセージ（簡易）

```bash
USER_MESSAGE='こんにちは！一文で挨拶して' SEED=1 ./poc/gpt-oss-metal/run.sh
```

1) messages配列をファイルで渡す

```bash
MESSAGES_FILE=./poc/gpt-oss-metal/messages.json ./poc/gpt-oss-metal/run.sh
```

1) OpenAI互換のリクエストJSONをそのまま渡す（`model` は上書き注入されます）

```bash
REQUEST_FILE=./poc/gpt-oss-metal/request.json ./poc/gpt-oss-metal/run.sh
```

### 生成パラメータ

```bash
TEMPERATURE=0.2 MAX_TOKENS=64 SEED=1 ./poc/gpt-oss-metal/run.sh
```

### ストリーミング(SSE)

```bash
STREAM=1 ./poc/gpt-oss-metal/run.sh
```

### プロセス制御/ログ

- `tmp/poc-gptoss-metal/` にログと作業用ディレクトリを作成します
- `KEEP_RUNNING=1` を指定すると、PoC終了後も router/node を停止しません（デバッグ用）

### 注意

- 初回はモデルをダウンロードするため時間とディスク容量が必要です。
- `tmp/poc-gptoss-metal/` 以下にログと作業用ディレクトリを作成します。
