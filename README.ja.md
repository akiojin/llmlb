# LLM Router

[English](./README.md) | 日本語

## 概要

LLM Router は、複数マシンに配置した C++ ノード（llama.cpp）を統合し、単一の OpenAI 互換 API
（`/v1/*`）を提供する Rust ルーターです。

## 主な特徴

- OpenAI 互換 API: `/v1/chat/completions`, `/v1/completions`, `/v1/embeddings`, `/v1/models`
- ロードバランシング: 利用可能なノードへ自動ルーティング
- ダッシュボード: `/dashboard` でノード、リクエスト履歴、ログ、モデルを管理
- ノード自己登録: ノードは起動時にルーターへ登録し、ハートビートを送信
- ノード主導モデル同期: ノードはルーターの `/v0/models` と `/v0/models/blob/:model_name` を参照して
  必要なモデルを取得（ルーターからの push 配布なし）
- クラウドプレフィックス: `openai:`, `google:`, `anthropic:` を `model` に付けて同一エンドポイントでプロキシ

## ダッシュボード

ルーターが `/dashboard` で提供します。

```
http://localhost:8080/dashboard
```

## LLM アシスタント向け MCP サーバー

LLM アシスタント（Claude Code など）は、専用の MCP サーバーを通じて LLM Router と連携できます。

### インストール

```bash
npm install -g @llm-router/mcp-server
# または
npx @llm-router/mcp-server
```

### 設定例 (.mcp.json)

```json
{
  "mcpServers": {
    "llm-router": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@llm-router/mcp-server"],
      "env": {
        "LLM_ROUTER_URL": "http://localhost:8080",
        "LLM_ROUTER_API_KEY": "sk_your_api_key"
      }
    }
  }
}
```

詳細は [mcp-server/README.md](./mcp-server/README.md) を参照してください。

## インストールと起動

### 前提条件
- Linux/macOS/Windows x64 (GPU推奨、GPUなしは登録不可)
- Rust toolchain (nightly不要) と cargo
- Docker (任意、コンテナ利用時)
- CUDAドライバ (GPU使用時。NVIDIAのみ)

### 1) Rustソースからビルド（推奨）
```bash
git clone https://github.com/akiojin/llm-router.git
cd llm-router
make quality-checks   # fmt/clippy/test/markdownlint 一式
cargo build -p llm-router --release
```
生成物: `target/release/llm-router`

### 2) Docker で起動
```bash
docker build -t llm-router:latest .
docker run --rm -p 8080:8080 --gpus all \
  -e OPENAI_API_KEY=... \
  llm-router:latest
```
GPUを使わない場合は `--gpus all` を外すか、`CUDA_VISIBLE_DEVICES=""` を設定。

### 3) C++ Node ビルド

```bash
npm run build:node

# 手動でビルドする場合:
cd node
cmake -B build -S .
cmake --build build --config Release
```

生成物: `node/build/llm-node`

### 4) 基本設定

#### ルーター（Rust）環境変数

| 環境変数 | デフォルト | 説明 |
|---------|-----------|------|
| `LLM_ROUTER_HOST` | `0.0.0.0` | バインドアドレス |
| `LLM_ROUTER_PORT` | `8080` | リッスンポート |
| `LLM_ROUTER_DATABASE_URL` | `sqlite:~/.llm-router/router.db` | データベースURL |
| `LLM_ROUTER_JWT_SECRET` | 自動生成 | JWT署名シークレット |
| `LLM_ROUTER_ADMIN_USERNAME` | `admin` | 初期管理者ユーザー名 |
| `LLM_ROUTER_ADMIN_PASSWORD` | - | 初期管理者パスワード |
| `LLM_ROUTER_LOG_LEVEL` | `info` | ログレベル |
| `LLM_ROUTER_HEALTH_CHECK_INTERVAL` | `30` | ヘルスチェック間隔（秒） |
| `LLM_ROUTER_NODE_TIMEOUT` | `60` | ノードタイムアウト（秒） |
| `LLM_ROUTER_LOAD_BALANCER_MODE` | `auto` | ロードバランサーモード |
| `LLM_QUANTIZE_BIN` | - | `llama-quantize` のパス（Q4/Q5等の量子化用） |

