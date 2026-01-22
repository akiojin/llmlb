# PoC: gpt-oss-20b (CUDA / llama.cpp GGUF)

> **注意**: このPoCは参考用であり、仕様・実装の正ではありません。\
> CUDA DLL（`gptoss_cuda.dll` / `nemotron_cuda.dll`）の管理ソースは `node/engines/gptoss/cuda/` と `node/engines/nemotron/cuda/` に移行中です。

このPoCは、`gpt-oss-20b` を **llmlb + xllm (CUDA / llama.cpp)** でロードして `chat/completions` が返ることを確認します。

このPoCは **safetensors を直接ロードしません**。CUDA向けの公式safetensors実行エンジンは未対応のため、Hugging Face の **GGUF版**（例: `ggml-org/gpt-oss-20b-GGUF`）を利用します。

## 前提

- Linux (NVIDIA GPU)
- CUDA Toolkit + Driver (`nvidia-smi` が成功すること)
- Nodeは CUDA 有効でビルドしていること（下記）

## ビルド

```bash
cmake -S node -B node/build -DBUILD_WITH_CUDA=ON
cmake --build node/build -j

# または:
# npm run build:node:cuda

cargo build -p llmlb
```

## 実行

```bash
./poc/gpt-oss-cuda/run.sh
```

### 入力/リクエストの差し替え

1) 1往復のメッセージ（簡易）

```bash
USER_MESSAGE='こんにちは！一文で挨拶して' SEED=1 ./poc/gpt-oss-cuda/run.sh
```

1) messages配列をファイルで渡す

```bash
MESSAGES_FILE=./poc/gpt-oss-cuda/messages.json ./poc/gpt-oss-cuda/run.sh
```

1) OpenAI互換のリクエストJSONをそのまま渡す（`model` は上書き注入されます）

```bash
REQUEST_FILE=./poc/gpt-oss-cuda/request.json ./poc/gpt-oss-cuda/run.sh
```

### 生成パラメータ

```bash
TEMPERATURE=0.2 MAX_TOKENS=64 SEED=1 ./poc/gpt-oss-cuda/run.sh
```

### ストリーミング(SSE)

```bash
STREAM=1 ./poc/gpt-oss-cuda/run.sh
```

### プロセス制御/ログ

- `tmp/poc-gptoss-cuda/` にログと作業用ディレクトリを作成します
- `KEEP_RUNNING=1` を指定すると、PoC終了後も router/node を停止しません（デバッグ用）
