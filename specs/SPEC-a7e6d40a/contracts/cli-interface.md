# CLI Interface Contract

## Router CLI

### コマンド形式

```text
llm-router [OPTIONS]
```

### オプション

| オプション | 短縮形 | 説明 |
|-----------|--------|------|
| `--help` | `-h` | ヘルプ表示 |
| `--version` | `-V` | バージョン表示 |

### 終了コード

| コード | 意味 |
|--------|------|
| 0 | 正常終了 |
| 1 | 引数エラー |
| 2 | 設定エラー |
| 3 | 起動失敗 |

### ヘルプ出力

```text
LLM Router - OpenAI-compatible API gateway

Usage: llm-router [OPTIONS]

Options:
  -h, --help     Print help information
  -V, --version  Print version information

Environment Variables:
  LLM_ROUTER_PORT              Server port (default: 8080)
  LLM_ROUTER_HOST              Server host (default: 0.0.0.0)
  LLM_ROUTER_LOG_LEVEL         Log level (default: info)
  LLM_ROUTER_JWT_SECRET        JWT signing secret (auto-generated)
  LLM_ROUTER_ADMIN_USERNAME    Initial admin username (default: admin)
  LLM_ROUTER_ADMIN_PASSWORD    Initial admin password (required on first run)
```

### バージョン出力

```text
llm-router {version}
```

## Node CLI

### コマンド形式

```text
llm-node [OPTIONS]
```

### オプション

| オプション | 短縮形 | 説明 |
|-----------|--------|------|
| `--help` | `-h` | ヘルプ表示 |
| `--version` | `-V` | バージョン表示 |

### 終了コード

| コード | 意味 |
|--------|------|
| 0 | 正常終了 |
| 1 | 引数エラー |
| 2 | 設定エラー |
| 3 | 起動失敗 |

### ヘルプ出力

```text
LLM Node - Inference engine for LLM Router

Usage: llm-node [OPTIONS]

Options:
  -h, --help     Print help information
  -V, --version  Print version information

Environment Variables:
  LLM_ROUTER_URL               Router URL (default: http://127.0.0.1:8080)
  LLM_NODE_PORT                Node listen port (default: 11435)
  LLM_NODE_MODELS_DIR          Model storage directory
  LLM_NODE_LOG_LEVEL           Log level (default: info)
```

### バージョン出力

```text
llm-node {version}
```

## 環境変数仕様

### Router環境変数

| 変数名 | 型 | 必須 | デフォルト | 説明 |
|--------|-----|------|-----------|------|
| `LLM_ROUTER_PORT` | u16 | No | 8080 | 待受ポート |
| `LLM_ROUTER_HOST` | String | No | 0.0.0.0 | 待受アドレス |
| `LLM_ROUTER_LOG_LEVEL` | String | No | info | ログレベル |
| `LLM_ROUTER_JWT_SECRET` | String | No | (自動生成) | JWT署名キー |
| `LLM_ROUTER_ADMIN_USERNAME` | String | No | admin | 初期管理者名 |
| `LLM_ROUTER_ADMIN_PASSWORD` | String | Yes* | - | 初期管理者パスワード |
| `LLM_ROUTER_DATABASE_URL` | String | No | sqlite://~/.llm-router/router.db | DB接続文字列 |
| `LLM_ROUTER_HEALTH_CHECK_INTERVAL` | u64 | No | 30 | ヘルスチェック間隔(秒) |
| `LLM_ROUTER_NODE_TIMEOUT` | u64 | No | 30 | ノードタイムアウト(秒) |
| `LLM_ROUTER_LOAD_BALANCER_MODE` | String | No | round_robin | 負荷分散モード |

*初回起動時のみ必須

### Node環境変数

| 変数名 | 型 | 必須 | デフォルト | 説明 |
|--------|-----|------|-----------|------|
| `LLM_ROUTER_URL` | String | No | `http://127.0.0.1:8080` | ルーターURL |
| `LLM_NODE_PORT` | u16 | No | 11435 | 待受ポート |
| `LLM_NODE_IP` | String | No | (自動検出) | ノードIP |
| `LLM_NODE_MODELS_DIR` | Path | No | ~/.runtime/models | モデル保存先 |
| `LLM_NODE_LOG_LEVEL` | String | No | info | ログレベル |
| `LLM_NODE_LOG_DIR` | Path | No | ~/.llm-node/logs | ログディレクトリ |
| `LLM_NODE_HEARTBEAT_SECS` | u64 | No | 10 | ハートビート間隔 |
| `LLM_NODE_BIND_ADDRESS` | String | No | 0.0.0.0 | バインドアドレス |
| `LLM_NODE_LOG_RETENTION_DAYS` | u64 | No | 7 | ログ保持日数 |

## JWT_SECRET ファイル仕様

### ファイルパス

```text
~/.llm-router/jwt_secret
```

### パーミッション

```text
-rw------- (0600)
```

### ファイル形式

- プレーンテキスト
- UUIDv4形式
- 改行なし

### 例

```text
550e8400-e29b-41d4-a716-446655440000
```

### 読み込み優先順位

1. 環境変数 `LLM_ROUTER_JWT_SECRET`
2. ファイル `~/.llm-router/jwt_secret`
3. 自動生成（ファイルに保存）