クラウドAPI:

- `OPENAI_API_KEY`, `GOOGLE_API_KEY`, `ANTHROPIC_API_KEY`

#### ノード（C++）環境変数

| 環境変数 | デフォルト | 説明 |
|---------|-----------|------|
| `LLM_ROUTER_URL` | `http://127.0.0.1:11434` | ルーターURL |
| `LLM_NODE_API_KEY` | - | ノード登録用APIキー（スコープ: `node:register`） |
| `LLM_NODE_PORT` | `11435` | HTTPサーバーポート |
| `LLM_NODE_MODELS_DIR` | `~/.runtime/models` | モデルディレクトリ |
| `LLM_NODE_BIND_ADDRESS` | `0.0.0.0` | バインドアドレス |
| `LLM_NODE_HEARTBEAT_SECS` | `10` | ハートビート間隔（秒） |
| `LLM_NODE_LOG_LEVEL` | `info` | ログレベル |
| `LLM_NODE_LOG_DIR` | `~/.llm-router/logs` | ログディレクトリ |

**注意**: 旧環境変数名（`ROUTER_HOST`, `LLM_MODELS_DIR`等）は非推奨です。
新しい環境変数名を使用してください。

### 5) 起動例
```bash
# ルーター
cargo run -p llm-router

# ノード (別シェル)
LLM_NODE_API_KEY=sk_node_register_key ./node/build/llm-node
```

### 6) 動作確認
- ダッシュボード: `http://localhost:8080/dashboard`
- 健康チェック: `curl -H "Authorization: Bearer sk_node_register_key" -H "X-Node-Token: <node_token>" http://localhost:8080/v0/health`
- OpenAI互換: `curl -H "Authorization: Bearer sk_api_key" http://localhost:8080/v1/models`

## 利用方法（OpenAI互換エンドポイント）

### 基本
- `POST /v1/chat/completions`
- `POST /v1/completions`
- `POST /v1/embeddings`

### クラウドモデルプレフィックス
- 付けるだけでクラウド経路に切替: `openai:`, `google:`, `anthropic:`（`ahtnorpic:` も許容）
- 例: `model: "openai:gpt-4o"` / `model: "google:gemini-1.5-pro"` / `model: "anthropic:claude-3-opus"`
- 転送時にプレフィックスは除去され、クラウドAPIへそのまま送られます。
- プレフィックスなしのモデルは従来どおりローカルLLMにルーティングされます。

### ストリーミング
- `stream: true` でクラウドSSE/チャンクをそのままパススルー。

### メトリクス
- `GET /v0/metrics/cloud` （Prometheus text）
  - `cloud_requests_total{provider,status}`
  - `cloud_request_latency_seconds{provider}`

## アーキテクチャ

LLM Router は、ローカルの llama.cpp ノードを調整し、オプションでモデルのプレフィックスを介してクラウド LLM プロバイダーにプロキシします。

### コンポーネント
- **Router (Rust)**: OpenAI 互換のトラフィックを受信し、パスを選択してリクエストをプロキシします。ダッシュボード、メトリクス、管理 API を公開します。
- **Local Nodes (C++ / llama.cpp)**: GGUF モデルを提供します。ルーターに登録し、ハートビートを送信します。
- **Cloud Proxy**: モデル名が `openai:`, `google:`, `anthropic:` で始まる場合、ルーターは対応するクラウド API に転送します。
- **Storage**: ルーターのメタデータ用の SQLite。モデルファイルは各ノードに存在します。
- **Observability**: Prometheus メトリクス、構造化ログ、ダッシュボード統計。

### システム構成

![システム構成](docs/diagrams/architecture.readme.ja.svg)

Draw.ioソース: `docs/diagrams/architecture.drawio`（Page: システム構成 (README.ja.md)）

### リクエストフロー
```
Client
  │ POST /v1/chat/completions
  ▼
Router (OpenAI-compatible)
  ├─ Prefix? → Cloud API (OpenAI / Google / Anthropic)
  └─ No prefix → Scheduler → Local Node
                       └─ llama.cpp inference → Response
```

### モデル同期（push配布なし）

