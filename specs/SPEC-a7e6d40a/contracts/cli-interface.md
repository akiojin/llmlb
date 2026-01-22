# CLI Interface Contract

## Load Balancer CLI

### コマンド形式

```text
llmlb [OPTIONS]
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
LLM Load Balancer - OpenAI-compatible API gateway

Usage: llmlb [OPTIONS]

Options:
  -h, --help     Print help information
  -V, --version  Print version information

Environment Variables:
  LLMLB_PORT              Server port (default: 8080)
  LLMLB_HOST              Server host (default: 0.0.0.0)
  LLMLB_LOG_LEVEL         Log level (default: info)
  LLMLB_JWT_SECRET        JWT signing secret (auto-generated)
  LLMLB_ADMIN_USERNAME    Initial admin username (default: admin)
  LLMLB_ADMIN_PASSWORD    Initial admin password (required on first run)
```

### バージョン出力

```text
llmlb {version}
```

## Node CLI

### コマンド形式

```text
xllm [OPTIONS]
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
LLM Node - Inference engine for LLM Load Balancer

Usage: xllm [OPTIONS]

Options:
  -h, --help     Print help information
  -V, --version  Print version information

Environment Variables:
  LLMLB_URL               Load Balancer URL (default: http://127.0.0.1:8080)
  XLLM_PORT                Node listen port (default: 11435)
  XLLM_MODELS_DIR          Model storage directory
  XLLM_LOG_LEVEL           Log level (default: info)
```

### バージョン出力

```text
xllm {version}
```

## 環境変数仕様

### Load Balancer環境変数

| 変数名 | 型 | 必須 | デフォルト | 説明 |
|--------|-----|------|-----------|------|
| `LLMLB_PORT` | u16 | No | 8080 | 待受ポート |
| `LLMLB_HOST` | String | No | 0.0.0.0 | 待受アドレス |
| `LLMLB_LOG_LEVEL` | String | No | info | ログレベル |
| `LLMLB_JWT_SECRET` | String | No | (自動生成) | JWT署名キー |
| `LLMLB_ADMIN_USERNAME` | String | No | admin | 初期管理者名 |
| `LLMLB_ADMIN_PASSWORD` | String | Yes* | - | 初期管理者パスワード |
| `LLMLB_DATABASE_URL` | String | No | sqlite://~/.llmlb/router.db | DB接続文字列 |
| `LLMLB_HEALTH_CHECK_INTERVAL` | u64 | No | 30 | ヘルスチェック間隔(秒) |
| `LLMLB_NODE_TIMEOUT` | u64 | No | 30 | ノードタイムアウト(秒) |
| `LLMLB_LOAD_BALANCER_MODE` | String | No | round_robin | 負荷分散モード |

*初回起動時のみ必須

### Node環境変数

| 変数名 | 型 | 必須 | デフォルト | 説明 |
|--------|-----|------|-----------|------|
| `LLMLB_URL` | String | No | `http://127.0.0.1:8080` | ロードバランサーURL |
| `XLLM_PORT` | u16 | No | 11435 | 待受ポート |
| `XLLM_IP` | String | No | (自動検出) | ノードIP |
| `XLLM_MODELS_DIR` | Path | No | ~/.runtime/models | モデル保存先 |
| `XLLM_LOG_LEVEL` | String | No | info | ログレベル |
| `XLLM_LOG_DIR` | Path | No | ~/.llmlb/logs | ログディレクトリ |
| `XLLM_HEARTBEAT_SECS` | u64 | No | 10 | ハートビート間隔 |
| `XLLM_BIND_ADDRESS` | String | No | 0.0.0.0 | バインドアドレス |
| `XLLM_LOG_RETENTION_DAYS` | u64 | No | 7 | ログ保持日数 |

## JWT_SECRET ファイル仕様

### ファイルパス

```text
~/.llmlb/jwt_secret
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

1. 環境変数 `LLMLB_JWT_SECRET`
2. ファイル `~/.llmlb/jwt_secret`
3. 自動生成（ファイルに保存）
