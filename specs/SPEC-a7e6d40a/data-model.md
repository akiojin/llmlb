# データモデル: CLI インターフェース整備

## CLI構造定義

### Router CLI (Rust/clap)

```rust
use clap::Parser;

#[derive(Parser)]
#[command(name = "llmlb")]
#[command(version, about = "LLM Load Balancer - OpenAI-compatible API gateway")]
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
| `LLMLB_PORT` | u16 | 8080 | 待受ポート |
| `LLMLB_HOST` | String | 0.0.0.0 | 待受アドレス |
| `LLMLB_LOG_LEVEL` | String | info | ログレベル |
| `LLMLB_JWT_SECRET` | String | (自動生成) | JWT署名キー |
| `LLMLB_ADMIN_USERNAME` | String | admin | 初期管理者名 |
| `LLMLB_ADMIN_PASSWORD` | String | (必須) | 初期管理者パスワード |

### Node環境変数

| 環境変数 | 型 | デフォルト | 説明 |
|----------|-----|-----------|------|
| `LLMLB_URL` | String | `http://127.0.0.1:8080` | ルーターURL |
| `XLLM_PORT` | u16 | 11435 | 待受ポート |
| `XLLM_IP` | String | (自動検出) | ノードIP |
| `XLLM_MODELS_DIR` | Path | ~/.runtime/models | モデル保存先 |
| `XLLM_LOG_LEVEL` | String | info | ログレベル |
| `XLLM_LOG_DIR` | Path | ~/.llmlb/logs | ログディレクトリ |
| `XLLM_HEARTBEAT_SECS` | u64 | 10 | ハートビート間隔 |
| `XLLM_BIND_ADDRESS` | String | 0.0.0.0 | バインドアドレス |

## JWT_SECRET永続化

### ファイル仕様

```rust
pub struct JwtSecretConfig {
    pub file_path: PathBuf,  // ~/.llmlb/jwt_secret
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
    if let Ok(secret) = std::env::var("LLMLB_JWT_SECRET") {
        return secret;
    }

    // 2. ファイルを確認
    let path = dirs::home_dir()
        .unwrap()
        .join(".llmlb/jwt_secret");
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

### Node

```text
LLM Node - Inference engine for LLM Load Balancer

Usage: xllm [OPTIONS]

Options:
  -h, --help     Print help information
  -V, --version  Print version information

Environment Variables:
  LLMLB_URL               Router URL (default: http://127.0.0.1:8080)
  XLLM_PORT                Node listen port (default: 11435)
  XLLM_MODELS_DIR          Model storage directory
  XLLM_LOG_LEVEL           Log level (default: info)
```

## バージョン出力形式

```text
llmlb 2.1.0
```

```text
xllm 0.1.0
```

## 終了コード

| コード | 意味 |
|--------|------|
| 0 | 正常終了（--help, --version） |
| 1 | 引数エラー |
| 2 | 設定エラー |
| 3 | 起動失敗 |