- ルーターは、登録・変換・キャッシュされたモデルだけを `/v1/models` に掲載します。
- ノードは `/v1/models` を参照してモデル一覧を取得します。
  - `path` が共有ストレージ等で参照可能なら、そのパスを直接使用します。
  - 参照できない場合は `/v0/models/blob/:model_name` からダウンロードしてローカルに保存します。
- ルーターからノードへの push 配布は行いません。

### スケジューリングとヘルスチェック
- ノードは `/v0/nodes` を介して登録します。ルーターはデフォルトで GPU のないノードを拒否します。
- ハートビートには、ロードバランシングに使用される CPU/GPU/メモリメトリクスが含まれます。
- ダッシュボードには `*_key_present` フラグが表示され、オペレーターはどのクラウドキーが設定されているかを確認できます。

## トラブルシューティング

### 起動時に GPU が見つからない
- 確認: `nvidia-smi` または `CUDA_VISIBLE_DEVICES`
- 環境変数で無効化: ノード側 `LLM_ALLOW_NO_GPU=true`（デフォルトは禁止）
- それでも失敗する場合は NVML ライブラリの有無を確認

### クラウドモデルが 401/400 を返す
- ルーター側で `OPENAI_API_KEY` / `GOOGLE_API_KEY` / `ANTHROPIC_API_KEY` が設定されているか確認
- ダッシュボード `/v0/dashboard/stats` の `*_key_present` が false なら未設定
- プレフィックスなしモデルはローカルにルーティングされるので、クラウドキーなしで利用したい場合はプレフィックスを付けない

### ポート競合で起動しない
- ルーター: `LLM_ROUTER_PORT` を変更（例: `LLM_ROUTER_PORT=18080`）
- ノード: `LLM_NODE_PORT` または `--port` で変更

### SQLite ファイル作成に失敗
- `LLM_ROUTER_DATABASE_URL` のパス先ディレクトリの書き込み権限を確認
- Windows の場合はパスにスペースが含まれていないか確認

### ダッシュボードが表示されない
- ブラウザキャッシュをクリア
- バンドル済み静的ファイルが壊れていないか `cargo clean` → `cargo run` を試す
- リバースプロキシ経由の場合は `/dashboard/*` の静的配信設定を確認

### OpenAI互換APIで 503 / モデル未登録
- 全ノードが `initializing` の場合 503 を返すことがあります。ノードのモデルロードを待つか、`/v0/dashboard/nodes` で状態を確認
- モデル指定がローカルに存在しない場合、ノードが自動プルするまで待機

### ログが多すぎる / 少なすぎる
- 環境変数 `LLM_ROUTER_LOG_LEVEL` または `RUST_LOG` で制御（例: `LLM_ROUTER_LOG_LEVEL=info` または `RUST_LOG=or_router=debug`）
- ノードのログは `spdlog` で出力。構造化ログは `tracing_subscriber` でJSON設定可

## モデル管理（Hugging Face, safetensors / GGUF）

- オプション環境変数: レートリミット回避に `HF_TOKEN`、社内ミラー利用時は `HF_BASE_URL` を指定します。
- Web（推奨）:
  - ダッシュボード → **Models** → **Register**
  - `format` を選択します: `safetensors`（新エンジン: TBD） または `gguf`（llama.cpp フォールバック）
    - 同一repoに safetensors と GGUF が両方ある場合、`format` は必須です。
    - 補足: `safetensors` でのテキスト生成は TBD（推論エンジン実装は後で決める）です。現時点で実行したい場合は `gguf` を選択してください。
  - Hugging Face repo（例: `nvidia/NVIDIA-Nemotron-3-Nano-30B-A3B-BF16`）を入力します。
  - `format=gguf` の場合:
    - 目的の `.gguf` を `filename` で直接指定するか、`gguf_policy`（`quality` / `memory` / `speed`）で siblings から自動選択します。
  - `format=safetensors` の場合:
    - HFスナップショットに `config.json` と `tokenizer.json` が必要です。
    - シャーディングされている場合は `.index.json` が必要です。
  - モデルIDは Hugging Face の repo ID（例: `org/model`）です。
  - `/v1/models` は、ダウンロード中/待機中/失敗も含め `lifecycle_status` と `download_progress` を返します。
