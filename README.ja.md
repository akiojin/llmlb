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
- ノード主導モデル同期: ノードはルーターの `/v1/models` と `/v0/models/blob/:model_name` を参照して
  必要なモデルを取得（ルーターからの push 配布なし）
- クラウドプレフィックス: `openai:`, `google:`, `anthropic:` を `model` に付けて同一エンドポイントでプロキシ

クイックリファレンス: [INSTALL](./INSTALL.md) / [USAGE](./USAGE.md) /
[TROUBLESHOOTING](./TROUBLESHOOTING.md)

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

## クイックスタート

### ルーター (llm-router)

```bash
cargo build --release -p llm-router
./target/release/llm-router
# デフォルト: http://0.0.0.0:8080
```

### ノード (llm-node)

```bash
cmake -S node -B node/build
cmake --build node/build -j

LLM_ROUTER_URL=http://localhost:8080 ./node/build/llm-node
```

### ノード環境変数（主要）

| 変数 | デフォルト | 説明 |
|-----|-----------|------|
| `LLM_ROUTER_URL` | `http://localhost:8080` | ルーターURL |
| `LLM_NODE_PORT` | `11435` | ノードのリッスンポート |
| `LLM_NODE_MODELS_DIR` | `~/.llm-router/models` | モデル保存ディレクトリ |
| `LLM_NODE_HEARTBEAT_SECS` | `10` | ハートビート間隔（秒） |
| `LLM_NODE_BIND_ADDRESS` | `0.0.0.0` | バインドアドレス |
| `LLM_NODE_LOG_LEVEL` | `info` | ログレベル |

## アーキテクチャ

### システム構成

```
┌───────────────────────────────────────┐
│               Client                  │
│     (apps / users / integrations)     │
└───────────────────┬───────────────────┘
                    │ OpenAI-compatible (/v1/*)
                    ▼
┌───────────────────────────────────────┐
│                Router                 │
│  - /v1/* proxy + load balancing       │
│  - /v0/* (nodes, models, dashboard)   │
│  - /dashboard (Web UI)                │
└──────────────┬───────────────┬────────┘
               │               │
               │ OpenAI-compatible (/v1/*)
               ▼               ▼
        ┌────────────┐   ┌────────────┐
        │    Node    │   │    Node    │
        │ llama.cpp  │   │ llama.cpp  │
        └────────────┘   └────────────┘
               ▲               ▲
               │ /v0/nodes, /v0/health (registration/heartbeat)
               └───────────────┘
```

### モデル同期（push配布なし）

- ルーターは、登録・変換・キャッシュされたモデルだけを `/v1/models` に掲載します。
- ノードは `/v1/models` を参照してモデル一覧を取得します。
  - `path` が共有ストレージ等で参照可能なら、そのパスを直接使用します。
  - 参照できない場合は `/v0/models/blob/:model_name` からダウンロードしてローカルに保存します。
- ルーターからノードへの push 配布は行いません。

## モデル管理（Hugging Face, GGUF-first）

- オプション環境変数: レートリミット回避に `HF_TOKEN`、社内ミラー利用時は `HF_BASE_URL` を指定します。
- Web（推奨）:
  - ダッシュボード → **Models** → **Register**
  - Hugging Face repo（例: `TheBloke/Llama-2-7B-GGUF`）と、任意で filename を入力します。
  - `/v1/models` は、ルーターのファイルシステム上にキャッシュ済みのモデルだけを返します。

モデル ID はファイル名ベース形式に正規化されます（例: `llama-2-7b`, `gpt-oss-20b`）。

## API 仕様

### ルーター（Router）

#### OpenAI 互換（API キー認証）

- POST `/v1/chat/completions`
- POST `/v1/completions`
- POST `/v1/embeddings`
- GET `/v1/models`（API キーまたは `X-Node-Token`）
- GET `/v1/models/:model_id`（API キーまたは `X-Node-Token`）

#### ノード管理

- POST `/v0/nodes`（登録）
- GET `/v0/nodes`（一覧）
- DELETE `/v0/nodes/:node_id`
- POST `/v0/nodes/:node_id/disconnect`
- PUT `/v0/nodes/:node_id/settings`
- POST `/v0/health`（ノードからのヘルス/メトリクス送信、`X-Node-Token` 必須）
- GET `/v0/nodes/:node_id/logs`

#### モデル管理

- GET `/v0/models/available`（例: `?source=hf`）
- POST `/v0/models/register`
- GET `/v0/models/registered`
- DELETE `/v0/models/*model_name`
- POST `/v0/models/discover-gguf`
- POST `/v0/models/convert`
- GET `/v0/models/convert`
- GET `/v0/models/convert/:task_id`
- DELETE `/v0/models/convert/:task_id`
- GET `/v0/models/blob/:model_name`

#### ダッシュボード/監視

- GET `/v0/dashboard/overview`
- GET `/v0/dashboard/stats`
- GET `/v0/dashboard/nodes`
- GET `/v0/dashboard/metrics/:node_id`
- GET `/v0/dashboard/request-history`
- GET `/v0/dashboard/request-responses`
- GET `/v0/dashboard/request-responses/:id`
- GET `/v0/dashboard/request-responses/export`
- GET `/v0/dashboard/logs/router`
- GET `/v0/metrics/cloud`
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
