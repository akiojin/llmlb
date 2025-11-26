# INSTALL

## Prerequisites
- Linux/macOS/Windows x64 (GPU推奨、GPUなしは登録不可)
- Rust toolchain (nightly不要) と cargo
- Docker (任意、コンテナ利用時)
- CUDAドライバ (GPU使用時。NVIDIAのみ)

## 1) Rustソースからビルド（推奨）
```bash
git clone https://github.com/akiojin/ollama-router.git
cd ollama-router
make quality-checks   # fmt/clippy/test/markdownlint 一式
cargo build -p or-router --release
```
生成物: `target/release/or-router`

## 2) Docker で起動
```bash
docker build -t ollama-router:latest .
docker run --rm -p 8080:8080 --gpus all \
  -e OPENAI_API_KEY=... \
  ollama-router:latest
```
GPUを使わない場合は `--gpus all` を外すか、`CUDA_VISIBLE_DEVICES=""` を設定。

## 3) C++ Node ビルド

```bash
npm run build:node

# 手動でビルドする場合:
cd node
cmake -B build -S .
cmake --build build --config Release
```

生成物: `node/build/ollama-node`

## 4) 基本設定
- ルーター（Rust）
  - `ROUTER_HOST` (default `0.0.0.0`)
  - `ROUTER_PORT` (default `8080`)
  - `DATABASE_URL` (default `sqlite:$HOME/.or/router.db`)
  - クラウドキー: `OPENAI_API_KEY`, `GOOGLE_API_KEY`, `ANTHROPIC_API_KEY`
- ノード（C++）
  - `OLLAMA_ROUTER_URL` (例: `http://localhost:8080`)
  - `OLLAMA_NODE_PORT` (default `11434`)
  - `OLLAMA_BIND_ADDRESS` (default `0.0.0.0`)
  - `OLLAMA_ALLOW_NO_GPU` を `true` にするとGPU必須を無効化（デフォルトは禁止）

## 5) 起動例
```bash
# ルーター
cargo run -p or-router

# ノード (別シェル)
./node/build/ollama-node
```

## 6) 動作確認
- ダッシュボード: `http://localhost:8080/dashboard`
- 健康チェック: `curl http://localhost:8080/api/health`
- OpenAI互換: `curl http://localhost:8080/v1/models`

## 7) 品質チェック（必須）
```bash
make quality-checks
```
上記がすべて成功してからコミット・プッシュすること。