- ノードはモデルをプッシュ配布されず、オンデマンドでルーターから取得します:
  - `GET /v0/models/registry/:model_name/manifest.json`
  - `GET /v0/models/registry/:model_name/files/:file_name`
  - （互換）単一GGUFのみ: `GET /v0/models/blob/:model_name`

## API 仕様

### 認証・権限

#### ユーザー（JWT）

| ロール | 権限 |
|-------|------|
| `admin` | `/v0` 管理系 API にアクセス可能 |
| `viewer` | `/v0/auth/*` のみ（管理 API は 403） |

#### APIキー（スコープ）

| スコープ | 目的 |
|---------|------|
| `node:register` | ノード登録 + ヘルスチェック + モデル同期（`POST /v0/nodes`, `POST /v0/health`, `GET /v0/models`, `GET /v0/models/registry/*`） |
| `api:inference` | OpenAI 互換推論 API（`/v1/*`） |
| `admin:*` | 管理系 API 全般（`/v0/users`, `/v0/api-keys`, `/v0/models/*`, `/v0/nodes/*`, `/v0/dashboard/*`, `/v0/metrics/*`） |

**補足**:
- `/v0/auth/login` は無認証、`/v0/health` は APIキー（`node:register`）+ `X-Node-Token` 必須。
- デバッグビルドでは `sk_debug*` 系 API キーが利用可能（`docs/authentication.md` 参照）。

### ルーター（Router）

#### OpenAI 互換（API キー認証）

- POST `/v1/chat/completions`
- POST `/v1/completions`
- POST `/v1/embeddings`
- GET `/v1/models`（API キーまたは `X-Node-Token`）
- GET `/v1/models/:model_id`（API キーまたは `X-Node-Token`）

#### ノード管理

- POST `/v0/nodes`（登録、APIキー: `node:register`）
- GET `/v0/nodes`（一覧、admin権限）
- DELETE `/v0/nodes/:node_id`（admin権限）
- POST `/v0/nodes/:node_id/disconnect`（admin権限）
- PUT `/v0/nodes/:node_id/settings`（admin権限）
- POST `/v0/health`（ノードからのヘルス/メトリクス送信、APIキー: `node:register` + `X-Node-Token`）
- GET `/v0/nodes/:node_id/logs`（admin権限）

#### モデル管理

- GET `/v0/models`（登録済みモデル一覧、APIキー: `node:register`）
- POST `/v0/models/register`（admin権限）
- DELETE `/v0/models/*model_name`（admin権限）
- POST `/v0/models/discover-gguf`（admin権限）
- GET `/v0/models/registry/:model_name/manifest.json`（APIキー: `node:register`）
- GET `/v0/models/registry/:model_name/files/:file_name`（APIキー: `node:register`）
- GET `/v0/models/blob/:model_name`（互換: 単一GGUFのみ）

#### ダッシュボード/監視

- GET `/v0/dashboard/overview`（admin権限）
- GET `/v0/dashboard/stats`（admin権限）
- GET `/v0/dashboard/nodes`（admin権限）
- GET `/v0/dashboard/metrics/:node_id`（admin権限）
- GET `/v0/dashboard/request-history`（admin権限）
- GET `/v0/dashboard/request-responses`（admin権限）
- GET `/v0/dashboard/request-responses/:id`（admin権限）
- GET `/v0/dashboard/request-responses/export`（admin権限）
- GET `/v0/dashboard/logs/router`（admin権限）
- GET `/v0/metrics/cloud`（admin権限）
- GET `/dashboard/*`
- GET `/playground/*`

### ノード（Node）

#### OpenAI 互換

- GET `/v1/models`
- POST `/v1/chat/completions`
- POST `/v1/completions`
- POST `/v1/embeddings`

#### 運用

- GET `/health`
- GET `/startup`
- GET `/metrics`
- GET `/metrics/prom`
- GET `/v0/logs?tail=200`
- GET `/log/level`
- POST `/log/level`

## 開発

```bash
make quality-checks
```

Dashboard を更新する場合:

```bash
pnpm install
pnpm --filter @llm-router/dashboard build
```

## ライセンス

MIT License

## 貢献

Issue、Pull Request をお待ちしています。

詳細な開発ガイドラインは [CLAUDE.md](./CLAUDE.md) を参照してください。
