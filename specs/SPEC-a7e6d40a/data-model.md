# データモデル: CLI インターフェース整備

## CLI構造定義

### Router CLI (Rust/clap)

```rust
use clap::Parser;

#[derive(Parser)]
#[command(name = "llm-router")]
#[command(version, about = "LLM Router - OpenAI-compatible API gateway")]
pub struct Cli {
    // 引数なしでサーバー起動（デフォルト動作）
}
```

### Node CLI (C++)

```cpp
struct CliOptions {
    bool show_help = false;
    bool show_version = false;
};

CliOptions parse_args(int argc, char** argv);
```

## 環境変数定義

### Router環境変数

| 環境変数 | 型 | デフォルト | 説明 |
|----------|-----|-----------|------|
| `LLM_ROUTER_PORT` | u16 | 8080 | 待受ポート |
| `LLM_ROUTER_HOST` | String | 0.0.0.0 | 待受アドレス |
| `LLM_ROUTER_LOG_LEVEL` | String | info | ログレベル |
| `LLM_ROUTER_JWT_SECRET` | String | (自動生成) | JWT署名キー |
| `LLM_ROUTER_ADMIN_USERNAME` | String | admin | 初期管理者名 |
| `LLM_ROUTER_ADMIN_PASSWORD` | String | (必須) | 初期管理者パスワード |

### Node環境変数

| 環境変数 | 型 | デフォルト | 説明 |
|----------|-----|-----------|------|
| `LLM_ROUTER_URL` | String | `http://127.0.0.1:8080` | ルーターURL |
| `LLM_NODE_PORT` | u16 | 11435 | 待受ポート |
| `LLM_NODE_IP` | String | (自動検出) | ノードIP |
| `LLM_NODE_MODELS_DIR` | Path | ~/.runtime/models | モデル保存先 |
| `LLM_NODE_LOG_LEVEL` | String | info | ログレベル |
| `LLM_NODE_LOG_DIR` | Path | ~/.llm-node/logs | ログディレクトリ |
| `LLM_NODE_HEARTBEAT_SECS` | u64 | 10 | ハートビート間隔 |
| `LLM_NODE_BIND_ADDRESS` | String | 0.0.0.0 | バインドアドレス |

## JWT_SECRET永続化

### ファイル仕様

```rust
pub struct JwtSecretConfig {
    pub file_path: PathBuf,  // ~/.llm-router/jwt_secret
    pub permissions: u32,    // 0o600
}
```

### ファイル形式

- プレーンテキスト（UUIDv4形式）
- 改行なし
- 例: `550e8400-e29b-41d4-a716-446655440000`

### 読み込みロジック

```rust
fn load_jwt_secret() -> String {
    // 1. 環境変数を確認
    if let Ok(secret) = std::env::var("LLM_ROUTER_JWT_SECRET") {
        return secret;
    }

    // 2. ファイルを確認
    let path = dirs::home_dir()
        .unwrap()
        .join(".llm-router/jwt_secret");
    if let Ok(secret) = std::fs::read_to_string(&path) {
        return secret.trim().to_string();
    }

    // 3. 新規生成して保存
    let secret = Uuid::new_v4().to_string();
    std::fs::create_dir_all(path.parent().unwrap()).ok();
    std::fs::write(&path, &secret).ok();
    #[cfg(unix)]
    std::fs::set_permissions(&path, Permissions::from_mode(0o600)).ok();
    secret
}
```

## ヘルプ出力形式

### Router

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

### Node

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

## バージョン出力形式

```text
llm-router 2.1.0
```

```text
llm-node 0.1.0
```

## 終了コード

| コード | 意味 |
|--------|------|
| 0 | 正常終了（--help, --version） |
| 1 | 引数エラー |
| 2 | 設定エラー |
| 3 | 起動失敗 |
